use anyhow::{anyhow, Result};
use candle_core::{Device, IndexOp, Module, Tensor, D};
use candle_nn::{embedding, layer_norm, linear_no_bias, Embedding, LayerNorm, Linear, VarBuilder};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct NomicBertConfig {
    pub vocab_size: usize,
    pub n_embd: usize,
    pub n_layer: usize,
    pub n_head: usize,
    pub n_inner: usize,
    pub n_positions: usize,
    pub layer_norm_epsilon: f64,
    pub rotary_emb_base: f32,
    pub rotary_emb_fraction: f32,
    #[serde(default)]
    pub type_vocab_size: usize,
}

impl Default for NomicBertConfig {
    fn default() -> Self {
        Self {
            vocab_size: 30528,
            n_embd: 768,
            n_layer: 12,
            n_head: 12,
            n_inner: 3072,
            n_positions: 8192,
            layer_norm_epsilon: 1e-12,
            rotary_emb_base: 1000.0,
            rotary_emb_fraction: 1.0,
            type_vocab_size: 2,
        }
    }
}

// Rotary Position Embedding
struct RotaryEmbedding {
    cos: Tensor,
    sin: Tensor,
    dim: usize,
}

impl RotaryEmbedding {
    fn new(dim: usize, max_seq_len: usize, base: f32, device: &Device) -> Result<Self> {
        let inv_freq: Vec<f32> = (0..dim)
            .step_by(2)
            .map(|i| 1.0 / base.powf(i as f32 / dim as f32))
            .collect();
        let inv_freq = Tensor::new(inv_freq.as_slice(), device)?;

        let positions: Vec<f32> = (0..max_seq_len).map(|i| i as f32).collect();
        let positions = Tensor::new(positions.as_slice(), device)?;

        // [seq_len, dim/2]
        let freqs = positions.unsqueeze(1)?.matmul(&inv_freq.unsqueeze(0)?)?;

        let cos = freqs.cos()?;
        let sin = freqs.sin()?;

        Ok(Self { cos, sin, dim })
    }

    fn apply(&self, x: &Tensor, seq_len: usize) -> Result<Tensor> {
        let (batch, heads, seq, head_dim) = x.dims4()?;
        let rotary_dim = self.dim;

        if rotary_dim > head_dim {
            return Err(anyhow!("Rotary dim {} > head dim {}", rotary_dim, head_dim));
        }

        // Split into rotary and non-rotary parts
        let x_rot = x.narrow(D::Minus1, 0, rotary_dim)?;
        let x_pass = if rotary_dim < head_dim {
            Some(x.narrow(D::Minus1, rotary_dim, head_dim - rotary_dim)?)
        } else {
            None
        };

        // Get cos/sin for current sequence length
        let cos = self.cos.i(..seq_len)?.unsqueeze(0)?.unsqueeze(0)?;
        let sin = self.sin.i(..seq_len)?.unsqueeze(0)?.unsqueeze(0)?;

        // Split x_rot into two halves for rotation
        let half = rotary_dim / 2;
        let x1 = x_rot.narrow(D::Minus1, 0, half)?;
        let x2 = x_rot.narrow(D::Minus1, half, half)?;

        // Apply rotation: [x1, x2] -> [x1*cos - x2*sin, x1*sin + x2*cos]
        let cos = cos.broadcast_as((batch, heads, seq, half))?;
        let sin = sin.broadcast_as((batch, heads, seq, half))?;

        let rotated_x1 = (x1.broadcast_mul(&cos)? - x2.broadcast_mul(&sin)?)?;
        let rotated_x2 = (x1.broadcast_mul(&sin)? + x2.broadcast_mul(&cos)?)?;

        let rotated = Tensor::cat(&[rotated_x1, rotated_x2], D::Minus1)?;

        // Concatenate with non-rotary part if exists
        match x_pass {
            Some(pass) => Ok(Tensor::cat(&[rotated, pass], D::Minus1)?),
            None => Ok(rotated),
        }
    }
}

// SwiGLU activation MLP with separate gate/value projections (fc11, fc12, fc2)
struct NomicMLP {
    fc11: Linear, // gate projection
    fc12: Linear, // value projection
    fc2: Linear,  // output projection
}

impl NomicMLP {
    fn new(config: &NomicBertConfig, vb: VarBuilder) -> Result<Self> {
        // CodeRankEmbed uses fc11 (gate) and fc12 (value) separately
        let fc11 = linear_no_bias(config.n_embd, config.n_inner, vb.pp("fc11"))?;
        let fc12 = linear_no_bias(config.n_embd, config.n_inner, vb.pp("fc12"))?;
        let fc2 = linear_no_bias(config.n_inner, config.n_embd, vb.pp("fc2"))?;
        Ok(Self { fc11, fc12, fc2 })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        // SwiGLU: silu(fc11(x)) * fc12(x)
        let gate = candle_nn::ops::silu(&self.fc11.forward(x)?)?;
        let value = self.fc12.forward(x)?;
        let hidden = gate.mul(&value)?;
        Ok(self.fc2.forward(&hidden)?)
    }
}

// Self-attention with rotary embeddings
struct NomicAttention {
    qkv: Linear,
    out_proj: Linear,
    rotary: RotaryEmbedding,
    n_head: usize,
    head_dim: usize,
}

impl NomicAttention {
    fn new(config: &NomicBertConfig, vb: VarBuilder, device: &Device) -> Result<Self> {
        let head_dim = config.n_embd / config.n_head;
        let rotary_dim = (head_dim as f32 * config.rotary_emb_fraction) as usize;

        let qkv = linear_no_bias(config.n_embd, config.n_embd * 3, vb.pp("Wqkv"))?;
        let out_proj = linear_no_bias(config.n_embd, config.n_embd, vb.pp("out_proj"))?;
        let rotary = RotaryEmbedding::new(
            rotary_dim,
            config.n_positions,
            config.rotary_emb_base,
            device,
        )?;

        Ok(Self {
            qkv,
            out_proj,
            rotary,
            n_head: config.n_head,
            head_dim,
        })
    }

    fn forward(&self, x: &Tensor, attention_mask: Option<&Tensor>) -> Result<Tensor> {
        let (batch, seq_len, _hidden) = x.dims3()?;

        // QKV projection
        let qkv = self.qkv.forward(x)?;
        let qkv = qkv.reshape((batch, seq_len, 3, self.n_head, self.head_dim))?;
        let qkv = qkv.permute((2, 0, 3, 1, 4))?; // [3, batch, heads, seq, head_dim]

        let q = qkv.i(0)?.contiguous()?;
        let k = qkv.i(1)?.contiguous()?;
        let v = qkv.i(2)?.contiguous()?;

        // Apply rotary embeddings
        let q = self.rotary.apply(&q, seq_len)?;
        let k = self.rotary.apply(&k, seq_len)?;

        // Attention scores
        let scale = (self.head_dim as f64).sqrt();
        let attn_weights = q.matmul(&k.transpose(D::Minus2, D::Minus1)?)?;
        let attn_weights = (attn_weights / scale)?;

        // Apply attention mask if provided
        // Input mask is [batch, seq_len] with 1s for tokens to attend to
        // We need to convert to [batch, 1, 1, seq_len] and broadcast
        let attn_weights = match attention_mask {
            Some(mask) => {
                // mask shape: [batch, seq_len] -> [batch, 1, 1, seq_len]
                let mask = mask.unsqueeze(1)?.unsqueeze(1)?;
                // Convert: 1 -> 0, 0 -> -inf for masked positions
                let mask = ((1.0 - mask.to_dtype(attn_weights.dtype())?)? * (-1e9))?;
                // Broadcast addition: [batch, heads, seq, seq] + [batch, 1, 1, seq]
                attn_weights.broadcast_add(&mask)?
            }
            None => attn_weights,
        };

        let attn_weights = candle_nn::ops::softmax(&attn_weights, D::Minus1)?;

        // Apply attention to values
        let attn_output = attn_weights.matmul(&v)?;

        // Reshape back
        let attn_output = attn_output.transpose(1, 2)?;
        let attn_output = attn_output.reshape((batch, seq_len, self.n_head * self.head_dim))?;

        Ok(self.out_proj.forward(&attn_output)?)
    }
}

// Transformer block (post-norm architecture based on config prenorm=false)
struct NomicBlock {
    attn: NomicAttention,
    mlp: NomicMLP,
    norm1: LayerNorm,
    norm2: LayerNorm,
}

impl NomicBlock {
    fn new(config: &NomicBertConfig, vb: VarBuilder, device: &Device) -> Result<Self> {
        let attn = NomicAttention::new(config, vb.pp("attn"), device)?;
        let mlp = NomicMLP::new(config, vb.pp("mlp"))?;
        // Use layer_norm which includes bias by default
        let norm1 = layer_norm(config.n_embd, config.layer_norm_epsilon, vb.pp("norm1"))?;
        let norm2 = layer_norm(config.n_embd, config.layer_norm_epsilon, vb.pp("norm2"))?;
        Ok(Self {
            attn,
            mlp,
            norm1,
            norm2,
        })
    }

    fn forward(&self, x: &Tensor, attention_mask: Option<&Tensor>) -> Result<Tensor> {
        // Post-norm architecture (prenorm=false in config)
        let residual = x;
        let x = self.attn.forward(x, attention_mask)?;
        let x = self.norm1.forward(&(residual + x)?)?;

        let residual = &x;
        let x = self.mlp.forward(&x)?;
        Ok(self.norm2.forward(&(residual + x)?)?)
    }
}

// Main NomicBert model
pub struct NomicBertModel {
    embeddings: Embedding,
    token_type_embeddings: Option<Embedding>,
    emb_ln: LayerNorm,
    blocks: Vec<NomicBlock>,
}

impl NomicBertModel {
    pub fn load(vb: VarBuilder, config: &NomicBertConfig, device: &Device) -> Result<Self> {
        let embeddings = embedding(
            config.vocab_size,
            config.n_embd,
            vb.pp("embeddings.word_embeddings"),
        )?;

        let token_type_embeddings = if config.type_vocab_size > 0 {
            Some(embedding(
                config.type_vocab_size,
                config.n_embd,
                vb.pp("embeddings.token_type_embeddings"),
            )?)
        } else {
            None
        };

        let emb_ln = layer_norm(config.n_embd, config.layer_norm_epsilon, vb.pp("emb_ln"))?;

        // CodeRankEmbed uses "encoder.layers.X" not "transformer.layers.X"
        let mut blocks = Vec::with_capacity(config.n_layer);
        for i in 0..config.n_layer {
            let block = NomicBlock::new(config, vb.pp(format!("encoder.layers.{}", i)), device)?;
            blocks.push(block);
        }

        Ok(Self {
            embeddings,
            token_type_embeddings,
            emb_ln,
            blocks,
        })
    }

    pub fn forward(
        &self,
        input_ids: &Tensor,
        token_type_ids: Option<&Tensor>,
        attention_mask: Option<&Tensor>,
    ) -> Result<Tensor> {
        let mut hidden = self.embeddings.forward(input_ids)?;

        // Add token type embeddings if available
        if let (Some(tte), Some(tti)) = (&self.token_type_embeddings, token_type_ids) {
            hidden = (hidden + tte.forward(tti)?)?;
        }

        hidden = self.emb_ln.forward(&hidden)?;

        for block in &self.blocks {
            hidden = block.forward(&hidden, attention_mask)?;
        }

        Ok(hidden)
    }
}
