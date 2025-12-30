//! Fast index command - Ultra-fast codebase indexing

use anyhow::Result;
use colored::Colorize;

use crate::core::fast_indexer::{FastIndexConfig, FastIndexer, IndexTier};

pub struct IndexOptions {
    pub path: Option<String>,
    pub store: Option<String>,
    /// BM25 only - instant indexing, no embeddings
    pub fast: bool,
    /// Balanced mode (default)
    pub balanced: bool,
    /// Full quality embeddings
    pub quality: bool,
    /// Force re-index all files
    pub force: bool,
    /// Number of threads (0 = auto)
    pub threads: usize,
    /// Batch size for embeddings
    pub batch_size: usize,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            path: None,
            store: None,
            fast: false,
            balanced: false,
            quality: false,
            force: false,
            threads: 0,
            batch_size: 50,
        }
    }
}

pub async fn run(options: IndexOptions) -> Result<()> {
    let path = options.path.unwrap_or_else(|| ".".to_string());
    let abs_path = std::fs::canonicalize(&path)?;
    let path_str = abs_path.to_string_lossy().to_string();

    // Determine tier
    let tier = if options.fast {
        IndexTier::Fast
    } else if options.quality {
        IndexTier::Quality
    } else {
        IndexTier::Balanced
    };

    let tier_name = match tier {
        IndexTier::Fast => "fast (BM25 only)",
        IndexTier::Balanced => "balanced",
        IndexTier::Quality => "quality",
    };

    println!(
        "{} {} {}",
        "⚡".yellow(),
        "Indexing".cyan().bold(),
        path_str.dimmed()
    );
    println!("   Mode: {}", tier_name.yellow());

    let config = FastIndexConfig {
        tier,
        batch_size: options.batch_size,
        num_threads: options.threads,
        incremental: !options.force,
        ..Default::default()
    };

    let indexer = FastIndexer::new(config)?;
    let result = indexer.index(&path_str, options.store.as_deref()).await?;

    // Display results
    println!();
    if result.indexed_files > 0 || result.skipped_files > 0 {
        println!(
            "{} Indexed {} files in {:.1}s",
            "✓".green().bold(),
            result.total_files.to_string().cyan(),
            result.duration_ms as f64 / 1000.0
        );

        println!(
            "   {} new, {} unchanged, {} chunks",
            result.indexed_files.to_string().green(),
            result.skipped_files.to_string().dimmed(),
            result.total_chunks.to_string().cyan()
        );

        // Show speed stats
        if result.duration_ms > 0 && result.indexed_files > 0 {
            let files_per_sec = (result.indexed_files as f64 * 1000.0) / result.duration_ms as f64;
            let chunks_per_sec = (result.total_chunks as f64 * 1000.0) / result.duration_ms as f64;
            println!(
                "   {} files/s, {} chunks/s",
                format!("{:.0}", files_per_sec).green(),
                format!("{:.0}", chunks_per_sec).green()
            );
        }

        // Suggest upgrade if using fast mode
        if tier == IndexTier::Fast {
            println!();
            println!(
                "   {} Run {} for semantic search",
                "💡".yellow(),
                "searchgrep index --balanced".cyan()
            );
        }
    } else {
        println!("{} No files found to index", "⚠".yellow());
    }

    Ok(())
}
