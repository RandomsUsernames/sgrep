use anyhow::Result;
use colored::Colorize;

use crate::core::graph::KnowledgeGraph;
use crate::core::store::VectorStore;

pub struct GraphOptions {
    /// File path to show graph for
    pub file: Option<String>,
    /// Show imports only
    pub imports: bool,
    /// Show importers only
    pub importers: bool,
    /// Depth of traversal
    pub depth: usize,
    /// Use alternative store name
    pub store: Option<String>,
    /// Show statistics only
    pub stats: bool,
    /// Output as JSON
    pub json: bool,
}

pub async fn run(options: GraphOptions) -> Result<()> {
    // Load just the knowledge graph (fast - skips ANN index building)
    let graph = VectorStore::load_graph_only(options.store.as_deref())?;

    if graph.files.is_empty() && graph.commits.is_empty() {
        if options.json {
            println!(
                "{}",
                serde_json::json!({
                    "error": "No index found",
                    "hint": "Index files first with: sgrep index ."
                })
            );
        } else {
            println!("{}", "No index found.".yellow());
            println!("Index files first with: {}", "sgrep index .".cyan());
        }
        return Ok(());
    }

    if options.stats {
        return show_stats(&graph, options.json);
    }

    if let Some(file_path) = &options.file {
        show_file_graph(&graph, file_path, &options)?;
    } else {
        show_overview(&graph, options.json)?;
    }

    Ok(())
}

fn show_stats(graph: &KnowledgeGraph, json: bool) -> Result<()> {
    let stats = graph.stats();

    if json {
        println!(
            "{}",
            serde_json::json!({
                "repos": stats.repos,
                "files": stats.files,
                "commits": stats.commits,
                "symbols": stats.symbols,
                "edges": stats.edges
            })
        );
    } else {
        println!("{}", "Knowledge Graph Statistics".cyan().bold());
        println!();
        println!("  {} {}", "Repositories:".dimmed(), stats.repos);
        println!("  {} {}", "Files:       ".dimmed(), stats.files);
        println!("  {} {}", "Commits:     ".dimmed(), stats.commits);
        println!("  {} {}", "Symbols:     ".dimmed(), stats.symbols);
        println!("  {} {}", "Edges:       ".dimmed(), stats.edges);
    }

    Ok(())
}

fn show_overview(graph: &KnowledgeGraph, json: bool) -> Result<()> {
    if graph.files.is_empty() {
        if json {
            println!(
                "{}",
                serde_json::json!({
                    "error": "No files in knowledge graph",
                    "hint": "Index files first with: sgrep index ."
                })
            );
        } else {
            println!("{}", "No files in knowledge graph.".yellow());
            println!("Index files first with: {}", "sgrep index .".cyan());
        }
        return Ok(());
    }

    let stats = graph.stats();

    if json {
        let files_with_imports: Vec<_> = graph
            .files
            .values()
            .filter(|f| !f.imports.is_empty())
            .map(|f| {
                serde_json::json!({
                    "path": f.path,
                    "imports": f.imports.len(),
                    "imported_by": f.imported_by.len()
                })
            })
            .collect();

        println!(
            "{}",
            serde_json::json!({
                "stats": {
                    "repos": stats.repos,
                    "files": stats.files,
                    "commits": stats.commits,
                    "symbols": stats.symbols,
                    "edges": stats.edges
                },
                "files_with_relationships": files_with_imports
            })
        );
    } else {
        println!("{}", "Knowledge Graph Overview".cyan().bold());
        println!();
        println!(
            "  {} {} files, {} commits, {} edges",
            "Stats:".dimmed(),
            stats.files,
            stats.commits,
            stats.edges
        );
        println!();

        // Show files with most imports
        let mut files_by_imports: Vec<_> = graph.files.values().collect();
        files_by_imports.sort_by(|a, b| b.imports.len().cmp(&a.imports.len()));

        if files_by_imports.iter().any(|f| !f.imports.is_empty()) {
            println!("{}", "Most connected files:".dimmed());
            for file in files_by_imports.iter().take(10) {
                if file.imports.is_empty() && file.imported_by.is_empty() {
                    continue;
                }
                println!(
                    "  {} {} {} imports, {} importers",
                    "→".cyan(),
                    file.path,
                    file.imports.len(),
                    file.imported_by.len()
                );
            }
        }
    }

    Ok(())
}

fn show_file_graph(graph: &KnowledgeGraph, file_path: &str, options: &GraphOptions) -> Result<()> {
    // Find the file in the graph
    let file_node = graph
        .files
        .values()
        .find(|f| f.path == file_path || f.path.ends_with(file_path) || f.id.ends_with(file_path));

    let file_node = match file_node {
        Some(f) => f,
        None => {
            if options.json {
                println!(
                    "{}",
                    serde_json::json!({
                        "error": format!("File not found in graph: {}", file_path)
                    })
                );
            } else {
                println!("{} File not found in graph: {}", "Error:".red(), file_path);
                println!("Make sure the file is indexed.");
            }
            return Ok(());
        }
    };

    if options.json {
        let imports: Vec<_> = file_node
            .imports
            .iter()
            .filter_map(|id| graph.files.get(id))
            .map(|f| &f.path)
            .collect();

        let importers: Vec<_> = file_node
            .imported_by
            .iter()
            .filter_map(|id| graph.files.get(id))
            .map(|f| &f.path)
            .collect();

        let related = graph.get_related_files(&file_node.id, options.depth);
        let related_paths: Vec<_> = related.iter().map(|f| &f.path).collect();

        println!(
            "{}",
            serde_json::json!({
                "file": file_node.path,
                "imports": imports,
                "imported_by": importers,
                "related": related_paths,
                "symbols": file_node.symbols
            })
        );
    } else {
        println!(
            "{} {}",
            "File:".cyan().bold(),
            file_node.path.white().bold()
        );
        println!();

        // Show imports
        if !options.importers {
            println!("{}", "Imports:".cyan());
            if file_node.imports.is_empty() {
                println!("  {}", "(none)".dimmed());
            } else {
                for import_id in &file_node.imports {
                    if let Some(imported) = graph.files.get(import_id) {
                        println!("  {} {}", "→".green(), imported.path);
                    } else {
                        // Show raw import path
                        let path = if let Some(idx) = import_id.find(':') {
                            &import_id[idx + 1..]
                        } else {
                            import_id
                        };
                        println!("  {} {} {}", "→".green(), path, "(external)".dimmed());
                    }
                }
            }
            println!();
        }

        // Show importers
        if !options.imports {
            println!("{}", "Imported by:".cyan());
            if file_node.imported_by.is_empty() {
                println!("  {}", "(none)".dimmed());
            } else {
                for importer_id in &file_node.imported_by {
                    if let Some(importer) = graph.files.get(importer_id) {
                        println!("  {} {}", "←".yellow(), importer.path);
                    }
                }
            }
            println!();
        }

        // Show symbols
        if !file_node.symbols.is_empty() {
            println!("{}", "Symbols:".cyan());
            for symbol in &file_node.symbols {
                println!("  {} {}", "•".magenta(), symbol);
            }
            println!();
        }

        // Show related files (deeper traversal)
        if options.depth > 1 {
            println!("{} (depth {})", "Related files:".cyan(), options.depth);
            let related = graph.get_related_files(&file_node.id, options.depth);
            if related.is_empty() {
                println!("  {}", "(none)".dimmed());
            } else {
                for file in related.iter().take(20) {
                    println!("  {} {}", "•".blue(), file.path);
                }
                if related.len() > 20 {
                    println!("  {} ... and {} more", "".dimmed(), related.len() - 20);
                }
            }
        }
    }

    Ok(())
}
