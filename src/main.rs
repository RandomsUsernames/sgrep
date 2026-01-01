use anyhow::Result;
use clap::{Parser, Subcommand};
use std::io::{self, Write};

mod commands;
mod core;
mod mcp;
pub mod ui;

use commands::{clean, compile, config, graph, history, index, search, status, watch};

#[derive(Parser)]
#[command(name = "sgrep")]
#[command(about = "Semantic grep for the AI era - natural language code search")]
#[command(version = "0.1.0")]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Search pattern (if no command specified)
    #[arg(value_name = "PATTERN")]
    pattern: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Search files using natural language
    #[command(alias = "s")]
    Search {
        /// Natural language search query
        pattern: String,

        /// Path to search in (defaults to current directory)
        path: Option<String>,

        /// Maximum number of results
        #[arg(short = 'm', long, default_value = "10")]
        max_count: usize,

        /// Show file content snippets
        #[arg(short = 'c', long)]
        content: bool,

        /// Generate AI answer from search results
        #[arg(short = 'a', long)]
        answer: bool,

        /// Sync files before searching
        #[arg(short = 's', long)]
        sync: bool,

        /// Disable result reranking
        #[arg(long)]
        no_rerank: bool,

        /// Use ColBERT token-level matching (local only)
        #[arg(long)]
        colbert: bool,

        /// Filter by file type (e.g., ts, py, js)
        #[arg(short = 't', long = "type", value_name = "EXT")]
        file_types: Vec<String>,

        /// Use alternative store name
        #[arg(long)]
        store: Option<String>,

        /// Code mode - use CodeRankEmbed optimized for code search
        #[arg(long)]
        code: bool,

        /// Hybrid mode - use BGE + CodeRankEmbed fusion for best quality
        #[arg(long)]
        hybrid: bool,

        /// Output results as JSON
        #[arg(long)]
        json: bool,

        /// Include related files (imports/importers) in results
        #[arg(long)]
        related: bool,

        /// Depth for related file traversal (default: 1)
        #[arg(long, default_value = "1")]
        related_depth: usize,
    },

    /// Index files and watch for changes
    #[command(alias = "w")]
    Watch {
        /// Path to watch (defaults to current directory)
        path: Option<String>,

        /// Use alternative store name
        #[arg(long)]
        store: Option<String>,

        /// Index files once without watching
        #[arg(long)]
        once: bool,

        /// Fast mode - use smaller model (MiniLM) for 2-3x faster indexing
        #[arg(long)]
        fast: bool,

        /// Quality mode - use F32 precision for highest accuracy
        #[arg(long)]
        quality: bool,

        /// Code mode - use CodeRankEmbed optimized for code search
        #[arg(long)]
        code: bool,
    },

    /// Configure sgrep settings
    #[command(alias = "c")]
    Config {
        /// Set OpenAI API key
        #[arg(long)]
        api_key: Option<String>,

        /// Set embedding model
        #[arg(long)]
        model: Option<String>,

        /// Set custom API base URL
        #[arg(long)]
        base_url: Option<String>,

        /// Set embedding provider (openai, local, or c2llm)
        #[arg(long)]
        provider: Option<String>,

        /// Set local embedding server URL
        #[arg(long)]
        local_url: Option<String>,

        /// Show current configuration
        #[arg(long)]
        show: bool,

        /// Clear all indexed files
        #[arg(long)]
        clear: bool,

        /// Download C2LLM-0.5B model for local embeddings
        #[arg(long)]
        download_model: bool,
    },

    /// Show index status and statistics
    #[command(alias = "st")]
    Status {
        /// Use alternative store name
        #[arg(long)]
        store: Option<String>,

        /// List indexed files
        #[arg(long)]
        files: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Run as MCP server for Claude Code integration
    #[command(name = "mcp-server")]
    McpServer,

    /// Remove stored indexes for privacy/storage management
    Clean {
        /// List all indexes and their sizes
        #[arg(short, long)]
        list: bool,

        /// Remove ALL indexes
        #[arg(short, long)]
        all: bool,

        /// Remove a specific index by name
        #[arg(short, long)]
        store: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Compile codebase map for LLM consumption (90% token reduction)
    Compile {
        /// Path to compile (defaults to current directory)
        path: Option<String>,

        /// Show the compiled map
        #[arg(short, long)]
        show: bool,

        /// Show minimal overview (just function names)
        #[arg(short, long)]
        minimal: bool,
    },

    /// Fast parallel indexing with multiple optimization strategies
    #[command(alias = "i")]
    Index {
        /// Path to index (defaults to current directory)
        path: Option<String>,

        /// Use alternative store name
        #[arg(long)]
        store: Option<String>,

        /// Fast mode - BM25 only, instant indexing (no embeddings)
        #[arg(long)]
        fast: bool,

        /// Balanced mode - BM25 + embeddings (default)
        #[arg(long)]
        balanced: bool,

        /// Quality mode - best embeddings, slower
        #[arg(long)]
        quality: bool,

        /// Force re-index all files (ignore cache)
        #[arg(short, long)]
        force: bool,

        /// Number of threads (0 = auto-detect)
        #[arg(short, long, default_value = "0")]
        threads: usize,

        /// Batch size for embedding requests
        #[arg(short, long, default_value = "50")]
        batch_size: usize,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Build and install sgrep to ~/.cargo/bin
    Install,

    /// Setup MCP integration for AI coding tools
    #[command(alias = "mcp")]
    Setup {
        /// Tool to configure: claude, opencode, cursor, windsurf, or all
        #[arg(default_value = "interactive")]
        tool: String,
    },

    /// Ask a question about your codebase
    #[command(alias = "a")]
    Ask {
        /// Question to ask about the code
        question: String,

        /// Path to search in
        path: Option<String>,

        /// Number of context files to use
        #[arg(short = 'm', long, default_value = "5")]
        max_count: usize,

        /// Sync files before asking
        #[arg(short = 's', long)]
        sync: bool,

        /// Use alternative store name
        #[arg(long)]
        store: Option<String>,
    },

    /// Update sgrep to the latest version from GitHub
    Update {
        /// Check for updates without installing
        #[arg(long)]
        check: bool,
    },

    /// Remove sgrep MCP configuration from AI tools
    Remove {
        /// Tool to remove from: claude, opencode, cursor, windsurf, or all
        #[arg(default_value = "interactive")]
        tool: String,
    },

    /// Initialize sgrep in current directory (index + compile)
    Init {
        /// Fast mode - skip embeddings, BM25 only
        #[arg(long)]
        fast: bool,

        /// Also setup MCP for Claude Code
        #[arg(long)]
        mcp: bool,
    },

    /// Install sgrep as a skill/tool for AI coding tools (in addition to MCP)
    #[command(alias = "skills")]
    Skill {
        /// Tool to install skill for: claude, gemini, opencode, or all
        #[arg(default_value = "interactive")]
        tool: String,
    },

    /// Show knowledge graph relationships for files
    #[command(alias = "g")]
    Graph {
        /// File path to show graph for (optional, shows overview if not provided)
        file: Option<String>,

        /// Show only imports
        #[arg(long)]
        imports: bool,

        /// Show only importers (files that import this file)
        #[arg(long)]
        importers: bool,

        /// Depth of relationship traversal
        #[arg(short, long, default_value = "1")]
        depth: usize,

        /// Use alternative store name
        #[arg(long)]
        store: Option<String>,

        /// Show graph statistics only
        #[arg(long)]
        stats: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show git history for a file with related changes
    #[command(alias = "h")]
    History {
        /// File path to show history for
        file: String,

        /// Number of commits to show
        #[arg(short, long, default_value = "10")]
        limit: usize,

        /// Use alternative store name
        #[arg(long)]
        store: Option<String>,

        /// Show diffs
        #[arg(short, long)]
        diff: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Watch files and keep index updated automatically
    #[command(alias = "w2")]
    Daemon {
        /// Path to watch (defaults to current directory)
        path: Option<String>,

        /// Use alternative store name
        #[arg(long)]
        store: Option<String>,

        /// Fast mode - BM25 only, instant indexing (no embeddings)
        #[arg(long)]
        fast: bool,

        /// Balanced mode - BM25 + embeddings (default)
        #[arg(long)]
        balanced: bool,

        /// Quality mode - best embeddings, slower
        #[arg(long)]
        quality: bool,

        /// Debounce delay in milliseconds (default: 500)
        #[arg(long, default_value = "500")]
        debounce: u64,

        /// Max delay before forced flush in seconds (default: 5)
        #[arg(long, default_value = "5")]
        max_delay: u64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Search {
            pattern,
            path,
            max_count,
            content,
            answer,
            sync,
            no_rerank,
            colbert,
            file_types,
            store,
            code,
            hybrid,
            json,
            related,
            related_depth,
        }) => {
            search::run(search::SearchOptions {
                pattern,
                path,
                max_count,
                content,
                answer,
                sync,
                rerank: !no_rerank,
                colbert,
                file_types: if file_types.is_empty() {
                    None
                } else {
                    Some(file_types)
                },
                store,
                code,
                hybrid,
                json,
                related,
                related_depth,
            })
            .await?;
        }
        Some(Commands::Watch {
            path,
            store,
            once,
            fast,
            quality,
            code,
        }) => {
            watch::run(watch::WatchOptions {
                path,
                store,
                once,
                fast,
                quality,
                code,
            })
            .await?;
        }
        Some(Commands::Config {
            api_key,
            model,
            base_url,
            provider,
            local_url,
            show,
            clear,
            download_model,
        }) => {
            config::run(config::ConfigOptions {
                api_key,
                model,
                base_url,
                provider,
                local_url,
                show,
                clear,
                download_model,
            })
            .await?;
        }
        Some(Commands::Status { store, files, json }) => {
            status::run(status::StatusOptions { store, files, json }).await?;
        }
        Some(Commands::McpServer) => {
            let mut server = mcp::McpServer::new();
            server.run()?;
        }
        Some(Commands::Clean {
            list,
            all,
            store,
            json,
        }) => {
            clean::run(clean::CleanOptions {
                list,
                all,
                store,
                json,
            })
            .await?;
        }
        Some(Commands::Compile {
            path,
            show,
            minimal,
        }) => {
            compile::run(compile::CompileOptions {
                path,
                show,
                minimal,
            })
            .await?;
        }
        Some(Commands::Index {
            path,
            store,
            fast,
            balanced,
            quality,
            force,
            threads,
            batch_size,
            json,
        }) => {
            index::run(index::IndexOptions {
                path,
                store,
                fast,
                balanced,
                quality,
                force,
                threads,
                batch_size,
                json,
            })
            .await?;
        }
        Some(Commands::Install) => {
            use anyhow::Context;
            use colored::Colorize;
            use std::process::Command;

            // Try to find project directory
            let project_dir = {
                // Try current directory first
                let cwd = std::env::current_dir()?;
                if cwd.join("Cargo.toml").exists() {
                    cwd
                } else {
                    // Walk up from current dir
                    let mut dir = cwd.clone();
                    let mut found = None;
                    for _ in 0..5 {
                        if dir.join("Cargo.toml").exists() {
                            found = Some(dir.clone());
                            break;
                        }
                        if let Some(parent) = dir.parent() {
                            dir = parent.to_path_buf();
                        } else {
                            break;
                        }
                    }
                    found.ok_or_else(|| anyhow::anyhow!(
                        "Could not find sgrep source. Run this command from the sgrep source directory."
                    ))?
                }
            };

            println!(
                "{} Building release from {}...",
                "⚙".cyan(),
                project_dir.display()
            );

            let status = Command::new("cargo")
                .args(["build", "--release"])
                .current_dir(&project_dir)
                .status()?;

            if !status.success() {
                anyhow::bail!("Build failed");
            }

            let target_binary = project_dir.join("target/release/sgrep");
            let home = dirs::home_dir().context("Could not find home directory")?;
            let install_path = home.join(".cargo/bin/sgrep");

            std::fs::copy(&target_binary, &install_path)?;

            println!(
                "{} Installed to {}",
                "✓".green().bold(),
                install_path.display()
            );
            println!("   Run {} to verify", "sgrep --version".cyan());
        }
        Some(Commands::Setup { tool }) => {
            use anyhow::Context;
            use colored::Colorize;
            use std::io::{self, Write};

            let home = dirs::home_dir().context("Could not find home directory")?;

            // Find sgrep binary path
            let sgrep_path = which::which("sgrep")
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "sgrep".to_string());

            let tools: Vec<&str> = match tool.as_str() {
                "interactive" => {
                    println!("{}", "sgrep MCP Setup".cyan().bold());
                    println!();
                    println!("Select AI coding tool to configure:");
                    println!();
                    println!("  {}  Claude Code", "1.".bold());
                    println!("  {}  OpenCode", "2.".bold());
                    println!("  {}  Cursor", "3.".bold());
                    println!("  {}  Windsurf", "4.".bold());
                    println!("  {}  Codex (OpenAI)", "5.".bold());
                    println!("  {}  Gemini CLI", "6.".bold());
                    println!("  {}  Cody (Sourcegraph)", "7.".bold());
                    println!("  {}  Continue", "8.".bold());
                    println!("  {}  Aider", "9.".bold());
                    println!("  {} Zed", "10.".bold());
                    println!("  {} ACP (Agent Control Protocol)", "11.".bold());
                    println!("  {} Droid", "12.".bold());
                    println!("  {} Amp", "13.".bold());
                    println!("  {} Roo Code", "14.".bold());
                    println!("  {} Cline", "15.".bold());
                    println!();
                    print!("Enter choice (1-15): ");
                    io::stdout().flush()?;

                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;

                    match input.trim() {
                        "1" => vec!["claude"],
                        "2" => vec!["opencode"],
                        "3" => vec!["cursor"],
                        "4" => vec!["windsurf"],
                        "5" => vec!["codex"],
                        "6" => vec!["gemini"],
                        "7" => vec!["cody"],
                        "8" => vec!["continue"],
                        "9" => vec!["aider"],
                        "10" => vec!["zed"],
                        "11" => vec!["acp"],
                        "12" => vec!["droid"],
                        "13" => vec!["amp"],
                        "14" => vec!["roo"],
                        "15" => vec!["cline"],
                        _ => {
                            println!("{} Invalid choice", "✗".red());
                            return Ok(());
                        }
                    }
                }
                "claude" => vec!["claude"],
                "opencode" => vec!["opencode"],
                "cursor" => vec!["cursor"],
                "windsurf" => vec!["windsurf"],
                "codex" => vec!["codex"],
                "gemini" => vec!["gemini"],
                "cody" => vec!["cody"],
                "continue" => vec!["continue"],
                "aider" => vec!["aider"],
                "zed" => vec!["zed"],
                "acp" => vec!["acp"],
                "droid" => vec!["droid"],
                "amp" => vec!["amp"],
                "roo" => vec!["roo"],
                "cline" => vec!["cline"],
                _ => {
                    println!("{} Unknown tool: {}", "✗".red(), tool);
                    println!("Available: claude, opencode, cursor, windsurf, codex, gemini, cody, continue, aider, zed, acp, droid, amp, roo, cline");
                    return Ok(());
                }
            };

            for tool_name in tools {
                // Each tool has its own config path and format
                let (config_path, config_type) = match tool_name {
                    "claude" => (home.join(".claude.json"), "claude"),
                    "cursor" => (home.join(".cursor/mcp.json"), "mcpServers"),
                    "windsurf" => (home.join(".codeium/windsurf/mcp_config.json"), "mcpServers"),
                    "codex" => (home.join(".codex/config.toml"), "codex_toml"),
                    "opencode" => (home.join(".config/opencode/opencode.json"), "opencode"),
                    "zed" => (home.join(".config/zed/settings.json"), "context_servers"),
                    "continue" => (home.join(".continue/config.json"), "experimental.modelContextProtocolServers"),
                    "cline" => (home.join("Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json"), "mcpServers"),
                    "roo" => (home.join("Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json"), "mcpServers"),
                    "amp" => (home.join(".amp/amp.yaml"), "mcpServers"),
                    "gemini" => (home.join(".gemini/settings.json"), "mcpServers"),
                    "cody" => (home.join("Library/Application Support/Code/User/globalStorage/sourcegraph.cody-ai/cody_mcp_settings.json"), "mcpServers"),
                    "aider" => (home.join(".aider/mcp.json"), "mcpServers"),
                    "droid" => (home.join(".droid/mcp.json"), "mcpServers"),
                    "acp" => (home.join(".acp/mcp.json"), "mcpServers"),
                    _ => continue,
                };

                // Create directory if needed
                if let Some(parent) = config_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                // Claude Code uses ~/.claude.json with projects[home]["mcpServers"] structure
                if config_type == "claude" {
                    let mcp_config = serde_json::json!({
                        "command": sgrep_path,
                        "args": ["mcp-server"],
                        "env": {}
                    });

                    let mut config: serde_json::Value = if config_path.exists() {
                        let content = std::fs::read_to_string(&config_path)?;
                        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
                    } else {
                        serde_json::json!({})
                    };

                    // Claude Code uses top-level mcpServers for user config
                    if config.get("mcpServers").is_none() {
                        config["mcpServers"] = serde_json::json!({});
                    }
                    config["mcpServers"]["sgrep"] = mcp_config;

                    let content = serde_json::to_string_pretty(&config)?;
                    std::fs::write(&config_path, content)?;
                } else if config_type == "context_servers" {
                    // Zed uses a different config structure
                    let zed_config = serde_json::json!({
                        "command": sgrep_path,
                        "args": ["mcp-server"]
                    });

                    let mut config: serde_json::Value = if config_path.exists() {
                        let content = std::fs::read_to_string(&config_path)?;
                        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
                    } else {
                        serde_json::json!({})
                    };

                    if config.get("context_servers").is_none() {
                        config["context_servers"] = serde_json::json!({});
                    }
                    config["context_servers"]["sgrep"] = zed_config;

                    let content = serde_json::to_string_pretty(&config)?;
                    std::fs::write(&config_path, content)?;
                } else if config_type == "experimental.modelContextProtocolServers" {
                    // Continue uses experimental.modelContextProtocolServers array
                    let continue_config = serde_json::json!({
                        "name": "sgrep",
                        "command": sgrep_path,
                        "args": ["mcp-server"]
                    });

                    let mut config: serde_json::Value = if config_path.exists() {
                        let content = std::fs::read_to_string(&config_path)?;
                        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
                    } else {
                        serde_json::json!({})
                    };

                    if config.get("experimental").is_none() {
                        config["experimental"] = serde_json::json!({});
                    }
                    if config["experimental"]
                        .get("modelContextProtocolServers")
                        .is_none()
                    {
                        config["experimental"]["modelContextProtocolServers"] =
                            serde_json::json!([]);
                    }

                    // Remove existing sgrep entry if present
                    if let Some(arr) =
                        config["experimental"]["modelContextProtocolServers"].as_array_mut()
                    {
                        arr.retain(|v| v.get("name").and_then(|n| n.as_str()) != Some("sgrep"));
                        arr.push(continue_config);
                    }

                    let content = serde_json::to_string_pretty(&config)?;
                    std::fs::write(&config_path, content)?;
                } else if config_type == "codex_toml" {
                    // Codex uses TOML format in ~/.codex/config.toml
                    let existing_content = if config_path.exists() {
                        std::fs::read_to_string(&config_path)?
                    } else {
                        String::new()
                    };

                    // Check if sgrep is already configured
                    if existing_content.contains("[mcp_servers.sgrep]") {
                        // Update existing entry
                        let updated = existing_content
                            .lines()
                            .filter(|line| {
                                !line.contains("[mcp_servers.sgrep]")
                                    && !line.starts_with("command = \"sgrep\"")
                                    && !(line.starts_with("args = ") && line.contains("mcp"))
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        let new_content = format!(
                            "{}\n\n[mcp_servers.sgrep]\ncommand = \"{}\"\nargs = [\"mcp-server\"]\n",
                            updated.trim(),
                            sgrep_path
                        );
                        std::fs::write(&config_path, new_content)?;
                    } else {
                        // Append new entry
                        let new_content = format!(
                            "{}\n\n[mcp_servers.sgrep]\ncommand = \"{}\"\nargs = [\"mcp-server\"]\n",
                            existing_content.trim(),
                            sgrep_path
                        );
                        std::fs::write(&config_path, new_content)?;
                    }
                } else if config_type == "opencode" {
                    // OpenCode uses JSON format with type, command array, and enabled
                    let mcp_config = serde_json::json!({
                        "type": "local",
                        "command": [sgrep_path, "mcp-server"],
                        "enabled": true
                    });

                    let mut config: serde_json::Value = if config_path.exists() {
                        let content = std::fs::read_to_string(&config_path)?;
                        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
                    } else {
                        serde_json::json!({})
                    };

                    if config.get("mcp").is_none() {
                        config["mcp"] = serde_json::json!({});
                    }
                    config["mcp"]["sgrep"] = mcp_config;

                    let content = serde_json::to_string_pretty(&config)?;
                    std::fs::write(&config_path, content)?;
                } else {
                    // Standard mcpServers format (Cursor, Cline, etc.)
                    let mcp_config = serde_json::json!({
                        "command": sgrep_path,
                        "args": ["mcp-server"],
                        "env": {}
                    });

                    let mut config: serde_json::Value = if config_path.exists() {
                        let content = std::fs::read_to_string(&config_path)?;
                        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
                    } else {
                        serde_json::json!({})
                    };

                    if config.get("mcpServers").is_none() {
                        config["mcpServers"] = serde_json::json!({});
                    }
                    config["mcpServers"]["sgrep"] = mcp_config;

                    let content = serde_json::to_string_pretty(&config)?;
                    std::fs::write(&config_path, content)?;
                }

                println!(
                    "{} Configured {} at {}",
                    "✓".green().bold(),
                    tool_name.cyan(),
                    config_path.display()
                );
            }

            println!();
            println!("{}", "Restart your AI tool to use sgrep.".yellow());
            println!(
                "Available MCP tools: {}, {}",
                "semantic_search".cyan(),
                "index_directory".cyan()
            );
        }
        Some(Commands::Ask {
            question,
            path,
            max_count,
            sync,
            store,
        }) => {
            search::run(search::SearchOptions {
                pattern: question,
                path,
                max_count,
                content: false,
                answer: true,
                sync,
                rerank: true,
                colbert: false,
                file_types: None,
                store,
                code: false,
                hybrid: false,
                json: false,
                related: false,
                related_depth: 1,
            })
            .await?;
        }
        Some(Commands::Update { check }) => {
            use colored::Colorize;

            let current_version = env!("CARGO_PKG_VERSION");
            println!(
                "{} Checking for updates (current: v{})...",
                "→".cyan(),
                current_version
            );

            // Fetch latest release from GitHub
            let client = reqwest::Client::new();
            let resp = client
                .get("https://api.github.com/repos/RandomsUsernames/Searchgrep/releases/latest")
                .header("User-Agent", "sgrep")
                .send()
                .await?;

            if !resp.status().is_success() {
                println!("{} Failed to check for updates", "✗".red());
                return Ok(());
            }

            let release: serde_json::Value = resp.json().await?;
            let latest_version = release["tag_name"]
                .as_str()
                .unwrap_or("unknown")
                .trim_start_matches('v');

            if latest_version == current_version {
                println!("{} Already up to date (v{})", "✓".green(), current_version);
                return Ok(());
            }

            println!(
                "{} New version available: v{} → v{}",
                "!".yellow(),
                current_version,
                latest_version
            );

            if check {
                println!("\nRun 'sgrep update' to install the latest version.");
                return Ok(());
            }

            // Download and install
            println!("{} Downloading latest release...", "→".cyan());

            #[cfg(target_os = "macos")]
            let asset_name = if cfg!(target_arch = "aarch64") {
                "sgrep-aarch64-apple-darwin.tar.gz"
            } else {
                "sgrep-x86_64-apple-darwin.tar.gz"
            };

            #[cfg(target_os = "linux")]
            let asset_name = "sgrep-x86_64-unknown-linux-gnu.tar.gz";

            #[cfg(target_os = "windows")]
            let asset_name = "sgrep-x86_64-pc-windows-msvc.zip";

            let download_url = format!(
                "https://github.com/RandomsUsernames/Searchgrep/releases/download/v{}/{}",
                latest_version, asset_name
            );

            let resp = client.get(&download_url).send().await?;
            if !resp.status().is_success() {
                println!("{} Failed to download release", "✗".red());
                println!("Try manually: {}", download_url);
                return Ok(());
            }

            let bytes = resp.bytes().await?;

            // Extract and install
            let temp_dir = std::env::temp_dir().join("sgrep-update");
            std::fs::create_dir_all(&temp_dir)?;

            let archive_path = temp_dir.join(asset_name);
            std::fs::write(&archive_path, &bytes)?;

            println!("{} Installing...", "→".cyan());

            // Extract using tar
            let status = std::process::Command::new("tar")
                .args([
                    "-xzf",
                    &archive_path.to_string_lossy(),
                    "-C",
                    &temp_dir.to_string_lossy(),
                ])
                .status()?;

            if !status.success() {
                println!("{} Failed to extract archive", "✗".red());
                return Ok(());
            }

            // Copy binary to cargo bin
            let cargo_bin = dirs::home_dir()
                .unwrap_or_default()
                .join(".cargo/bin/sgrep");

            let new_binary = temp_dir.join("sgrep");
            if new_binary.exists() {
                std::fs::copy(&new_binary, &cargo_bin)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&cargo_bin, std::fs::Permissions::from_mode(0o755))?;
                }
            }

            // Cleanup
            let _ = std::fs::remove_dir_all(&temp_dir);

            println!("{} Updated to v{}!", "✓".green(), latest_version);
        }
        Some(Commands::Remove { tool }) => {
            use colored::Colorize;

            let home =
                dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;

            let tools: Vec<&str> = match tool.as_str() {
                "interactive" => {
                    println!("{}", "sgrep MCP Removal".cyan().bold());
                    println!();
                    println!("Select AI tool to remove sgrep from:");
                    println!();
                    println!("  {}  Claude Code", "1.".bold());
                    println!("  {}  OpenCode", "2.".bold());
                    println!("  {}  Cursor", "3.".bold());
                    println!("  {}  Windsurf", "4.".bold());
                    println!("  {}  All configured tools", "5.".bold());
                    println!();
                    print!("Enter choice (1-5): ");
                    io::stdout().flush()?;

                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;

                    match input.trim() {
                        "1" => vec!["claude"],
                        "2" => vec!["opencode"],
                        "3" => vec!["cursor"],
                        "4" => vec!["windsurf"],
                        "5" => vec![
                            "claude", "opencode", "cursor", "windsurf", "codex", "gemini", "cody",
                            "continue", "aider", "zed", "acp", "droid", "amp", "roo", "cline",
                        ],
                        _ => {
                            println!("{} Invalid choice", "✗".red());
                            return Ok(());
                        }
                    }
                }
                "all" => vec![
                    "claude", "opencode", "cursor", "windsurf", "codex", "gemini", "cody",
                    "continue", "aider", "zed", "acp", "droid", "amp", "roo", "cline",
                ],
                other => vec![other],
            };

            for tool_name in tools {
                let (config_path, config_type) = match tool_name {
                    "claude" => (home.join(".claude.json"), "claude"),
                    "cursor" => (home.join(".cursor/mcp.json"), "mcpServers"),
                    "windsurf" => (home.join(".codeium/windsurf/mcp_config.json"), "mcpServers"),
                    "codex" => (home.join(".codex/config.toml"), "codex_toml"),
                    "opencode" => (home.join(".opencode/mcp.json"), "mcpServers"),
                    "zed" => (home.join(".config/zed/settings.json"), "context_servers"),
                    "continue" => (home.join(".continue/config.json"), "experimental.modelContextProtocolServers"),
                    "cline" => (home.join("Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json"), "mcpServers"),
                    "roo" => (home.join("Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json"), "mcpServers"),
                    "amp" => (home.join(".amp/amp.yaml"), "mcpServers"),
                    "gemini" => (home.join(".gemini/settings.json"), "mcpServers"),
                    "cody" => (home.join("Library/Application Support/Code/User/globalStorage/sourcegraph.cody-ai/cody_mcp_settings.json"), "mcpServers"),
                    "aider" => (home.join(".aider/mcp.json"), "mcpServers"),
                    "droid" => (home.join(".droid/mcp.json"), "mcpServers"),
                    "acp" => (home.join(".acp/mcp.json"), "mcpServers"),
                    _ => continue,
                };

                if !config_path.exists() {
                    continue;
                }

                // Handle TOML-based configs (Codex) separately
                if config_type == "codex_toml" {
                    let content = std::fs::read_to_string(&config_path)?;
                    if content.contains("[mcp_servers.sgrep]") {
                        // Remove the [mcp_servers.sgrep] section and its contents
                        let mut lines: Vec<&str> = content.lines().collect();
                        let mut i = 0;
                        let mut in_sgrep_section = false;
                        let mut to_remove = Vec::new();

                        while i < lines.len() {
                            let line = lines[i].trim();
                            if line == "[mcp_servers.sgrep]" {
                                in_sgrep_section = true;
                                to_remove.push(i);
                            } else if in_sgrep_section {
                                if line.starts_with('[') {
                                    // New section started
                                    in_sgrep_section = false;
                                } else {
                                    to_remove.push(i);
                                }
                            }
                            i += 1;
                        }

                        // Remove lines in reverse order to preserve indices
                        for idx in to_remove.into_iter().rev() {
                            lines.remove(idx);
                        }

                        let new_content = lines.join("\n").trim().to_string() + "\n";
                        std::fs::write(&config_path, new_content)?;
                        println!("{} Removed sgrep from {}", "✓".green(), tool_name);
                    }
                    continue;
                }

                // JSON-based configs
                let content = std::fs::read_to_string(&config_path)?;
                let mut config: serde_json::Value =
                    serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}));

                let removed = if config_type == "claude" {
                    // Claude Code uses top-level mcpServers
                    if let Some(servers) = config["mcpServers"].as_object_mut() {
                        servers.remove("sgrep").is_some()
                    } else {
                        false
                    }
                } else if config_type == "context_servers" {
                    if let Some(servers) = config["context_servers"].as_object_mut() {
                        servers.remove("sgrep").is_some()
                    } else {
                        false
                    }
                } else if config_type == "experimental.modelContextProtocolServers" {
                    if let Some(arr) =
                        config["experimental"]["modelContextProtocolServers"].as_array_mut()
                    {
                        let len_before = arr.len();
                        arr.retain(|v| v.get("name").and_then(|n| n.as_str()) != Some("sgrep"));
                        arr.len() < len_before
                    } else {
                        false
                    }
                } else {
                    if let Some(servers) = config["mcpServers"].as_object_mut() {
                        servers.remove("sgrep").is_some()
                    } else {
                        false
                    }
                };

                if removed {
                    std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
                    println!("{} Removed sgrep from {}", "✓".green(), tool_name);
                }
            }
        }
        Some(Commands::Init { fast, mcp }) => {
            use colored::Colorize;

            let current_dir = std::env::current_dir()?;
            println!(
                "{} Initializing sgrep in {}",
                "→".cyan(),
                current_dir.display()
            );

            // Index the directory
            println!("{} Indexing files...", "→".cyan());

            let speed_mode = if fast {
                crate::core::local_embeddings::SpeedMode::Fast
            } else {
                crate::core::local_embeddings::SpeedMode::Balanced
            };

            crate::commands::watch::sync_files(
                current_dir.to_str().unwrap_or("."),
                None,
                speed_mode,
            )
            .await?;

            println!("{} Indexed successfully", "✓".green());

            // Compile codebase map
            println!("{} Compiling codebase map...", "→".cyan());
            match compile::run(compile::CompileOptions {
                path: Some(current_dir.to_string_lossy().to_string()),
                show: false,
                minimal: false,
            })
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    println!("{} Failed to compile: {}", "✗".yellow(), e);
                }
            }

            // Setup MCP if requested
            if mcp {
                println!("{} Setting up Claude Code MCP...", "→".cyan());
                let home = dirs::home_dir()
                    .ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;
                let sgrep_path = which::which("sgrep")
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "sgrep".to_string());

                let config_path = home.join(".claude.json");

                let mut config: serde_json::Value = if config_path.exists() {
                    let content = std::fs::read_to_string(&config_path)?;
                    serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
                } else {
                    serde_json::json!({})
                };

                // Claude Code uses top-level mcpServers for user config
                if config.get("mcpServers").is_none() {
                    config["mcpServers"] = serde_json::json!({});
                }
                config["mcpServers"]["sgrep"] = serde_json::json!({
                    "command": sgrep_path,
                    "args": ["mcp-server"],
                    "env": {}
                });

                std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
                println!("{} Claude Code MCP configured", "✓".green());
            }

            println!();
            println!("{}", "Ready! You can now:".green().bold());
            println!("  • Search: sgrep search \"your query\"");
            println!("  • Ask: sgrep ask \"how does X work?\"");
            if !mcp {
                println!("  • Setup MCP: sgrep mcp");
            }
        }
        Some(Commands::Skill { tool }) => {
            use anyhow::Context;
            use colored::Colorize;

            let home = dirs::home_dir().context("Could not find home directory")?;

            let tools: Vec<&str> = match tool.as_str() {
                "interactive" => {
                    println!("{}", "sgrep Skill Setup".cyan().bold());
                    println!();
                    println!("Install sgrep as a skill/tool for:");
                    println!();
                    println!("  {}  Claude (claude.ai & Claude Code)", "1.".bold());
                    println!("  {}  Gemini CLI", "2.".bold());
                    println!("  {}  OpenCode", "3.".bold());
                    println!("  {}  All", "4.".bold());
                    println!();
                    print!("Enter choice (1-4): ");
                    io::stdout().flush()?;

                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;

                    match input.trim() {
                        "1" => vec!["claude"],
                        "2" => vec!["gemini"],
                        "3" => vec!["opencode"],
                        "4" | "" => vec!["claude", "gemini", "opencode"],
                        _ => {
                            println!("{} Invalid choice", "✗".red());
                            return Ok(());
                        }
                    }
                }
                "claude" => vec!["claude"],
                "gemini" => vec!["gemini"],
                "opencode" => vec!["opencode"],
                "all" => vec!["claude", "gemini", "opencode"],
                _ => {
                    println!("{} Unknown tool: {}", "✗".red(), tool);
                    println!("Available: claude, gemini, opencode, all");
                    return Ok(());
                }
            };

            for tool_name in tools {
                match tool_name {
                    "claude" => {
                        // Claude uses SKILL.md files in .claude/skills/
                        let skill_dir = home.join(".claude/skills/sgrep");
                        std::fs::create_dir_all(&skill_dir)?;

                        let skill_content = r#"---
name: sgrep
description: Semantic code search using AI embeddings. Search your codebase using natural language instead of regex patterns.
---

# sgrep - Semantic Code Search

A powerful semantic grep tool that uses AI embeddings to search code by meaning, not just text matching.

## When to Use This Skill

Use sgrep when you need to:
- Find code related to a concept (e.g., "authentication logic", "error handling")
- Search for implementations without knowing exact function names
- Explore unfamiliar codebases
- Find similar code patterns

## Commands

### Search
```bash
sgrep search "your natural language query" [path]
sgrep search -m 20 "database connection handling"  # more results
sgrep search -c "API error responses"  # show content snippets
```

### Ask (AI-powered Q&A)
```bash
sgrep ask "how does the payment system work?"
sgrep ask "what authentication methods are used?"
```

### Index (for faster searches)
```bash
sgrep index .  # index current directory
sgrep index --fast .  # quick BM25-only indexing
```

## Best Practices

1. Use descriptive natural language queries
2. Be specific about what you're looking for
3. Index large codebases first for faster results

## Examples

- `sgrep search "user authentication middleware"`
- `sgrep search "database connection pooling" src/`
- `sgrep ask "how are API routes organized?"`
- `sgrep search -t py "machine learning model training"`
"#;

                        let skill_path = skill_dir.join("SKILL.md");
                        std::fs::write(&skill_path, skill_content)?;

                        println!(
                            "{} Installed Claude skill at {}",
                            "✓".green().bold(),
                            skill_path.display()
                        );
                    }
                    "gemini" => {
                        // Gemini CLI uses extensions in .gemini/extensions/
                        let ext_dir = home.join(".gemini/extensions/sgrep");
                        std::fs::create_dir_all(&ext_dir)?;

                        // Find sgrep binary path
                        let sgrep_path = which::which("sgrep")
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|_| "sgrep".to_string());

                        let extension_json = serde_json::json!({
                            "name": "sgrep",
                            "description": "Semantic code search using AI embeddings. Search your codebase using natural language.",
                            "mcpServers": {
                                "sgrep": {
                                    "command": sgrep_path,
                                    "args": ["mcp-server"]
                                }
                            },
                            "context": [
                                {
                                    "file": "SKILL.md"
                                }
                            ]
                        });

                        let ext_json_path = ext_dir.join("gemini-extension.json");
                        std::fs::write(
                            &ext_json_path,
                            serde_json::to_string_pretty(&extension_json)?,
                        )?;

                        // Also add a SKILL.md for context
                        let skill_content = r#"# sgrep - Semantic Code Search

Use sgrep to search code by meaning using natural language.

## Commands

- `sgrep search "query"` - Semantic search
- `sgrep ask "question"` - Ask questions about code
- `sgrep index .` - Index for faster searches

## Examples

```bash
sgrep search "authentication middleware"
sgrep search -m 20 "error handling patterns"
sgrep ask "how does the API handle errors?"
```
"#;
                        let skill_path = ext_dir.join("SKILL.md");
                        std::fs::write(&skill_path, skill_content)?;

                        println!(
                            "{} Installed Gemini CLI extension at {}",
                            "✓".green().bold(),
                            ext_dir.display()
                        );
                    }
                    "opencode" => {
                        let skill_dir = home.join(".config/opencode/tool");
                        std::fs::create_dir_all(&skill_dir)?;

                        let skill_content = r#"import { tool } from "@opencode-ai/plugin"

const SKILL = `
---
name: sgrep
description: A semantic grep-like search tool for your local files. Uses AI embeddings for natural language code search. Much better than grep/ripgrep for understanding code meaning.
license: MIT
---

## When to use this skill

Whenever you need to search your local files semantically. Use this instead of grep/rg when you want to find code by meaning, not just text matching.

## How to use this skill

Use \`sgrep\` to search your local files. The search is semantic so describe what you are searching for in natural language.

### Do

\`\`\`bash
sgrep search "authentication middleware"  # search in the current directory
sgrep search "database connection pooling" src/  # search in src directory
sgrep search -m 20 "error handling patterns"  # get more results
sgrep ask "how does the payment system work?"  # ask a question about code
\`\`\`

### Don't

\`\`\`bash
sgrep search "foo"  # Too vague, use descriptive queries
\`\`\`

## Keywords
search, grep, semantic search, code search, natural language search, ai search
`;

export default tool({
  description: SKILL,
  args: {
    q: tool.schema.string().describe("The semantic search query."),
    m: tool.schema.number().default(10).describe("The number of results to return."),
    a: tool.schema.boolean().default(false).describe("If true, use 'ask' mode to generate an answer."),
    p: tool.schema.string().optional().describe("Path to search in (defaults to current directory)."),
  },
  async execute(args) {
    const cmd = args.a ? "ask" : "search";
    const pathArg = args.p ? args.p : ".";
    const result = await Bun.$`sgrep ${cmd} -m ${args.m} ${args.q} ${pathArg}`.text();
    return result.trim();
  },
})"#;

                        let skill_path = skill_dir.join("sgrep.ts");
                        std::fs::write(&skill_path, skill_content)?;

                        println!(
                            "{} Installed OpenCode skill at {}",
                            "✓".green().bold(),
                            skill_path.display()
                        );
                    }
                    _ => continue,
                }
            }

            println!();
            println!(
                "{}",
                "Restart your AI tool to use the sgrep skill.".yellow()
            );
        }
        Some(Commands::Graph {
            file,
            imports,
            importers,
            depth,
            store,
            stats,
            json,
        }) => {
            graph::run(graph::GraphOptions {
                file,
                imports,
                importers,
                depth,
                store,
                stats,
                json,
            })
            .await?;
        }
        Some(Commands::History {
            file,
            limit,
            store,
            diff,
            json,
        }) => {
            history::run(history::HistoryOptions {
                file,
                limit,
                store,
                diff,
                json,
            })
            .await?;
        }
        Some(Commands::Daemon {
            path,
            store,
            fast,
            balanced: _,
            quality,
            debounce,
            max_delay,
        }) => {
            use colored::Colorize;
            use core::daemon::{Daemon, DaemonConfig, DebounceConfig};
            use core::fast_indexer::IndexTier;
            use std::time::Duration;

            let watch_path = path.unwrap_or_else(|| ".".to_string());
            let watch_path = std::fs::canonicalize(&watch_path)?;

            // Determine index tier
            let tier = if fast {
                IndexTier::Fast
            } else if quality {
                IndexTier::Quality
            } else {
                IndexTier::Balanced
            };

            // Build debounce config
            let debounce_config = DebounceConfig {
                debounce_delay: Duration::from_millis(debounce),
                max_delay: Duration::from_secs(max_delay),
                ..Default::default()
            };

            // Build daemon config
            let mut config = DaemonConfig::new(&watch_path)
                .with_tier(tier)
                .with_debounce(debounce_config);

            if let Some(store_name) = store {
                config = config.with_store(store_name);
            }

            println!(
                "{} Starting daemon for {}",
                "→".cyan(),
                watch_path.display()
            );
            println!(
                "   Mode: {}, Debounce: {}ms, Max delay: {}s",
                match tier {
                    IndexTier::Fast => "fast",
                    IndexTier::Balanced => "balanced",
                    IndexTier::Quality => "quality",
                },
                debounce,
                max_delay
            );
            println!("   Press Ctrl+C to stop\n");

            // Run daemon (blocking)
            let daemon = Daemon::new(config);
            daemon.run().await?;
        }
        None => {
            if let Some(pattern) = cli.pattern {
                search::run(search::SearchOptions {
                    pattern,
                    path: None,
                    max_count: 10,
                    content: false,
                    answer: false,
                    sync: false,
                    rerank: true,
                    colbert: false,
                    file_types: None,
                    store: None,
                    code: false,
                    hybrid: false,
                    json: false,
                    related: false,
                    related_depth: 1,
                })
                .await?;
            } else {
                // Show help
                use clap::CommandFactory;
                Cli::command().print_help()?;
            }
        }
    }

    Ok(())
}
