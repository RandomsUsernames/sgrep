use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;

use crate::core::chunker::CodeChunker;
use crate::core::config::Config;
use crate::core::embeddings::EmbeddingProvider;
use crate::core::local_embeddings::SpeedMode;
use crate::core::scanner::FileScanner;
use crate::core::store::{
    compute_file_hash, generate_chunk_id, FileChunk, IndexedFile, VectorStore,
};

pub struct WatchOptions {
    pub path: Option<String>,
    pub store: Option<String>,
    pub once: bool,
    pub fast: bool,
    pub quality: bool,
    pub code: bool,
}

pub async fn run(options: WatchOptions) -> Result<()> {
    let path = options.path.unwrap_or_else(|| ".".to_string());
    let abs_path = std::fs::canonicalize(&path)?;
    let path_str = abs_path.to_string_lossy().to_string();

    // Determine speed mode from flags (code takes priority)
    let speed_mode = if options.code {
        SpeedMode::Code
    } else if options.fast {
        SpeedMode::Fast
    } else if options.quality {
        SpeedMode::Quality
    } else {
        SpeedMode::Balanced
    };

    println!("{} {}", "Indexing".cyan(), path_str.dimmed());

    // Initial sync
    sync_files(&path_str, options.store.as_deref(), speed_mode).await?;

    if options.once {
        println!("{}", "✓ Indexing complete".green());
        return Ok(());
    }

    // Watch for changes
    println!("{}", "Watching for changes... (Ctrl+C to stop)".dimmed());

    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        },
        NotifyConfig::default().with_poll_interval(Duration::from_secs(2)),
    )?;

    watcher.watch(Path::new(&path_str), RecursiveMode::Recursive)?;

    let store_name = options.store.clone();
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(event) => {
                use notify::EventKind;
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                        for path in event.paths {
                            let path_str = path.to_string_lossy().to_string();
                            println!("{} {}", "Changed:".yellow(), path_str.dimmed());
                        }
                        // Re-sync
                        if let Err(e) =
                            sync_files(&path_str, store_name.as_deref(), speed_mode).await
                        {
                            eprintln!("{} {}", "Error syncing:".red(), e);
                        }
                    }
                    _ => {}
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}

pub async fn sync_files(path: &str, store_name: Option<&str>, speed_mode: SpeedMode) -> Result<()> {
    let config = Config::load()?;
    let mut store = VectorStore::load(store_name)?;
    let embeddings = EmbeddingProvider::with_speed_mode(config, speed_mode);
    let chunker = CodeChunker::default();
    let scanner = FileScanner::new(path);

    let files = scanner.scan()?;

    if files.is_empty() {
        println!("{}", "No files found to index".yellow());
        return Ok(());
    }

    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut indexed_count = 0;
    let mut skipped_count = 0;

    for file in files {
        pb.set_message(truncate_path(&file.path, 40));

        let hash = compute_file_hash(&file.content);

        // Check if file needs updating
        if !store.file_needs_update(&file.path, &hash) {
            skipped_count += 1;
            pb.inc(1);
            continue;
        }

        // Remove old chunks if file exists
        store.remove_file(&file.path);

        // Chunk the file
        let chunks = chunker.chunk(&file.content, file.language.as_deref());

        if chunks.is_empty() {
            pb.inc(1);
            continue;
        }

        // Generate embeddings for all chunks
        let chunk_texts: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();

        let chunk_embeddings = match embeddings.embed(&chunk_texts).await {
            Ok(emb) => emb,
            Err(e) => {
                eprintln!("{} {} - {}", "Error embedding".red(), file.path, e);
                pb.inc(1);
                continue;
            }
        };

        // Store chunks
        let mut chunk_ids = Vec::new();
        for (i, chunk) in chunks.iter().enumerate() {
            let chunk_id = generate_chunk_id(&file.path, chunk.start_line, chunk.end_line);
            chunk_ids.push(chunk_id.clone());

            store.add_chunk(FileChunk {
                id: chunk_id,
                file_path: file.path.clone(),
                content: chunk.content.clone(),
                start_line: chunk.start_line,
                end_line: chunk.end_line,
                chunk_type: chunk.chunk_type.as_str().to_string(),
                language: file.language.clone(),
                embedding: chunk_embeddings[i].clone(),
                token_embeddings: None,
            });
        }

        // Store file metadata
        store.add_file(IndexedFile {
            path: file.path.clone(),
            hash,
            chunks: chunk_ids,
            indexed_at: chrono::Utc::now().to_rfc3339(),
        });

        indexed_count += 1;
        pb.inc(1);
    }

    pb.finish_and_clear();

    // Update BM25 stats
    store.update_bm25_stats();
    store.save(store_name)?;

    println!(
        "{} {} files ({} new, {} unchanged)",
        "✓ Indexed".green(),
        indexed_count + skipped_count,
        indexed_count,
        skipped_count
    );

    Ok(())
}

fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else {
        format!("...{}", &path[path.len() - max_len + 3..])
    }
}
