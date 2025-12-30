use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

use crate::core::config::Config;
use crate::core::local_embeddings::{LocalEmbedder, SpeedMode};

#[derive(Debug, Serialize)]
struct OpenAIEmbeddingRequest {
    input: Vec<String>,
    model: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

#[derive(Debug, Serialize)]
struct OllamaEmbeddingRequest {
    model: String,
    prompt: String,
}

#[derive(Debug, Deserialize)]
struct OllamaEmbeddingResponse {
    embedding: Vec<f32>,
}

pub struct EmbeddingProvider {
    config: Config,
    client: reqwest::Client,
    local_embedder: Option<Mutex<LocalEmbedder>>,
}

impl EmbeddingProvider {
    pub fn new(config: Config) -> Self {
        Self::with_speed_mode(config, SpeedMode::default())
    }

    pub fn with_speed_mode(config: Config, speed_mode: SpeedMode) -> Self {
        // Try to load dual local embedder (CodeRankEmbed) if provider is "local"
        let local_embedder = if config.provider == "local" {
            match LocalEmbedder::with_speed_mode(speed_mode) {
                Ok(embedder) => Some(Mutex::new(embedder)),
                Err(e) => {
                    eprintln!("Failed to load local embedder: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            config,
            client: reqwest::Client::new(),
            local_embedder,
        }
    }

    pub async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if self.config.provider == "local" {
            self.embed_local_model(texts)
        } else {
            self.embed_openai(texts).await
        }
    }

    fn embed_local_model(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut embedder = self
            .local_embedder
            .as_ref()
            .ok_or_else(|| {
                anyhow!("Local models not loaded. Run: searchgrep config --download-model")
            })?
            .lock()
            .map_err(|e| anyhow!("Failed to lock embedder: {}", e))?;
        embedder.embed(texts)
    }

    pub async fn embed_single(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.embed(&[text.to_string()]).await?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No embedding returned"))
    }

    /// Embed a search query (applies query prefix for CodeRankEmbed)
    pub async fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        if self.config.provider == "local" {
            let mut embedder = self
                .local_embedder
                .as_ref()
                .ok_or_else(|| {
                    anyhow!("Local models not loaded. Run: searchgrep config --download-model")
                })?
                .lock()
                .map_err(|e| anyhow!("Failed to lock embedder: {}", e))?;
            embedder.embed_query(query)
        } else {
            self.embed_single(query).await
        }
    }

    async fn embed_openai(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let api_key = self
            .config
            .get_api_key()
            .context("No API key configured. Run: searchgrep config --api-key YOUR_KEY")?;

        let base_url = self.config.get_base_url();
        let url = format!("{}/embeddings", base_url);

        let request = OpenAIEmbeddingRequest {
            input: texts.to_vec(),
            model: self.config.model.clone(),
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send embedding request")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("OpenAI API error {}: {}", status, text));
        }

        let result: OpenAIEmbeddingResponse = response
            .json()
            .await
            .context("Failed to parse embedding response")?;

        Ok(result.data.into_iter().map(|d| d.embedding).collect())
    }

    pub async fn embed_with_tokens(&self, text: &str) -> Result<(Vec<f32>, Vec<Vec<f32>>)> {
        if self.config.provider == "local" {
            let mut embedder = self
                .local_embedder
                .as_ref()
                .ok_or_else(|| {
                    anyhow!("Local models not loaded. Run: searchgrep config --download-model")
                })?
                .lock()
                .map_err(|e| anyhow!("Failed to lock embedder: {}", e))?;
            embedder.embed_with_tokens(text)
        } else {
            // For API-based providers, return empty token embeddings
            let embedding = self.embed_single(text).await?;
            Ok((embedding, vec![]))
        }
    }
}

// Vector operations
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

#[allow(dead_code)]
pub fn normalize(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

// ColBERT-style max similarity
pub fn colbert_max_sim(query_tokens: &[Vec<f32>], doc_tokens: &[Vec<f32>]) -> f32 {
    if query_tokens.is_empty() || doc_tokens.is_empty() {
        return 0.0;
    }

    let mut total_sim = 0.0;

    for q_tok in query_tokens {
        let max_sim = doc_tokens
            .iter()
            .map(|d_tok| cosine_similarity(q_tok, d_tok))
            .fold(f32::NEG_INFINITY, f32::max);
        total_sim += max_sim;
    }

    total_sim / query_tokens.len() as f32
}
