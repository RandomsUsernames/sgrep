#!/usr/bin/env node
/**
 * Postinstall script for searchgrep
 * Automatically registers the MCP server with supported AI coding agents
 */

import { existsSync, readFileSync, writeFileSync, mkdirSync } from "fs";
import { homedir } from "os";
import { join } from "path";

const HOME = homedir();

// MCP server configuration for searchgrep
const SEARCHGREP_MCP = {
  command: "searchgrep-mcp",
  args: [],
  env: {},
};

// Supported AI agent configurations
const AI_AGENTS = [
  {
    name: "Claude Desktop",
    configPath: join(HOME, ".config", "claude-desktop", "mcp.json"),
    configDir: join(HOME, ".config", "claude-desktop"),
    key: "mcpServers",
  },
  {
    name: "Claude Code",
    configPath: join(HOME, ".claude", "mcp_servers.json"),
    configDir: join(HOME, ".claude"),
    key: "mcpServers",
  },
  {
    name: "Claude Code (config)",
    configPath: join(HOME, ".config", "claude-code", "mcp_servers.json"),
    configDir: join(HOME, ".config", "claude-code"),
    key: "mcpServers",
  },
];

function log(msg) {
  console.log(`\x1b[36m[searchgrep]\x1b[0m ${msg}`);
}

function success(msg) {
  console.log(`\x1b[32m[searchgrep]\x1b[0m ${msg}`);
}

function warn(msg) {
  console.log(`\x1b[33m[searchgrep]\x1b[0m ${msg}`);
}

function registerWithAgent(agent) {
  try {
    // Check if config directory exists
    if (!existsSync(agent.configDir)) {
      // Only create if parent exists (don't create for agents not installed)
      const parent = join(agent.configDir, "..");
      if (!existsSync(parent)) {
        return false;
      }
      mkdirSync(agent.configDir, { recursive: true });
    }

    let config = {};

    // Read existing config if it exists
    if (existsSync(agent.configPath)) {
      try {
        const content = readFileSync(agent.configPath, "utf-8");
        config = JSON.parse(content);
      } catch (e) {
        // If parsing fails, start fresh
        config = {};
      }
    }

    // Ensure mcpServers key exists
    if (!config[agent.key]) {
      config[agent.key] = {};
    }

    // Check if already registered
    if (config[agent.key].searchgrep) {
      log(`Already registered with ${agent.name}`);
      return true;
    }

    // Add searchgrep
    config[agent.key].searchgrep = SEARCHGREP_MCP;

    // Write updated config
    writeFileSync(agent.configPath, JSON.stringify(config, null, 2) + "\n");
    success(`Registered with ${agent.name}`);
    return true;
  } catch (e) {
    warn(`Could not register with ${agent.name}: ${e.message}`);
    return false;
  }
}

function main() {
  console.log("");
  log("Setting up MCP server for AI coding agents...");
  console.log("");

  let registered = 0;

  for (const agent of AI_AGENTS) {
    if (registerWithAgent(agent)) {
      registered++;
    }
  }

  console.log("");

  if (registered > 0) {
    success(`Searchgrep MCP server installed!`);
    console.log("");
    console.log("  Available tools in your AI agent:");
    console.log("    - \x1b[1msearch\x1b[0m: Semantic code search with natural language");
    console.log("    - \x1b[1mindex\x1b[0m: Index a directory for searching");
    console.log("    - \x1b[1mstatus\x1b[0m: Check index status");
    console.log("");
    console.log("  \x1b[90mRestart your AI agent to activate the MCP server.\x1b[0m");
  } else {
    log("No supported AI agents detected.");
    console.log("");
    console.log("  To manually configure, add to your MCP config:");
    console.log("");
    console.log('    "searchgrep": {');
    console.log('      "command": "searchgrep-mcp"');
    console.log("    }");
  }

  console.log("");
}

main();
