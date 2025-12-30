use anyhow::Result;
use colored::Colorize;

use crate::core::store::VectorStore;

pub struct StatusOptions {
    pub store: Option<String>,
    pub files: bool,
}

pub async fn run(options: StatusOptions) -> Result<()> {
    let store = VectorStore::load(options.store.as_deref())?;

    println!("{}", "searchgrep index status".bold());
    println!();

    let file_count = store.file_count();
    let chunk_count = store.chunk_count();

    println!("  {} {}", "Files indexed:".dimmed(), file_count);
    println!("  {} {}", "Total chunks:".dimmed(), chunk_count);

    if store.doc_count > 0 {
        println!("  {} {}", "BM25 terms:".dimmed(), store.bm25_idf.len());
    }

    // Calculate approximate embedding size
    if let Some(chunk) = store.chunks.values().next() {
        let embedding_dim = chunk.embedding.len();
        let total_size = chunk_count * embedding_dim * 4; // 4 bytes per f32
        let size_mb = total_size as f64 / (1024.0 * 1024.0);
        println!(
            "  {} {} ({}D)",
            "Embedding size:".dimmed(),
            format!("{:.2} MB", size_mb),
            embedding_dim
        );
    }

    if options.files {
        println!();
        println!("{}", "Indexed files:".bold());

        let mut files: Vec<_> = store.files.values().collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));

        for file in files {
            let chunk_count = file.chunks.len();
            println!(
                "  {} {} {}",
                file.path,
                format!("({} chunks)", chunk_count).dimmed(),
                file.indexed_at.dimmed()
            );
        }
    }

    if file_count == 0 {
        println!();
        println!("{}", "No files indexed yet. Run:".yellow());
        println!("  searchgrep watch [path]");
    }

    Ok(())
}
