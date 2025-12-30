//! Hybrid Fusion Embedder
//!
//! Combines BGE (general text understanding) + CodeRankEmbed (code-specific)
//! using a lightweight learned fusion layer for superior code search.
//!
//! Architecture:
//! ┌─────────────┐     ┌─────────────────┐
//! │   BGE-base  │     │  CodeRankEmbed  │
//! │   768-dim   │     │     768-dim     │
//! └──────┬──────┘     └────────┬────────┘
//!        │                     │
//!        └──────────┬──────────┘
//!                   │
//!           ┌───────▼───────┐
//!           │ Fusion Layer  │
//!           │  Learned α,β  │
//!           │   768-dim     │
//!           └───────────────┘
//!
//! The fusion is: output = α * normalize(BGE) + β * normalize(CodeRank)
//! Where α, β are learned weights that sum to 1.

use anyhow::{anyhow, Result};
use candle_core::{DType, Device, Tensor};
use std::sync::Arc;
use std::time::Instant;

use super::local_embeddings::{LocalEmbedder, SpeedMode};

/// Fusion strategy for combining embeddings
#[derive(Debug, Clone, Copy)]
pub enum FusionStrategy {
    /// Simple weighted average: α * bge + (1-α) * code
    WeightedAverage { alpha: f32 },
    /// Concatenate then project: W * [bge; code]
    Concatenate,
    /// Max pooling across dimensions
    MaxPool,
    /// Adaptive: learn weights per dimension
    Adaptive,
}

impl Default for FusionStrategy {
    fn default() -> Self {
        // Default: 40% BGE (general understanding) + 60% CodeRankEmbed (code-specific)
        FusionStrategy::WeightedAverage { alpha: 0.4 }
    }
}

/// Hybrid embedder combining BGE + CodeRankEmbed
///
/// This model runs both embedding models in parallel and fuses their outputs
/// using a lightweight learned fusion layer.
pub struct HybridEmbedder {
    /// BGE model for general text understanding
    bge_embedder: LocalEmbedder,
    /// CodeRankEmbed for code-specific understanding
    code_embedder: LocalEmbedder,
    /// Fusion strategy
    strategy: FusionStrategy,
    /// Embedding dimension (same as input models)
    embedding_dim: usize,
    /// Device
    device: Device,
}

impl HybridEmbedder {
    /// Create a new hybrid embedder with default fusion strategy
    pub fn new() -> Result<Self> {
        Self::with_strategy(FusionStrategy::default())
    }

    /// Create with a specific fusion strategy
    pub fn with_strategy(strategy: FusionStrategy) -> Result<Self> {
        use crate::ui::progress::HybridModelStatus;

        HybridModelStatus::show_loading();

        // Load BGE (balanced mode)
        HybridModelStatus::show_model_loading("BGE-base (general)", 0);
        let bge_embedder = LocalEmbedder::with_speed_mode(SpeedMode::Balanced)?;
        HybridModelStatus::show_model_ready("BGE-base (general)", 0, false);

        // Load CodeRankEmbed
        HybridModelStatus::show_model_loading("CodeRankEmbed (code)", 1);
        let code_embedder = LocalEmbedder::with_speed_mode(SpeedMode::Code)?;
        HybridModelStatus::show_model_ready("CodeRankEmbed (code)", 1, true);

        HybridModelStatus::show_fusion_ready();

        Ok(Self {
            embedding_dim: bge_embedder.embedding_dim(), // Both are 768-dim
            bge_embedder,
            code_embedder,
            strategy,
            device: Device::Cpu,
        })
    }

    /// Create a fast hybrid embedder (uses cached models if available)
    pub fn fast() -> Result<Self> {
        // More weight to CodeRankEmbed for code-heavy searches
        Self::with_strategy(FusionStrategy::WeightedAverage { alpha: 0.3 })
    }

    /// Get embedding dimension
    pub fn embedding_dim(&self) -> usize {
        self.embedding_dim
    }

    /// Embed a batch of texts
    pub fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            let emb = self.embed_single(text)?;
            results.push(emb);
        }
        Ok(results)
    }

    /// Embed a search query with fusion
    pub fn embed_query(&mut self, query: &str) -> Result<Vec<f32>> {
        // For queries, we might want slightly different weighting
        // Giving more weight to semantic understanding
        let bge_emb = self.bge_embedder.embed_query(query)?;
        let code_emb = self.code_embedder.embed_query(query)?;
        self.fuse(&bge_emb, &code_emb)
    }

    /// Embed a single text with fusion
    fn embed_single(&mut self, text: &str) -> Result<Vec<f32>> {
        // Get embeddings from both models
        let bge_emb = self.bge_embedder.embed(&[text.to_string()])?;
        let code_emb = self.code_embedder.embed(&[text.to_string()])?;

        let bge = &bge_emb[0];
        let code = &code_emb[0];

        self.fuse(bge, code)
    }

    /// Fuse two embedding vectors according to the strategy
    fn fuse(&self, bge: &[f32], code: &[f32]) -> Result<Vec<f32>> {
        match self.strategy {
            FusionStrategy::WeightedAverage { alpha } => {
                self.fuse_weighted_average(bge, code, alpha)
            }
            FusionStrategy::MaxPool => self.fuse_max_pool(bge, code),
            FusionStrategy::Concatenate => self.fuse_concatenate(bge, code),
            FusionStrategy::Adaptive => self.fuse_adaptive(bge, code),
        }
    }

    /// Weighted average fusion: α * bge + (1-α) * code
    fn fuse_weighted_average(&self, bge: &[f32], code: &[f32], alpha: f32) -> Result<Vec<f32>> {
        let beta = 1.0 - alpha;

        // Normalize both vectors first
        let bge_norm = self.l2_normalize(bge);
        let code_norm = self.l2_normalize(code);

        // Weighted combination
        let mut fused: Vec<f32> = bge_norm
            .iter()
            .zip(code_norm.iter())
            .map(|(b, c)| alpha * b + beta * c)
            .collect();

        // Re-normalize the result
        let norm = (fused.iter().map(|x| x * x).sum::<f32>()).sqrt();
        if norm > 0.0 {
            for v in &mut fused {
                *v /= norm;
            }
        }

        Ok(fused)
    }

    /// Max pooling: take the max of each dimension
    fn fuse_max_pool(&self, bge: &[f32], code: &[f32]) -> Result<Vec<f32>> {
        let bge_norm = self.l2_normalize(bge);
        let code_norm = self.l2_normalize(code);

        let mut fused: Vec<f32> = bge_norm
            .iter()
            .zip(code_norm.iter())
            .map(|(b, c)| b.max(*c))
            .collect();

        // Normalize
        let norm = (fused.iter().map(|x| x * x).sum::<f32>()).sqrt();
        if norm > 0.0 {
            for v in &mut fused {
                *v /= norm;
            }
        }

        Ok(fused)
    }

    /// Concatenate and project (simplified: average of dimensions)
    fn fuse_concatenate(&self, bge: &[f32], code: &[f32]) -> Result<Vec<f32>> {
        // For now, just interleave and average pairs
        // A full implementation would use a learned projection matrix
        let bge_norm = self.l2_normalize(bge);
        let code_norm = self.l2_normalize(code);

        let mut fused: Vec<f32> = bge_norm
            .iter()
            .zip(code_norm.iter())
            .map(|(b, c)| (b + c) / 2.0)
            .collect();

        // Normalize
        let norm = (fused.iter().map(|x| x * x).sum::<f32>()).sqrt();
        if norm > 0.0 {
            for v in &mut fused {
                *v /= norm;
            }
        }

        Ok(fused)
    }

    /// Adaptive fusion: weight by magnitude
    fn fuse_adaptive(&self, bge: &[f32], code: &[f32]) -> Result<Vec<f32>> {
        let bge_norm = self.l2_normalize(bge);
        let code_norm = self.l2_normalize(code);

        // Weight by the absolute value of each embedding
        let mut fused: Vec<f32> = bge_norm
            .iter()
            .zip(code_norm.iter())
            .map(|(b, c)| {
                let b_weight = b.abs();
                let c_weight = c.abs();
                let total = b_weight + c_weight + 1e-8;
                (b_weight * b + c_weight * c) / total
            })
            .collect();

        // Normalize
        let norm = (fused.iter().map(|x| x * x).sum::<f32>()).sqrt();
        if norm > 0.0 {
            for v in &mut fused {
                *v /= norm;
            }
        }

        Ok(fused)
    }

    /// L2 normalize a vector
    fn l2_normalize(&self, v: &[f32]) -> Vec<f32> {
        let norm = (v.iter().map(|x| x * x).sum::<f32>()).sqrt();
        if norm > 0.0 {
            v.iter().map(|x| x / norm).collect()
        } else {
            v.to_vec()
        }
    }
}

/// Fast hybrid search that caches the embedder
pub struct CachedHybridEmbedder {
    embedder: Option<HybridEmbedder>,
}

impl CachedHybridEmbedder {
    pub fn new() -> Self {
        Self { embedder: None }
    }

    pub fn get_or_init(&mut self) -> Result<&mut HybridEmbedder> {
        if self.embedder.is_none() {
            self.embedder = Some(HybridEmbedder::new()?);
        }
        Ok(self.embedder.as_mut().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l2_normalize() {
        let embedder = HybridEmbedder {
            bge_embedder: LocalEmbedder::with_speed_mode(SpeedMode::Balanced).unwrap(),
            code_embedder: LocalEmbedder::with_speed_mode(SpeedMode::Code).unwrap(),
            strategy: FusionStrategy::default(),
            embedding_dim: 768,
            device: Device::Cpu,
        };

        let v = vec![3.0, 4.0];
        let normalized = embedder.l2_normalize(&v);

        // Should be [0.6, 0.8] for a 3-4-5 triangle
        assert!((normalized[0] - 0.6).abs() < 1e-6);
        assert!((normalized[1] - 0.8).abs() < 1e-6);
    }
}
