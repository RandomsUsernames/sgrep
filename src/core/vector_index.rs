use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

use crate::core::config::Config;

/// Fast ANN index using usearch
/// Falls back to brute force for small collections
pub struct VectorIndex {
    index: Option<Index>,
    id_to_chunk: HashMap<u64, String>, // usearch key -> chunk_id
    chunk_to_id: HashMap<String, u64>, // chunk_id -> usearch key
    next_id: u64,
    dimensions: usize,
    threshold: usize, // Use brute force below this count
}

impl VectorIndex {
    const DEFAULT_THRESHOLD: usize = 1000;
    const DEFAULT_DIMENSIONS: usize = 768;

    pub fn new(dimensions: usize) -> Result<Self> {
        Ok(Self {
            index: None,
            id_to_chunk: HashMap::new(),
            chunk_to_id: HashMap::new(),
            next_id: 0,
            dimensions,
            threshold: Self::DEFAULT_THRESHOLD,
        })
    }

    pub fn with_threshold(mut self, threshold: usize) -> Self {
        self.threshold = threshold;
        self
    }

    /// Initialize the usearch index
    fn init_index(&mut self, capacity: usize) -> Result<()> {
        let options = IndexOptions {
            dimensions: self.dimensions,
            metric: MetricKind::Cos, // Cosine similarity
            quantization: ScalarKind::F32,
            connectivity: 16,     // M parameter - edges per node
            expansion_add: 128,   // efConstruction
            expansion_search: 64, // ef - higher = more accurate, slower
            multi: false,
        };

        let index = Index::new(&options).context("Failed to create usearch index")?;

        index
            .reserve(capacity)
            .context("Failed to reserve index capacity")?;

        self.index = Some(index);
        Ok(())
    }

    /// Add a vector to the index
    pub fn add(&mut self, chunk_id: &str, embedding: &[f32]) -> Result<()> {
        if embedding.len() != self.dimensions {
            anyhow::bail!(
                "Embedding dimension mismatch: expected {}, got {}",
                self.dimensions,
                embedding.len()
            );
        }

        // Initialize index if needed and we're above threshold
        if self.index.is_none() && self.id_to_chunk.len() >= self.threshold {
            self.init_index(self.id_to_chunk.len() * 2)?;
            // Re-add existing vectors
            self.rebuild_index()?;
        }

        let key = self.next_id;
        self.next_id += 1;

        self.id_to_chunk.insert(key, chunk_id.to_string());
        self.chunk_to_id.insert(chunk_id.to_string(), key);

        if let Some(ref index) = self.index {
            index
                .add(key, embedding)
                .context("Failed to add vector to index")?;
        }

        Ok(())
    }

    /// Remove a vector from the index
    pub fn remove(&mut self, chunk_id: &str) -> Result<bool> {
        if let Some(key) = self.chunk_to_id.remove(chunk_id) {
            self.id_to_chunk.remove(&key);

            if let Some(ref index) = self.index {
                index
                    .remove(key)
                    .context("Failed to remove vector from index")?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Search for nearest neighbors
    /// Returns (chunk_id, distance) pairs
    pub fn search(&self, query: &[f32], limit: usize) -> Result<Vec<(String, f32)>> {
        if self.id_to_chunk.is_empty() {
            return Ok(vec![]);
        }

        // Use usearch if available, otherwise this would need brute force
        // But brute force is handled at the HybridSearcher level
        if let Some(ref index) = self.index {
            let results = index
                .search(query, limit)
                .context("usearch search failed")?;

            let mut output = Vec::with_capacity(results.keys.len());
            for (key, distance) in results.keys.iter().zip(results.distances.iter()) {
                if let Some(chunk_id) = self.id_to_chunk.get(key) {
                    // Convert distance to similarity (usearch returns distance for cosine)
                    let similarity = 1.0 - distance;
                    output.push((chunk_id.clone(), similarity));
                }
            }
            Ok(output)
        } else {
            // Below threshold - return empty, let caller do brute force
            Ok(vec![])
        }
    }

    /// Check if using ANN index (vs brute force)
    pub fn is_indexed(&self) -> bool {
        self.index.is_some()
    }

    /// Get count of indexed vectors
    pub fn len(&self) -> usize {
        self.id_to_chunk.len()
    }

    pub fn is_empty(&self) -> bool {
        self.id_to_chunk.is_empty()
    }

    /// Clear the index
    pub fn clear(&mut self) {
        self.index = None;
        self.id_to_chunk.clear();
        self.chunk_to_id.clear();
        self.next_id = 0;
    }

    /// Rebuild index from stored mappings (used after loading)
    fn rebuild_index(&mut self) -> Result<()> {
        // Index needs to be rebuilt with actual vectors
        // This is a placeholder - actual rebuild happens in VectorStore
        Ok(())
    }

    /// Save index to disk
    pub fn save(&self, store_name: Option<&str>) -> Result<()> {
        if let Some(ref index) = self.index {
            let path = Self::index_path(store_name)?;
            index
                .save(&path.to_string_lossy())
                .context("Failed to save usearch index")?;
        }
        Ok(())
    }

    /// Load index from disk
    pub fn load(store_name: Option<&str>, dimensions: usize) -> Result<Self> {
        let path = Self::index_path(store_name)?;

        let mut vi = Self::new(dimensions)?;

        if path.exists() {
            let options = IndexOptions {
                dimensions,
                metric: MetricKind::Cos,
                quantization: ScalarKind::F32,
                connectivity: 16,
                expansion_add: 128,
                expansion_search: 64,
                multi: false,
            };

            let index = Index::new(&options)?;
            index
                .load(&path.to_string_lossy())
                .context("Failed to load usearch index")?;

            vi.index = Some(index);
        }

        Ok(vi)
    }

    fn index_path(store_name: Option<&str>) -> Result<PathBuf> {
        let config_dir = Config::config_dir()?;
        let name = store_name.unwrap_or("default");
        Ok(config_dir.join(format!("{}.usearch", name)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_index_no_ann() {
        let mut idx = VectorIndex::new(4).unwrap();
        idx.add("chunk1", &[1.0, 0.0, 0.0, 0.0]).unwrap();
        idx.add("chunk2", &[0.0, 1.0, 0.0, 0.0]).unwrap();

        // Below threshold, no ANN index
        assert!(!idx.is_indexed());
        assert_eq!(idx.len(), 2);
    }

    #[test]
    fn test_large_index_uses_ann() {
        let mut idx = VectorIndex::new(4).unwrap().with_threshold(5);

        for i in 0..10 {
            let mut vec = [0.0f32; 4];
            vec[i % 4] = 1.0;
            idx.add(&format!("chunk{}", i), &vec).unwrap();
        }

        // Above threshold, should use ANN
        assert!(idx.is_indexed());
    }
}
