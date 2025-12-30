# searchgrep

**Semantic grep for the AI era** - natural language code search powered by Rust.

## Installation

### npm (macOS, Linux, Windows)

```bash
npm i -g searchgrep
```

### Homebrew (macOS)

```bash
brew install RandomsUsernames/tap/searchgrep
```

### From Source

```bash
# Requires Rust: https://rustup.rs
cargo install --git https://github.com/RandomsUsernames/Searchgrep.git
```

## Quick Start

```bash
# Index your codebase
searchgrep watch --once

# Search
searchgrep search "authentication middleware"
searchgrep search "where are errors handled" --content
searchgrep ask "how does the login flow work"
```

## Claude Code Integration (MCP)

searchgrep includes an MCP server for direct Claude Code integration.

Add to `~/.claude/mcp_servers.json`:

```json
{
  "mcpServers": {
    "searchgrep": {
      "command": "searchgrep-mcp",
      "args": [],
      "env": {}
    }
  }
}
```

## Search Modes

| Mode | Flag | Best For |
|------|------|----------|
| Balanced | (default) | General text search |
| Code | `--code` | Code-specific search |
| Hybrid | `--hybrid` | Best quality (combines models) |

## Links

- [GitHub](https://github.com/RandomsUsernames/Searchgrep)
- [Issues](https://github.com/RandomsUsernames/Searchgrep/issues)

## License

MIT
