//! Fast Indexer - Ultra-fast codebase indexing with multiple optimization strategies
//!
//! Optimizations:
//! 1. Parallel multi-threaded file processing with rayon
//! 2. Batch embedding requests (process 50 files at once)
//! 3. Tiered indexing (BM25-only fast mode vs full hybrid)
//! 4. Incremental delta indexing (only changed files)
//! 5. Streaming/lazy indexing on first search

use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::core::chunker::{Chunk, CodeChunker};
use crate::core::config::Config;
use crate::core::embeddings::EmbeddingProvider;
use crate::core::local_embeddings::SpeedMode;
use crate::core::scanner::{FileScanner, ScannedFile};
use crate::core::store::{
    compute_file_hash, generate_chunk_id, FileChunk, IndexedFile, VectorStore,
};

/// Indexing tier/quality level
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IndexTier {
    /// BM25 only - instant, keyword-based search (no embeddings)
    Fast,
    /// BM25 + lightweight embeddings (balanced speed/quality)
    Balanced,
    /// Full hybrid with best quality embeddings
    Quality,
}

impl Default for IndexTier {
    fn default() -> Self {
        Self::Balanced
    }
}

/// Configuration for the fast indexer
#[derive(Debug, Clone)]
pub struct FastIndexConfig {
    /// Indexing tier
    pub tier: IndexTier,
    /// Batch size for embedding requests
    pub batch_size: usize,
    /// Number of parallel threads (0 = auto)
    pub num_threads: usize,
    /// Skip unchanged files (incremental indexing)
    pub incremental: bool,
    /// Maximum file size to index (bytes)
    pub max_file_size: usize,
}

impl Default for FastIndexConfig {
    fn default() -> Self {
        Self {
            tier: IndexTier::Balanced,
            batch_size: 50,
            num_threads: 0, // auto-detect
            incremental: true,
            max_file_size: 1024 * 1024, // 1MB
        }
    }
}

/// Result of indexing operation
#[derive(Debug)]
pub struct IndexResult {
    pub total_files: usize,
    pub indexed_files: usize,
    pub skipped_files: usize,
    pub total_chunks: usize,
    pub duration_ms: u128,
    pub tier: IndexTier,
}

/// Processed file ready for storage
struct ProcessedFile {
    file: ScannedFile,
    hash: String,
    chunks: Vec<Chunk>,
}

/// Fast parallel indexer
pub struct FastIndexer {
    config: FastIndexConfig,
    app_config: Config,
}

impl FastIndexer {
    pub fn new(config: FastIndexConfig) -> Result<Self> {
        let app_config = Config::load()?;

        // Configure rayon thread pool if specified
        if config.num_threads > 0 {
            rayon::ThreadPoolBuilder::new()
                .num_threads(config.num_threads)
                .build_global()
                .ok(); // Ignore if already initialized
        }

        Ok(Self { config, app_config })
    }

    /// Index a directory with all optimizations
    pub async fn index(&self, path: &str, store_name: Option<&str>) -> Result<IndexResult> {
        let start = Instant::now();
        let abs_path = std::fs::canonicalize(path)?;
        let path_str = abs_path.to_string_lossy().to_string();

        // Load existing store for incremental indexing
        let store = Arc::new(Mutex::new(VectorStore::load(store_name)?));

        // Phase 1: Scan files (parallel)
        let scanner = FileScanner::new(&path_str);
        let all_files = scanner.scan()?;
        let total_files = all_files.len();

        if total_files == 0 {
            return Ok(IndexResult {
                total_files: 0,
                indexed_files: 0,
                skipped_files: 0,
                total_chunks: 0,
                duration_ms: start.elapsed().as_millis(),
                tier: self.config.tier,
            });
        }

        // Phase 2: Filter unchanged files (incremental)
        let files_to_process: Vec<ScannedFile> = if self.config.incremental {
            let store_guard = store.lock().unwrap();
            all_files
                .into_iter()
                .filter(|f| {
                    let hash = compute_file_hash(&f.content);
                    store_guard.file_needs_update(&f.path, &hash)
                })
                .collect()
        } else {
            all_files
        };

        let skipped_files = total_files - files_to_process.len();

        if files_to_process.is_empty() {
            return Ok(IndexResult {
                total_files,
                indexed_files: 0,
                skipped_files,
                total_chunks: 0,
                duration_ms: start.elapsed().as_millis(),
                tier: self.config.tier,
            });
        }

        // Phase 3: Parallel chunking
        let chunker = CodeChunker::default();
        let processed_files: Vec<ProcessedFile> = files_to_process
            .into_par_iter()
            .filter(|f| f.content.len() <= self.config.max_file_size)
            .map(|file| {
                let hash = compute_file_hash(&file.content);
                let chunks = chunker.chunk(&file.content, file.language.as_deref());
                ProcessedFile { file, hash, chunks }
            })
            .filter(|pf| !pf.chunks.is_empty())
            .collect();

        let indexed_files = processed_files.len();

        // Phase 4: Generate embeddings based on tier
        let total_chunks = match self.config.tier {
            IndexTier::Fast => {
                // BM25 only - no embeddings needed, just store chunks
                self.store_chunks_bm25_only(&store, processed_files)?
            }
            IndexTier::Balanced | IndexTier::Quality => {
                // Generate embeddings in batches
                self.store_chunks_with_embeddings(&store, processed_files, store_name)
                    .await?
            }
        };

        // Phase 5: Update BM25 stats and save
        {
            let mut store_guard = store.lock().unwrap();
            store_guard.update_bm25_stats();
            store_guard.save(store_name)?;
        }

        Ok(IndexResult {
            total_files,
            indexed_files,
            skipped_files,
            total_chunks,
            duration_ms: start.elapsed().as_millis(),
            tier: self.config.tier,
        })
    }

    /// Store chunks with BM25 only (no embeddings) - ultra fast
    fn store_chunks_bm25_only(
        &self,
        store: &Arc<Mutex<VectorStore>>,
        processed_files: Vec<ProcessedFile>,
    ) -> Result<usize> {
        let mut total_chunks = 0;
        let mut store_guard = store.lock().unwrap();

        for pf in processed_files {
            // Remove old data
            store_guard.remove_file(&pf.file.path);

            let mut chunk_ids = Vec::new();
            for chunk in &pf.chunks {
                let chunk_id = generate_chunk_id(&pf.file.path, chunk.start_line, chunk.end_line);
                chunk_ids.push(chunk_id.clone());

                // Empty embedding for BM25-only mode
                store_guard.add_chunk(FileChunk {
                    id: chunk_id,
                    file_path: pf.file.path.clone(),
                    content: chunk.content.clone(),
                    start_line: chunk.start_line,
                    end_line: chunk.end_line,
                    chunk_type: chunk.chunk_type.as_str().to_string(),
                    language: pf.file.language.clone(),
                    embedding: vec![], // Empty for BM25 mode
                    token_embeddings: None,
                });
                total_chunks += 1;
            }

            store_guard.add_file(IndexedFile {
                path: pf.file.path.clone(),
                hash: pf.hash,
                chunks: chunk_ids,
                indexed_at: chrono::Utc::now().to_rfc3339(),
            });
        }

        Ok(total_chunks)
    }

    /// Store chunks with embeddings in batches
    async fn store_chunks_with_embeddings(
        &self,
        store: &Arc<Mutex<VectorStore>>,
        processed_files: Vec<ProcessedFile>,
        _store_name: Option<&str>,
    ) -> Result<usize> {
        let speed_mode = match self.config.tier {
            IndexTier::Fast => SpeedMode::Fast,
            IndexTier::Balanced => SpeedMode::Balanced,
            IndexTier::Quality => SpeedMode::Quality,
        };

        let embeddings_provider =
            EmbeddingProvider::with_speed_mode(self.app_config.clone(), speed_mode);

        // Collect all chunks with their metadata
        let mut all_chunks: Vec<(String, String, Chunk, Option<String>)> = Vec::new(); // (file_path, hash, chunk, language)
        let mut file_chunk_ranges: HashMap<String, (String, Vec<usize>)> = HashMap::new(); // file_path -> (hash, chunk indices)

        for pf in &processed_files {
            let start_idx = all_chunks.len();
            for chunk in &pf.chunks {
                all_chunks.push((
                    pf.file.path.clone(),
                    pf.hash.clone(),
                    chunk.clone(),
                    pf.file.language.clone(),
                ));
            }
            let end_idx = all_chunks.len();
            file_chunk_ranges.insert(
                pf.file.path.clone(),
                (pf.hash.clone(), (start_idx..end_idx).collect()),
            );
        }

        let total_chunks = all_chunks.len();

        if total_chunks == 0 {
            return Ok(0);
        }

        // Process in batches
        let batch_size = self.config.batch_size;
        let mut all_embeddings: Vec<Vec<f32>> = Vec::with_capacity(total_chunks);

        for batch_start in (0..total_chunks).step_by(batch_size) {
            let batch_end = (batch_start + batch_size).min(total_chunks);
            let batch_texts: Vec<String> = all_chunks[batch_start..batch_end]
                .iter()
                .map(|(_, _, chunk, _)| chunk.content.clone())
                .collect();

            match embeddings_provider.embed(&batch_texts).await {
                Ok(embeddings) => {
                    all_embeddings.extend(embeddings);
                }
                Err(e) => {
                    eprintln!("Batch embedding error: {}", e);
                    // Fill with empty embeddings on error
                    all_embeddings.extend(vec![vec![]; batch_texts.len()]);
                }
            }
        }

        // Store all chunks with embeddings
        {
            let mut store_guard = store.lock().unwrap();

            // First, remove old files
            for (file_path, _) in &file_chunk_ranges {
                store_guard.remove_file(file_path);
            }

            // Add chunks with embeddings
            for (idx, (file_path, _hash, chunk, language)) in all_chunks.iter().enumerate() {
                let chunk_id = generate_chunk_id(file_path, chunk.start_line, chunk.end_line);

                let embedding = if idx < all_embeddings.len() {
                    all_embeddings[idx].clone()
                } else {
                    vec![]
                };

                store_guard.add_chunk(FileChunk {
                    id: chunk_id,
                    file_path: file_path.clone(),
                    content: chunk.content.clone(),
                    start_line: chunk.start_line,
                    end_line: chunk.end_line,
                    chunk_type: chunk.chunk_type.as_str().to_string(),
                    language: language.clone(),
                    embedding,
                    token_embeddings: None,
                });
            }

            // Add file metadata
            for (file_path, (hash, chunk_indices)) in &file_chunk_ranges {
                let chunk_ids: Vec<String> = chunk_indices
                    .iter()
                    .map(|&idx| {
                        let (_, _, chunk, _) = &all_chunks[idx];
                        generate_chunk_id(file_path, chunk.start_line, chunk.end_line)
                    })
                    .collect();

                store_guard.add_file(IndexedFile {
                    path: file_path.clone(),
                    hash: hash.clone(),
                    chunks: chunk_ids,
                    indexed_at: chrono::Utc::now().to_rfc3339(),
                });
            }
        }

        Ok(total_chunks)
    }
}

/// Lazy indexer for on-demand indexing during search
pub struct LazyIndexer {
    indexed_paths: Arc<Mutex<HashMap<String, bool>>>,
}

impl LazyIndexer {
    pub fn new() -> Self {
        Self {
            indexed_paths: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if a path needs indexing, index if needed
    pub async fn ensure_indexed(&self, path: &str, store_name: Option<&str>) -> Result<bool> {
        let abs_path = std::fs::canonicalize(path)?;
        let path_key = abs_path.to_string_lossy().to_string();

        // Check if already indexed this session
        {
            let indexed = self.indexed_paths.lock().unwrap();
            if indexed.contains_key(&path_key) {
                return Ok(false); // Already indexed
            }
        }

        // Check if store exists and has content
        let store = VectorStore::load(store_name)?;
        if store.file_count() > 0 {
            // Mark as indexed
            let mut indexed = self.indexed_paths.lock().unwrap();
            indexed.insert(path_key, true);
            return Ok(false);
        }

        // Need to index - use fast BM25 mode for instant results
        let config = FastIndexConfig {
            tier: IndexTier::Fast,
            incremental: true,
            ..Default::default()
        };

        let indexer = FastIndexer::new(config)?;
        indexer.index(path, store_name).await?;

        // Mark as indexed
        {
            let mut indexed = self.indexed_paths.lock().unwrap();
            indexed.insert(path_key, true);
        }

        Ok(true) // Did index
    }

    /// Upgrade from BM25-only to full embeddings in background
    pub async fn upgrade_to_semantic(
        &self,
        path: &str,
        store_name: Option<&str>,
    ) -> Result<IndexResult> {
        let config = FastIndexConfig {
            tier: IndexTier::Balanced,
            incremental: false, // Re-index everything with embeddings
            ..Default::default()
        };

        let indexer = FastIndexer::new(config)?;
        indexer.index(path, store_name).await
    }
}

impl Default for LazyIndexer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_tier_default() {
        assert_eq!(IndexTier::default(), IndexTier::Balanced);
    }

    #[test]
    fn test_fast_index_config_default() {
        let config = FastIndexConfig::default();
        assert_eq!(config.batch_size, 50);
        assert!(config.incremental);
    }
}
