use std::collections::HashMap;

use crate::core::embeddings::{colbert_max_sim, cosine_similarity};
use crate::core::store::{FileChunk, VectorStore};

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub chunk: FileChunk,
    pub score: f32,
    pub bm25_score: f32,
    pub vector_score: f32,
    pub colbert_score: Option<f32>,
}

pub struct HybridSearcher {
    bm25_weight: f32,
    vector_weight: f32,
    k1: f32,
    b: f32,
}

impl Default for HybridSearcher {
    fn default() -> Self {
        Self {
            bm25_weight: 0.3,
            vector_weight: 0.7,
            k1: 1.2,
            b: 0.75,
        }
    }
}

impl HybridSearcher {
    pub fn new(bm25_weight: f32, vector_weight: f32) -> Self {
        Self {
            bm25_weight,
            vector_weight,
            k1: 1.2,
            b: 0.75,
        }
    }

    pub fn search(
        &self,
        store: &VectorStore,
        query_embedding: &[f32],
        query_text: &str,
        limit: usize,
        file_types: Option<&[String]>,
        use_colbert: bool,
        query_token_embeddings: Option<&[Vec<f32>]>,
    ) -> Vec<SearchResult> {
        let query_terms: Vec<String> = query_text
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        let avg_doc_len = if store.doc_count > 0 {
            store
                .chunks
                .values()
                .map(|c| c.content.len())
                .sum::<usize>() as f32
                / store.doc_count as f32
        } else {
            500.0
        };

        // Try ANN fast path first (for large indexes)
        // Fetch more candidates than limit to allow for filtering and reranking
        let ann_candidates = store.ann_search(query_embedding, limit * 3);

        let chunks_iter: Box<dyn Iterator<Item = &FileChunk>> =
            if let Some(ref candidates) = ann_candidates {
                // Fast path: only score ANN candidates
                Box::new(
                    candidates
                        .iter()
                        .filter_map(|(chunk_id, _)| store.chunks.get(chunk_id)),
                )
            } else {
                // Slow path: brute force all chunks
                Box::new(store.chunks.values())
            };

        let mut results: Vec<SearchResult> = chunks_iter
            .filter(|chunk| {
                // Filter by file type if specified
                if let Some(types) = file_types {
                    let file_ext = std::path::Path::new(&chunk.file_path)
                        .extension()
                        .map(|e| e.to_string_lossy().to_lowercase());

                    match file_ext {
                        Some(ext) => types.iter().any(|t| t.to_lowercase() == ext),
                        None => false,
                    }
                } else {
                    true
                }
            })
            .map(|chunk| {
                // Vector similarity (recompute for exact score, ANN gives approximate)
                let vector_score = cosine_similarity(query_embedding, &chunk.embedding);

                // BM25 score
                let bm25_score =
                    self.compute_bm25(&chunk.content, &query_terms, &store.bm25_idf, avg_doc_len);

                // ColBERT score (optional)
                let colbert_score = if use_colbert {
                    query_token_embeddings.and_then(|q_tokens| {
                        chunk
                            .token_embeddings
                            .as_ref()
                            .map(|d_tokens| colbert_max_sim(q_tokens, d_tokens))
                    })
                } else {
                    None
                };

                // Combined score
                let combined_score = if let Some(col_score) = colbert_score {
                    // When using ColBERT, blend all three
                    self.vector_weight * 0.5 * vector_score
                        + self.vector_weight * 0.5 * col_score
                        + self.bm25_weight * Self::normalize_bm25(bm25_score)
                } else {
                    self.vector_weight * vector_score
                        + self.bm25_weight * Self::normalize_bm25(bm25_score)
                };

                SearchResult {
                    chunk: chunk.clone(),
                    score: combined_score,
                    bm25_score,
                    vector_score,
                    colbert_score,
                }
            })
            .collect();

        // Sort by combined score
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Return top results
        results.truncate(limit);
        results
    }

    fn compute_bm25(
        &self,
        content: &str,
        query_terms: &[String],
        idf: &HashMap<String, f32>,
        avg_doc_len: f32,
    ) -> f32 {
        let doc_len = content.len() as f32;
        let content_lower = content.to_lowercase();

        // Count term frequencies
        let mut term_freq: HashMap<&str, usize> = HashMap::new();
        for word in content_lower.split_whitespace() {
            *term_freq.entry(word).or_insert(0) += 1;
        }

        let mut score = 0.0;
        for term in query_terms {
            let tf = *term_freq.get(term.as_str()).unwrap_or(&0) as f32;
            let term_idf = *idf.get(term).unwrap_or(&0.0);

            if tf > 0.0 {
                let numerator = tf * (self.k1 + 1.0);
                let denominator = tf + self.k1 * (1.0 - self.b + self.b * doc_len / avg_doc_len);
                score += term_idf * (numerator / denominator);
            }
        }

        score
    }

    fn normalize_bm25(score: f32) -> f32 {
        // Sigmoid normalization to [0, 1]
        1.0 / (1.0 + (-score * 0.1).exp())
    }
}

// Quick vector-only search (uses ANN when available)
pub fn vector_search(
    store: &VectorStore,
    query_embedding: &[f32],
    limit: usize,
    file_types: Option<&[String]>,
) -> Vec<SearchResult> {
    // Try ANN fast path
    let ann_candidates = store.ann_search(query_embedding, limit * 3);

    let chunks_iter: Box<dyn Iterator<Item = &FileChunk>> =
        if let Some(ref candidates) = ann_candidates {
            Box::new(
                candidates
                    .iter()
                    .filter_map(|(chunk_id, _)| store.chunks.get(chunk_id)),
            )
        } else {
            Box::new(store.chunks.values())
        };

    let mut results: Vec<SearchResult> = chunks_iter
        .filter(|chunk| {
            if let Some(types) = file_types {
                let file_ext = std::path::Path::new(&chunk.file_path)
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase());

                match file_ext {
                    Some(ext) => types.iter().any(|t| t.to_lowercase() == ext),
                    None => false,
                }
            } else {
                true
            }
        })
        .map(|chunk| {
            let score = cosine_similarity(query_embedding, &chunk.embedding);
            SearchResult {
                chunk: chunk.clone(),
                score,
                bm25_score: 0.0,
                vector_score: score,
                colbert_score: None,
            }
        })
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit);
    results
}
