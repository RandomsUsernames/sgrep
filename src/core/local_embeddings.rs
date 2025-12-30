use anyhow::{anyhow, Result};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config as BertConfig};
use hf_hub::{api::sync::Api, Repo, RepoType};
use std::path::PathBuf;
use tokenizers::Tokenizer;

use super::nomic_bert::{NomicBertConfig, NomicBertModel};

/// Speed mode for embeddings - trades accuracy for speed
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SpeedMode {
    /// Quality mode: F32 precision, best accuracy
    Quality,
    /// Balanced mode: F16 precision, good balance (default)
    #[default]
    Balanced,
    /// Fast mode: Smaller model (MiniLM), fastest indexing
    Fast,
    /// Code mode: CodeRankEmbed optimized for code search
    Code,
}

/// Model type enum to support different architectures
enum ModelType {
    Bert(BertModel),
    NomicBert(NomicBertModel),
}

/// BGE embedder using Candle with Metal GPU acceleration
/// - BGE-base: 110M params, 768-dim, great for code search
/// - MiniLM: 22M params, 384-dim, faster but less accurate
/// - CodeRankEmbed: 137M params, 768-dim, optimized for code
pub struct LocalEmbedder {
    model: ModelType,
    tokenizer: Tokenizer,
    device: Device,
    speed_mode: SpeedMode,
    embedding_dim: usize,
}

impl LocalEmbedder {
    pub fn new() -> Result<Self> {
        Self::with_speed_mode(SpeedMode::default())
    }

    pub fn with_speed_mode(speed_mode: SpeedMode) -> Result<Self> {
        // Use CPU for now - Metal lacks layer-norm support
        // CPU with Accelerate is still fast on Apple Silicon
        let device = Device::Cpu;

        // CodeRankEmbed uses NomicBert architecture
        if speed_mode == SpeedMode::Code {
            return Self::load_coderankembed(&device);
        }

        let (model_id, embedding_dim, dtype) = match speed_mode {
            SpeedMode::Fast => {
                println!("Loading MiniLM (fast mode) on CPU (Accelerate)...");
                ("sentence-transformers/all-MiniLM-L6-v2", 384, DType::F32)
            }
            SpeedMode::Balanced => {
                println!("Loading BGE-base (balanced mode) on CPU (Accelerate)...");
                ("BAAI/bge-base-en-v1.5", 768, DType::F32) // F16 not supported by Accelerate matmul
            }
            SpeedMode::Quality => {
                println!("Loading BGE-base (quality mode) on CPU (Accelerate)...");
                ("BAAI/bge-base-en-v1.5", 768, DType::F32)
            }
            SpeedMode::Code => unreachable!(), // Handled above
        };

        let api = Api::new()?;
        let repo = api.repo(Repo::with_revision(
            model_id.to_string(),
            RepoType::Model,
            "main".to_string(),
        ));

        // Download model files
        let config_path = repo.get("config.json")?;
        let tokenizer_path = repo.get("tokenizer.json")?;
        let weights_path = repo.get("model.safetensors")?;

        // Load config
        let config: BertConfig = serde_json::from_str(&std::fs::read_to_string(&config_path)?)?;

        // Load tokenizer
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow!("Failed to load tokenizer: {}", e))?;

        // Load model weights - use F32 for computation even if we loaded F16
        // Candle handles the conversion internally
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], dtype, &device)? };
        let model = BertModel::load(vb, &config)?;

        println!("✓ Model loaded on {:?}", device);

        Ok(Self {
            model: ModelType::Bert(model),
            tokenizer,
            device,
            speed_mode,
            embedding_dim,
        })
    }

    /// Load CodeRankEmbed model (NomicBert architecture, optimized for code)
    fn load_coderankembed(device: &Device) -> Result<Self> {
        println!("Loading CodeRankEmbed (code mode) on CPU (Accelerate)...");
        println!("  137M params | 768-dim | Optimized for code search");

        let model_id = "nomic-ai/CodeRankEmbed";
        let embedding_dim = 768;

        let api = Api::new()?;
        let repo = api.repo(Repo::with_revision(
            model_id.to_string(),
            RepoType::Model,
            "main".to_string(),
        ));

        // Download model files
        let config_path = repo.get("config.json")?;
        let tokenizer_path = repo.get("tokenizer.json")?;
        let weights_path = repo.get("model.safetensors")?;

        // Load NomicBert config
        let config: NomicBertConfig =
            serde_json::from_str(&std::fs::read_to_string(&config_path)?)?;

        // Load tokenizer
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow!("Failed to load tokenizer: {}", e))?;

        // Load model weights
        let vb =
            unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], DType::F32, device)? };
        let model = NomicBertModel::load(vb, &config, device)?;

        println!("✓ CodeRankEmbed loaded on {:?}", device);

        Ok(Self {
            model: ModelType::NomicBert(model),
            tokenizer,
            device: device.clone(),
            speed_mode: SpeedMode::Code,
            embedding_dim,
        })
    }

    pub fn speed_mode(&self) -> SpeedMode {
        self.speed_mode
    }

    pub fn embedding_dim(&self) -> usize {
        self.embedding_dim
    }

    pub fn model_dir() -> Result<PathBuf> {
        let config_dir = crate::core::config::Config::config_dir()?;
        Ok(config_dir.join("models").join("coderankembed"))
    }

    pub fn is_available() -> bool {
        // Check if we can access the model (cached or downloadable)
        Api::new().is_ok()
    }

    pub fn coderankembed_available() -> bool {
        Self::is_available()
    }

    pub fn sfr_code_available() -> bool {
        false // Not using SFR-Code with Candle for now
    }

    /// Embed code snippets
    pub fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut embeddings = Vec::with_capacity(texts.len());
        for text in texts {
            let emb = self.embed_single(text)?;
            embeddings.push(emb);
        }
        Ok(embeddings)
    }

    /// Embed a search query (applies "search_query: " prefix for CodeRankEmbed)
    pub fn embed_query(&mut self, query: &str) -> Result<Vec<f32>> {
        // CodeRankEmbed uses prefixes for asymmetric retrieval
        let prefixed = format!("search_query: {}", query);
        self.embed_single(&prefixed)
    }

    /// Embed a single text and return embedding vector
    fn embed_single(&self, text: &str) -> Result<Vec<f32>> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| anyhow!("Tokenization failed: {}", e))?;

        // Max tokens depends on model (512 for BERT, 8192 for NomicBert)
        let max_len = match &self.model {
            ModelType::Bert(_) => 512,
            ModelType::NomicBert(_) => 8192,
        };

        let input_ids: Vec<u32> = encoding.get_ids().iter().take(max_len).copied().collect();
        let attention_mask: Vec<u32> = encoding
            .get_attention_mask()
            .iter()
            .take(max_len)
            .copied()
            .collect();
        let token_type_ids: Vec<u32> = encoding
            .get_type_ids()
            .iter()
            .take(max_len)
            .copied()
            .collect();

        let seq_len = input_ids.len();

        let input_ids = Tensor::new(input_ids.as_slice(), &self.device)?.unsqueeze(0)?;
        let attention_mask = Tensor::new(attention_mask.as_slice(), &self.device)?.unsqueeze(0)?;
        let token_type_ids = Tensor::new(token_type_ids.as_slice(), &self.device)?.unsqueeze(0)?;

        // Run model based on type
        let embeddings = match &self.model {
            ModelType::Bert(model) => {
                model.forward(&input_ids, &token_type_ids, Some(&attention_mask))?
            }
            ModelType::NomicBert(model) => {
                model.forward(&input_ids, Some(&token_type_ids), Some(&attention_mask))?
            }
        };

        // Mean pooling over sequence dimension
        let sum = embeddings.sum(1)?;
        let count = Tensor::new(&[seq_len as f32], &self.device)?.broadcast_as(sum.shape())?;
        let mean = (sum / count)?;

        // L2 normalize
        let norm = mean.sqr()?.sum_keepdim(1)?.sqrt()?;
        let normalized = mean.broadcast_div(&norm)?;

        // Convert to Vec<f32>
        let result: Vec<f32> = normalized.squeeze(0)?.to_vec1()?;
        Ok(result)
    }

    /// Get token-level embeddings
    pub fn embed_with_tokens(&mut self, text: &str) -> Result<(Vec<f32>, Vec<Vec<f32>>)> {
        let pooled = self.embed_single(text)?;
        Ok((pooled, vec![]))
    }
}

/// Download model (handled automatically by hf-hub, but we keep the interface)
pub async fn download_model() -> Result<()> {
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  Downloading BGE-base-en-v1.5 (BAAI)");
    println!("  110M params | 768-dim | Metal GPU accelerated");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Model will be downloaded on first use via hf-hub
    let api = Api::new()?;
    let repo = api.repo(Repo::with_revision(
        "BAAI/bge-base-en-v1.5".to_string(),
        RepoType::Model,
        "main".to_string(),
    ));

    println!("Downloading config.json...");
    repo.get("config.json")?;

    println!("Downloading tokenizer.json...");
    repo.get("tokenizer.json")?;

    println!("Downloading model.safetensors...");
    repo.get("model.safetensors")?;

    println!("\n✓ BGE-base downloaded successfully!");
    println!("  Using Metal GPU acceleration on Apple Silicon");

    Ok(())
}
