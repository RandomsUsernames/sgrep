//! MCP (Model Context Protocol) Server for searchgrep
//!
//! Exposes searchgrep as a tool for Claude Code via JSON-RPC over stdio.

mod protocol;
mod server;

pub use server::McpServer;
