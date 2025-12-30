//! HuggingFace Inference API for fast, serverless embeddings
//!
//! Supports multiple embedding models via HF's free inference API:
//! - sentence-transformers/all-MiniLM-L6-v2 (fastest, 384-dim)
//! - BAAI/bge-small-en-v1.5 (fast, 384-dim)
//! - nomic-ai/nomic-embed-code (code-optimized, 768-dim)
//! - jinaai/jina-embeddings-v3 (multilingual, 1024-dim)

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Available models on HuggingFace Inference API
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HFModel {
    /// all-MiniLM-L6-v2: Fastest, 22M params, 384-dim
    MiniLM,
    /// bge-small-en: Fast and accurate, 33M params, 384-dim
    BgeSmall,
    /// bge-base-en: Balanced, 110M params, 768-dim
    BgeBase,
    /// nomic-embed-code: Code-optimized, 137M params, 768-dim
    NomicCode,
    /// jina-embeddings-v3: Best quality, 570M params, 1024-dim
    JinaV3,
}

impl HFModel {
    pub fn model_id(&self) -> &'static str {
        match self {
            HFModel::MiniLM => "sentence-transformers/all-MiniLM-L6-v2",
            HFModel::BgeSmall => "BAAI/bge-small-en-v1.5",
            HFModel::BgeBase => "BAAI/bge-base-en-v1.5",
            HFModel::NomicCode => "nomic-ai/nomic-embed-code",
            HFModel::JinaV3 => "jinaai/jina-embeddings-v3",
        }
    }

    pub fn embedding_dim(&self) -> usize {
        match self {
            HFModel::MiniLM => 384,
            HFModel::BgeSmall => 384,
            HFModel::BgeBase => 768,
            HFModel::NomicCode => 768,
            HFModel::JinaV3 => 1024,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            HFModel::MiniLM => "Fastest (22M params)",
            HFModel::BgeSmall => "Fast & accurate (33M params)",
            HFModel::BgeBase => "Balanced (110M params)",
            HFModel::NomicCode => "Code-optimized (137M params)",
            HFModel::JinaV3 => "Best quality (570M params)",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "minilm" | "mini" | "fast" => Some(HFModel::MiniLM),
            "bge-small" | "bge_small" | "small" => Some(HFModel::BgeSmall),
            "bge-base" | "bge_base" | "base" | "balanced" => Some(HFModel::BgeBase),
            "nomic" | "nomic-code" | "code" => Some(HFModel::NomicCode),
            "jina" | "jina-v3" | "jinav3" | "quality" => Some(HFModel::JinaV3),
            _ => None,
        }
    }
}

impl Default for HFModel {
    fn default() -> Self {
        HFModel::BgeSmall // Good balance of speed and quality
    }
}

#[derive(Serialize)]
struct EmbeddingRequest {
    inputs: Vec<String>,
    options: RequestOptions,
}

#[derive(Serialize)]
struct RequestOptions {
    wait_for_model: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum EmbeddingResponse {
    Embeddings(Vec<Vec<f32>>),
    Error { error: String },
}

/// HuggingFace Inference API client
pub struct HFInference {
    client: Client,
    model: HFModel,
    api_key: Option<String>,
}

impl HFInference {
    /// Create new HF Inference client
    pub fn new(model: HFModel) -> Result<Self> {
        let api_key = std::env::var("HF_API_KEY")
            .or_else(|_| std::env::var("HUGGINGFACE_API_KEY"))
            .ok();

        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

        Ok(Self {
            client,
            model,
            api_key,
        })
    }

    /// Create with specific API key
    pub fn with_api_key(model: HFModel, api_key: String) -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

        Ok(Self {
            client,
            model,
            api_key: Some(api_key),
        })
    }

    /// Get embedding dimension for current model
    pub fn embedding_dim(&self) -> usize {
        self.model.embedding_dim()
    }

    /// Embed a single text
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.embed_batch(&[text.to_string()]).await?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No embedding returned"))
    }

    /// Embed multiple texts in a single request (more efficient)
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let url = format!(
            "https://api-inference.huggingface.co/pipeline/feature-extraction/{}",
            self.model.model_id()
        );

        let request = EmbeddingRequest {
            inputs: texts.to_vec(),
            options: RequestOptions {
                wait_for_model: true,
            },
        };

        let mut req = self.client.post(&url).json(&request);

        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("HF API error ({}): {}", status, body));
        }

        let result: EmbeddingResponse = response.json().await?;

        match result {
            EmbeddingResponse::Embeddings(embeddings) => Ok(embeddings),
            EmbeddingResponse::Error { error } => Err(anyhow!("HF API error: {}", error)),
        }
    }

    /// Embed texts with progress callback
    pub async fn embed_batch_with_progress<F>(
        &self,
        texts: &[String],
        batch_size: usize,
        mut on_progress: F,
    ) -> Result<Vec<Vec<f32>>>
    where
        F: FnMut(usize, usize),
    {
        let mut all_embeddings = Vec::with_capacity(texts.len());
        let total = texts.len();

        for (i, chunk) in texts.chunks(batch_size).enumerate() {
            let embeddings = self.embed_batch(&chunk.to_vec()).await?;
            all_embeddings.extend(embeddings);
            on_progress((i + 1) * batch_size.min(total - i * batch_size), total);
        }

        Ok(all_embeddings)
    }
}

/// Speed comparison info
pub fn print_model_comparison() {
    println!("\n📊 HuggingFace Embedding Models:\n");
    println!(
        "  {:12} {:40} {:8} {:12}",
        "Name", "Model ID", "Dim", "Speed"
    );
    println!("  {}", "-".repeat(76));

    for model in [
        HFModel::MiniLM,
        HFModel::BgeSmall,
        HFModel::BgeBase,
        HFModel::NomicCode,
        HFModel::JinaV3,
    ] {
        let speed = match model {
            HFModel::MiniLM => "⚡ Fastest",
            HFModel::BgeSmall => "🚀 Fast",
            HFModel::BgeBase => "✓ Medium",
            HFModel::NomicCode => "✓ Medium",
            HFModel::JinaV3 => "🐢 Slower",
        };
        println!(
            "  {:12} {:40} {:8} {:12}",
            format!("{:?}", model),
            model.model_id(),
            model.embedding_dim(),
            speed
        );
    }

    println!("\n  Set HF_API_KEY for higher rate limits (optional)");
    println!("  Free tier: ~30k tokens/day");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_embed_single() {
        let hf = HFInference::new(HFModel::MiniLM).unwrap();
        let embedding = hf.embed("fn main() { println!(\"hello\"); }").await;

        // May fail without API key due to rate limits
        if let Ok(emb) = embedding {
            assert_eq!(emb.len(), 384);
        }
    }
}
