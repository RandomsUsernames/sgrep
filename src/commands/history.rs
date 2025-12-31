use anyhow::Result;
use colored::Colorize;

use crate::core::git::GitRepo;
use crate::core::store::VectorStore;

pub struct HistoryOptions {
    /// File path to show history for
    pub file: String,
    /// Number of commits to show
    pub limit: usize,
    /// Use alternative store name
    pub store: Option<String>,
    /// Show diffs
    pub diff: bool,
    /// Output as JSON
    pub json: bool,
}

pub async fn run(options: HistoryOptions) -> Result<()> {
    // Try direct git first (fast path - no store loading needed)
    let cwd = std::env::current_dir()?;

    if let Ok(git_repo) = GitRepo::open(cwd.to_str().unwrap_or(".")) {
        let commits = git_repo.get_file_commits(&options.file, options.limit)?;

        if !commits.is_empty() {
            return display_commits(&options, &commits);
        }
    }

    // Fall back to knowledge graph if store exists
    if let Ok(store) = VectorStore::load(options.store.as_deref()) {
        let graph = &store.graph;

        // Try to find the file in the graph
        let file_node = graph.files.values().find(|f| {
            f.path == options.file
                || f.path.ends_with(&options.file)
                || f.id.ends_with(&options.file)
        });

        if let Some(file_node) = file_node {
            let commits = graph.get_file_history(&file_node.id);

            if !commits.is_empty() {
                if options.json {
                    let commit_json: Vec<_> = commits
                        .iter()
                        .take(options.limit)
                        .map(|c| {
                            serde_json::json!({
                                "hash": c.hash,
                                "message": c.message,
                                "author": c.author,
                                "timestamp": c.timestamp,
                                "files_modified": c.files_modified.len()
                            })
                        })
                        .collect();

                    println!(
                        "{}",
                        serde_json::json!({
                            "file": file_node.path,
                            "commits": commit_json,
                            "total": commits.len(),
                            "source": "graph"
                        })
                    );
                    return Ok(());
                }

                println!(
                    "{} {}",
                    "History for:".cyan().bold(),
                    file_node.path.white().bold()
                );
                println!();

                for commit in commits.iter().take(options.limit) {
                    let short_hash = &commit.hash[..7.min(commit.hash.len())];
                    let date = chrono::DateTime::from_timestamp(commit.timestamp, 0)
                        .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    println!(
                        "  {} {} {}",
                        short_hash.yellow(),
                        date.dimmed(),
                        commit.author.cyan()
                    );
                    println!("    {}", commit.message);
                    println!();
                }

                if commits.len() > options.limit {
                    println!(
                        "  {} Showing {} of {} commits",
                        "...".dimmed(),
                        options.limit,
                        commits.len()
                    );
                }

                return Ok(());
            }
        }
    }

    // No commits found anywhere
    println!(
        "{} No commit history found for: {}",
        "Note:".yellow(),
        options.file
    );
    Ok(())
}

use crate::core::graph::CommitNode;

fn display_commits(options: &HistoryOptions, commits: &[CommitNode]) -> Result<()> {
    if options.json {
        let commit_json: Vec<_> = commits
            .iter()
            .map(|c| {
                serde_json::json!({
                    "hash": c.hash,
                    "message": c.message,
                    "author": c.author,
                    "timestamp": c.timestamp
                })
            })
            .collect();

        println!(
            "{}",
            serde_json::json!({
                "file": options.file,
                "commits": commit_json,
                "total": commits.len(),
                "source": "git"
            })
        );
        return Ok(());
    }

    println!(
        "{} {}",
        "History for:".cyan().bold(),
        options.file.white().bold()
    );
    println!();

    for commit in commits {
        let short_hash = &commit.hash[..7.min(commit.hash.len())];
        let date = chrono::DateTime::from_timestamp(commit.timestamp, 0)
            .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "unknown".to_string());

        println!(
            "  {} {} {}",
            short_hash.yellow(),
            date.dimmed(),
            commit.author.cyan()
        );
        println!("    {}", commit.message);

        if options.diff {
            // Show diff for this commit using git show
            if let Ok(output) = std::process::Command::new("git")
                .args(["show", "--format=", &commit.hash, "--", &options.file])
                .output()
            {
                let diff = String::from_utf8_lossy(&output.stdout);
                if !diff.is_empty() {
                    println!();
                    for line in diff.lines().take(20) {
                        if line.starts_with('+') && !line.starts_with("+++") {
                            println!("    {}", line.green());
                        } else if line.starts_with('-') && !line.starts_with("---") {
                            println!("    {}", line.red());
                        } else if line.starts_with("@@") {
                            println!("    {}", line.cyan());
                        }
                    }
                }
            }
        }

        println!();
    }

    Ok(())
}
