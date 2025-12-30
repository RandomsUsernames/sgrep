//! MCP Server implementation for searchgrep
//!
//! Runs as a stdio JSON-RPC server for Claude Code integration.

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

use crate::core::config::Config;
use crate::core::hybrid_embedder::HybridEmbedder;
use crate::core::local_embeddings::{LocalEmbedder, SpeedMode};
use crate::core::search::HybridSearcher;
use crate::core::store::VectorStore;

use super::protocol::*;

pub struct McpServer {
    initialized: bool,
}

impl McpServer {
    pub fn new() -> Self {
        Self { initialized: false }
    }

    /// Run the MCP server (blocking, reads from stdin, writes to stdout)
    pub fn run(&mut self) -> Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }

            // Parse JSON-RPC request
            let request: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    let response =
                        JsonRpcResponse::error(None, -32700, format!("Parse error: {}", e));
                    self.write_response(&mut stdout, &response)?;
                    continue;
                }
            };

            // Handle the request
            let response = self.handle_request(request);
            self.write_response(&mut stdout, &response)?;
        }

        Ok(())
    }

    fn write_response(&self, stdout: &mut io::Stdout, response: &JsonRpcResponse) -> Result<()> {
        let json = serde_json::to_string(response)?;
        writeln!(stdout, "{}", json)?;
        stdout.flush()?;
        Ok(())
    }

    fn handle_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id),
            "initialized" => JsonRpcResponse::success(request.id, json!({})),
            "tools/list" => self.handle_tools_list(request.id),
            "tools/call" => self.handle_tools_call(request.id, request.params),
            "ping" => JsonRpcResponse::success(request.id, json!({})),
            _ => JsonRpcResponse::error(
                request.id,
                -32601,
                format!("Method not found: {}", request.method),
            ),
        }
    }

    fn handle_initialize(&mut self, id: Option<Value>) -> JsonRpcResponse {
        self.initialized = true;

        let result = InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                tools: ToolsCapability {
                    list_changed: false,
                },
            },
            server_info: ServerInfo {
                name: "searchgrep".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    fn handle_tools_list(&self, id: Option<Value>) -> JsonRpcResponse {
        let tools = vec![
            ToolDefinition {
                name: "semantic_search".to_string(),
                description: "Search code semantically using natural language. Finds relevant code based on meaning, not just keywords. Uses AI embeddings to understand code context and find related files, functions, and patterns.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Natural language search query (e.g., 'error handling for HTTP requests', 'database connection pooling', 'authentication middleware')"
                        },
                        "path": {
                            "type": "string",
                            "description": "Optional: Directory path to search in (defaults to current indexed directory)"
                        },
                        "max_results": {
                            "type": "integer",
                            "description": "Maximum number of results to return (default: 10, max: 50)",
                            "default": 10
                        },
                        "mode": {
                            "type": "string",
                            "enum": ["balanced", "code", "hybrid"],
                            "description": "Search mode: 'balanced' (general), 'code' (code-optimized), 'hybrid' (best quality, combines both)",
                            "default": "balanced"
                        },
                        "include_content": {
                            "type": "boolean",
                            "description": "Include file content in results",
                            "default": true
                        }
                    },
                    "required": ["query"]
                }),
            },
            ToolDefinition {
                name: "index_directory".to_string(),
                description: "Index a directory for semantic search. Creates vector embeddings of all code files for fast semantic search.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Directory path to index"
                        },
                        "mode": {
                            "type": "string",
                            "enum": ["fast", "balanced", "code"],
                            "description": "Indexing mode: 'fast' (quick, lower quality), 'balanced' (default), 'code' (code-optimized)",
                            "default": "balanced"
                        }
                    },
                    "required": ["path"]
                }),
            },
        ];

        let result = ToolsListResult { tools };
        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    fn handle_tools_call(&self, id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(id, -32602, "Missing params".to_string());
            }
        };

        let call: ToolCallParams = match serde_json::from_value(params) {
            Ok(c) => c,
            Err(e) => {
                return JsonRpcResponse::error(id, -32602, format!("Invalid params: {}", e));
            }
        };

        let result = match call.name.as_str() {
            "semantic_search" => self.execute_semantic_search(call.arguments),
            "index_directory" => self.execute_index_directory(call.arguments),
            _ => ToolCallResult::error(format!("Unknown tool: {}", call.name)),
        };

        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    fn execute_semantic_search(&self, args: Option<Value>) -> ToolCallResult {
        let args = match args {
            Some(a) => a,
            None => return ToolCallResult::error("Missing arguments".to_string()),
        };

        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q.to_string(),
            None => return ToolCallResult::error("Missing required 'query' argument".to_string()),
        };

        let max_results = args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .min(50) as usize;

        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("balanced");

        let include_content = args
            .get("include_content")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let store_path = args.get("path").and_then(|v| v.as_str());

        // Load the vector store
        let store = match VectorStore::load(store_path) {
            Ok(s) => s,
            Err(e) => return ToolCallResult::error(format!("Failed to load index: {}", e)),
        };

        if store.chunk_count() == 0 {
            return ToolCallResult::error(
                "No files indexed. Run 'searchgrep watch <path>' first to index your codebase."
                    .to_string(),
            );
        }

        // Generate query embedding based on mode
        let query_embedding = match mode {
            "hybrid" => match HybridEmbedder::new() {
                Ok(mut embedder) => match embedder.embed_query(&query) {
                    Ok(emb) => emb,
                    Err(e) => return ToolCallResult::error(format!("Embedding failed: {}", e)),
                },
                Err(e) => return ToolCallResult::error(format!("Model load failed: {}", e)),
            },
            "code" => match LocalEmbedder::with_speed_mode(SpeedMode::Code) {
                Ok(mut embedder) => match embedder.embed_query(&query) {
                    Ok(emb) => emb,
                    Err(e) => return ToolCallResult::error(format!("Embedding failed: {}", e)),
                },
                Err(e) => return ToolCallResult::error(format!("Model load failed: {}", e)),
            },
            _ => {
                // balanced mode
                match LocalEmbedder::with_speed_mode(SpeedMode::Balanced) {
                    Ok(mut embedder) => match embedder.embed_query(&query) {
                        Ok(emb) => emb,
                        Err(e) => return ToolCallResult::error(format!("Embedding failed: {}", e)),
                    },
                    Err(e) => return ToolCallResult::error(format!("Model load failed: {}", e)),
                }
            }
        };

        // Search
        let searcher = HybridSearcher::default();
        let results = searcher.search(
            &store,
            &query_embedding,
            &query,
            max_results,
            None,
            false,
            None,
        );

        if results.is_empty() {
            return ToolCallResult::success(format!(
                "No results found for query: '{}'\n\nTry:\n- Different search terms\n- Check if the directory is indexed",
                query
            ));
        }

        // Format results
        let mut output = format!("Found {} results for: '{}'\n\n", results.len(), query);

        for (i, result) in results.iter().enumerate() {
            let score_pct = (result.score * 100.0) as u32;
            output.push_str(&format!(
                "{}. {} ({}% match)\n",
                i + 1,
                result.chunk.file_path,
                score_pct
            ));
            output.push_str(&format!(
                "   Lines {}-{}\n",
                result.chunk.start_line, result.chunk.end_line
            ));

            if include_content {
                output.push_str("   ```\n");
                for line in result.chunk.content.lines().take(15) {
                    output.push_str(&format!("   {}\n", line));
                }
                if result.chunk.content.lines().count() > 15 {
                    output.push_str("   ...\n");
                }
                output.push_str("   ```\n");
            }
            output.push('\n');
        }

        ToolCallResult::success(output)
    }

    fn execute_index_directory(&self, args: Option<Value>) -> ToolCallResult {
        let args = match args {
            Some(a) => a,
            None => return ToolCallResult::error("Missing arguments".to_string()),
        };

        let path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return ToolCallResult::error("Missing required 'path' argument".to_string()),
        };

        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("balanced");

        let speed_mode = match mode {
            "fast" => SpeedMode::Fast,
            "code" => SpeedMode::Code,
            _ => SpeedMode::Balanced,
        };

        // Run indexing synchronously (blocking)
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => return ToolCallResult::error(format!("Runtime error: {}", e)),
        };

        match rt.block_on(async {
            crate::commands::watch::sync_files(&path, None, speed_mode).await
        }) {
            Ok(_) => ToolCallResult::success(format!(
                "Successfully indexed directory: {}\n\nYou can now use semantic_search to find code.",
                path
            )),
            Err(e) => ToolCallResult::error(format!("Indexing failed: {}", e)),
        }
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}
