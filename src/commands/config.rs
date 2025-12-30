use anyhow::Result;
use colored::Colorize;

use crate::core::config::Config;
use crate::core::local_embeddings::{download_model, LocalEmbedder};
use crate::core::store::VectorStore;

pub struct ConfigOptions {
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub provider: Option<String>,
    pub local_url: Option<String>,
    pub show: bool,
    pub clear: bool,
    pub download_model: bool,
}

pub async fn run(options: ConfigOptions) -> Result<()> {
    let mut config = Config::load()?;

    if options.download_model {
        println!(
            "{}",
            "Downloading dual embedding models (CodeRankEmbed + SFR-Code)...".cyan()
        );
        println!("  • CodeRankEmbed: 137M params, 768-dim (code retrieval SOTA)");
        println!("  • SFR-Code: 400M params, 1024-dim (Salesforce 2025)");
        println!();
        download_model().await?;
        println!();
        println!("{}", "✓ Models downloaded successfully".green());
        println!(
            "  Run: {} to use local embeddings",
            "searchgrep config --provider local".yellow()
        );
        return Ok(());
    }

    if options.clear {
        let mut store = VectorStore::load(None)?;
        store.clear();
        store.save(None)?;
        println!("{}", "✓ Cleared all indexed files".green());
        return Ok(());
    }

    if options.show {
        show_config(&config);
        return Ok(());
    }

    let mut updated = false;

    if let Some(key) = options.api_key {
        config.api_key = Some(key);
        updated = true;
        println!("{}", "✓ API key updated".green());
    }

    if let Some(model) = options.model {
        config.model = model;
        updated = true;
        println!("{}", "✓ Model updated".green());
    }

    if let Some(url) = options.base_url {
        config.base_url = Some(url);
        updated = true;
        println!("{}", "✓ Base URL updated".green());
    }

    if let Some(provider) = options.provider {
        if provider != "openai" && provider != "local" {
            println!("{}", "Error: provider must be 'openai' or 'local'".red());
            println!("  • openai: Use OpenAI API for embeddings");
            println!("  • local: Use CodeRankEmbed + SFR-Code dual models");
            return Ok(());
        }
        if provider == "local" && !LocalEmbedder::is_available() {
            println!(
                "{}",
                "Warning: Local models not found. Run: searchgrep config --download-model".yellow()
            );
        }
        config.provider = provider;
        updated = true;
        println!("{}", "✓ Provider updated".green());
    }

    if let Some(url) = options.local_url {
        config.local_url = Some(url);
        updated = true;
        println!("{}", "✓ Local URL updated".green());
    }

    if updated {
        config.save()?;
    } else {
        show_config(&config);
    }

    Ok(())
}

fn show_config(config: &Config) {
    println!("{}", "searchgrep configuration".bold());
    println!();

    println!("  {} {}", "Provider:".dimmed(), config.provider);

    if config.provider == "openai" {
        println!("  {} {}", "Model:".dimmed(), config.model);

        if let Some(ref key) = config.api_key {
            let masked = if key.len() > 8 {
                format!("{}...{}", &key[..4], &key[key.len() - 4..])
            } else {
                "****".to_string()
            };
            println!("  {} {}", "API Key:".dimmed(), masked);
        } else {
            println!("  {} {}", "API Key:".dimmed(), "(not set)".yellow());
        }

        if let Some(ref url) = config.base_url {
            println!("  {} {}", "Base URL:".dimmed(), url);
        }
    }

    println!();
    println!("{}", "Local embedding models (dual-model system):".bold());

    let coderankembed_available = LocalEmbedder::coderankembed_available();
    let sfr_code_available = LocalEmbedder::sfr_code_available();

    if coderankembed_available {
        println!(
            "  {} {} {}",
            "CodeRankEmbed:".dimmed(),
            "installed".green(),
            "(768-dim, 8192 ctx)".dimmed()
        );
    } else {
        println!(
            "  {} {}",
            "CodeRankEmbed:".dimmed(),
            "not downloaded".yellow()
        );
    }

    if sfr_code_available {
        println!(
            "  {} {} {}",
            "SFR-Code:".dimmed(),
            "installed".green(),
            "(1024-dim, 8192 ctx, Salesforce 2025)".dimmed()
        );
    } else {
        println!("  {} {}", "SFR-Code:".dimmed(), "not downloaded".yellow());
    }

    if coderankembed_available && sfr_code_available {
        println!();
        println!(
            "  {} {}",
            "Combined embedding:".dimmed(),
            "1792-dim (768 + 1024)".green()
        );
    } else if !coderankembed_available && !sfr_code_available {
        println!();
        println!("  {}", "Run: searchgrep config --download-model".yellow());
    }

    println!();
    println!("{}", "Environment variables:".dimmed());

    if std::env::var("OPENAI_API_KEY").is_ok() {
        println!("  {} set", "OPENAI_API_KEY:".dimmed());
    }
    if std::env::var("JINA_API_KEY").is_ok() {
        println!("  {} set (for reranking)", "JINA_API_KEY:".dimmed());
    }
    if std::env::var("COHERE_API_KEY").is_ok() {
        println!("  {} set (for reranking)", "COHERE_API_KEY:".dimmed());
    }
}
