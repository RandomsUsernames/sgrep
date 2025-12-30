use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::core::config::Config;
use crate::core::search::SearchResult;

#[derive(Debug, Serialize)]
struct RerankerRequest {
    model: String,
    query: String,
    documents: Vec<String>,
    top_n: usize,
}

#[derive(Debug, Deserialize)]
struct RerankerResponse {
    results: Vec<RerankerResult>,
}

#[derive(Debug, Deserialize)]
struct RerankerResult {
    index: usize,
    relevance_score: f32,
}

#[derive(Debug, Serialize)]
struct JinaRerankerRequest {
    model: String,
    query: String,
    documents: Vec<String>,
    top_n: usize,
}

#[derive(Debug, Deserialize)]
struct JinaRerankerResponse {
    results: Vec<JinaRerankerResult>,
}

#[derive(Debug, Deserialize)]
struct JinaRerankerResult {
    index: usize,
    relevance_score: f32,
}

pub struct Reranker {
    config: Config,
    client: reqwest::Client,
}

impl Reranker {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    pub async fn rerank(
        &self,
        query: &str,
        results: Vec<SearchResult>,
        top_n: usize,
    ) -> Result<Vec<SearchResult>> {
        if results.is_empty() {
            return Ok(results);
        }

        // Try Jina reranker first (free tier available)
        match self.rerank_jina(query, &results, top_n).await {
            Ok(reranked) => return Ok(reranked),
            Err(_) => {
                // Fall back to Cohere if available
                if let Some(api_key) = std::env::var("COHERE_API_KEY").ok() {
                    match self.rerank_cohere(query, &results, top_n, &api_key).await {
                        Ok(reranked) => return Ok(reranked),
                        Err(_) => {}
                    }
                }
            }
        }

        // If no reranker available, return original results
        Ok(results.into_iter().take(top_n).collect())
    }

    async fn rerank_jina(
        &self,
        query: &str,
        results: &[SearchResult],
        top_n: usize,
    ) -> Result<Vec<SearchResult>> {
        let api_key = std::env::var("JINA_API_KEY").context("JINA_API_KEY not set")?;

        let documents: Vec<String> = results.iter().map(|r| r.chunk.content.clone()).collect();

        let request = JinaRerankerRequest {
            model: "jina-reranker-v2-base-multilingual".to_string(),
            query: query.to_string(),
            documents,
            top_n,
        };

        let response = self
            .client
            .post("https://api.jina.ai/v1/rerank")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send rerank request")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Jina API error {}: {}", status, text));
        }

        let result: JinaRerankerResponse = response
            .json()
            .await
            .context("Failed to parse rerank response")?;

        let mut reranked: Vec<SearchResult> = result
            .results
            .into_iter()
            .filter_map(|r| {
                results.get(r.index).map(|original| {
                    let mut new_result = original.clone();
                    new_result.score = r.relevance_score;
                    new_result
                })
            })
            .collect();

        reranked.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(reranked)
    }

    async fn rerank_cohere(
        &self,
        query: &str,
        results: &[SearchResult],
        top_n: usize,
        api_key: &str,
    ) -> Result<Vec<SearchResult>> {
        let documents: Vec<String> = results.iter().map(|r| r.chunk.content.clone()).collect();

        let request = serde_json::json!({
            "model": "rerank-english-v3.0",
            "query": query,
            "documents": documents,
            "top_n": top_n,
        });

        let response = self
            .client
            .post("https://api.cohere.ai/v1/rerank")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send Cohere rerank request")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Cohere API error {}: {}", status, text));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Cohere response")?;

        let reranked_results = result["results"]
            .as_array()
            .ok_or_else(|| anyhow!("Invalid Cohere response"))?;

        let mut reranked: Vec<SearchResult> = reranked_results
            .iter()
            .filter_map(|r| {
                let index = r["index"].as_u64()? as usize;
                let score = r["relevance_score"].as_f64()? as f32;
                results.get(index).map(|original| {
                    let mut new_result = original.clone();
                    new_result.score = score;
                    new_result
                })
            })
            .collect();

        reranked.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(reranked)
    }
}

// Simple local reranking using keyword overlap
pub fn simple_rerank(query: &str, mut results: Vec<SearchResult>) -> Vec<SearchResult> {
    let query_terms: std::collections::HashSet<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    for result in &mut results {
        let content_lower = result.chunk.content.to_lowercase();
        let mut keyword_boost = 0.0;

        for term in &query_terms {
            if content_lower.contains(term) {
                keyword_boost += 0.05;
            }
        }

        result.score += keyword_boost;
    }

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}
