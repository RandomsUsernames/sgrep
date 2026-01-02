#!/usr/bin/env node
/**
 * MCP Wrapper for searchgrep
 * 
 * Some MCP clients (like Codex CLI and Gemini CLI) have stdio transport issues
 * with direct Rust binaries. This Node.js wrapper properly pipes stdio to fix
 * "Transport closed" and "Client is not connected" errors.
 * 
 * Usage:
 *   node mcp-wrapper.js [path-to-searchgrep]
 * 
 * If no path is provided, it searches common installation locations.
 */

const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');

// Find searchgrep binary
let searchgrepPath = process.argv[2];

if (!searchgrepPath) {
  const possiblePaths = [
    path.join(process.env.HOME || '', '.cargo/bin/searchgrep'),
    '/usr/local/bin/searchgrep',
    '/opt/homebrew/bin/searchgrep',
    path.join(process.env.HOME || '', '.local/bin/searchgrep'),
  ];
  
  for (const p of possiblePaths) {
    if (fs.existsSync(p)) {
      searchgrepPath = p;
      break;
    }
  }
  
  // Fall back to PATH lookup
  if (!searchgrepPath) {
    searchgrepPath = 'searchgrep';
  }
}

const child = spawn(searchgrepPath, ['mcp-server'], {
  stdio: ['pipe', 'pipe', 'inherit']
});

// Properly pipe stdio
process.stdin.pipe(child.stdin);
child.stdout.pipe(process.stdout);

child.on('error', (err) => {
  console.error('Failed to start searchgrep:', err.message);
  console.error('Make sure searchgrep is installed: cargo install searchgrep');
  console.error('Or via Homebrew: brew install RandomsUsernames/sgrep/sgrep');
  process.exit(1);
});

child.on('close', (code) => {
  process.exit(code || 0);
});

// Handle signals
process.on('SIGINT', () => { child.kill('SIGINT'); });
process.on('SIGTERM', () => { child.kill('SIGTERM'); });
