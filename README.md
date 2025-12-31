<p align="center">
  <h1 align="center">searchgrep</h1>
  <p align="center">
    <strong>Semantic grep for the AI era</strong><br>
    Natural language code search powered by Rust
  </p>
</p>

<p align="center">
  <a href="https://crates.io/crates/searchgrep"><img src="https://img.shields.io/crates/v/searchgrep.svg" alt="Crates.io"></a>
  <a href="https://www.npmjs.com/package/searchgrep"><img src="https://img.shields.io/npm/v/searchgrep.svg" alt="npm"></a>
  <a href="https://github.com/RandomsUsernames/sgrep/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License"></a>
  <a href="https://github.com/RandomsUsernames/sgrep/releases"><img src="https://img.shields.io/github/v/release/RandomsUsernames/sgrep" alt="GitHub release"></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/MCP-Compatible-green" alt="MCP Compatible">
  <img src="https://img.shields.io/badge/Apple%20Silicon-Optimized-black" alt="Apple Silicon">
</p>

---

Search your codebase using natural language. Ask questions like *"where are authentication errors handled"* or *"database connection pooling logic"* and get instant results.

```bash
# Install
brew install RandomsUsernames/searchgrep/searchgrep

# Index & Search
searchgrep index .
searchgrep search "error handling for API requests"
```

## Why searchgrep?

| Traditional grep | searchgrep |
|-----------------|------------|
| `grep -r "error"` finds literal matches | Finds code by *meaning* |
| Requires knowing exact terms | Use natural language |
| Misses synonyms and related code | Understands context |
| No AI integration | Works with Claude, Cursor, etc. |

## Installation

### Homebrew (macOS)

```bash
brew tap RandomsUsernames/searchgrep
brew install searchgrep
```

### npm / npx

```bash
npx searchgrep          # Run directly
npm install -g searchgrep  # Install globally
```

### From Source

```bash
git clone https://github.com/RandomsUsernames/sgrep.git
cd sgrep
cargo install --path .
```

## Quick Start

```bash
# 1. Index your codebase
searchgrep index .

# 2. Search with natural language
searchgrep search "authentication middleware"

# 3. Get AI-powered answers
searchgrep ask "how does the login flow work"

# 4. View codebase structure
searchgrep map
```

## Features

### Semantic Search
Find code by meaning, not just keywords. The query *"handle user login"* will find authentication code even if it doesn't contain those exact words.

```bash
searchgrep search "database connection pooling"
searchgrep search "where are errors logged" --content
```

### AI Answers
Get synthesized answers about your codebase using GPT.

```bash
searchgrep ask "explain the authentication flow"
searchgrep ask "what testing framework is used"
```

### Codebase Map
Get a structural overview of your code - functions, classes, and their relationships.

```bash
searchgrep map              # Full codebase map
searchgrep map src/         # Specific directory
```

### MCP Server
Integrates directly with Claude Code, Cursor, and other MCP-compatible tools.

```bash
searchgrep setup   # Interactive setup for AI tools
```

### Multiple Search Modes

| Mode | Flag | Best For |
|------|------|----------|
| Balanced | *(default)* | General search |
| Code | `--code` | Code-specific queries |
| Hybrid | `--hybrid` | Best quality (slower) |

## AI Tool Integration

### Claude Code / Cursor / Continue

```bash
searchgrep setup  # Interactive MCP setup
```

Or manually add to your MCP config:

```json
{
  "mcpServers": {
    "searchgrep": {
      "command": "searchgrep",
      "args": ["mcp-server"]
    }
  }
}
```

### Skills (Claude, Gemini CLI, OpenCode)

```bash
searchgrep skill         # Interactive setup
searchgrep skill claude  # Claude only
searchgrep skill all     # All tools
```

## Commands

| Command | Description |
|---------|-------------|
| `searchgrep index <path>` | Index a directory |
| `searchgrep search <query>` | Semantic search |
| `searchgrep ask <question>` | AI-powered Q&A |
| `searchgrep map [path]` | Codebase structure map |
| `searchgrep setup` | Configure MCP for AI tools |
| `searchgrep skill [tool]` | Install as skill |
| `searchgrep status` | Show index status |
| `searchgrep config` | Configure settings |

### Search Options

```bash
searchgrep search "query" [options]

  -m, --max-results <n>   Max results (default: 10)
  -c, --content           Show code snippets
  -a, --answer            Generate AI answer
  --code                  Code-optimized model
  --hybrid                Best quality (BGE + CodeRankEmbed)
```

## How It Works

1. **Index** - Files are chunked and converted to vector embeddings locally using BGE or CodeRankEmbed
2. **Store** - Embeddings cached in `~/.searchgrep/`
3. **Search** - Your query is embedded and compared using cosine similarity
4. **Rank** - Results sorted by semantic relevance
5. **Answer** - Optionally, top results sent to GPT for synthesis

## Performance

- **Apple Silicon** - Uses Accelerate framework for fast inference
- **Local Models** - No API calls needed for search
- **Indexing** - ~100 files/minute
- **Search** - ~2s (single model), ~3.5s (hybrid)

## Configuration

```bash
searchgrep config --api-key sk-...   # Set OpenAI key (for answers)
searchgrep config --show             # Show config
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENAI_API_KEY` | For `--answer` and `ask` commands |
| `OPENAI_BASE_URL` | Custom API endpoint |

### Ignore Files

searchgrep respects `.gitignore` and `.searchgrepignore`.

## Examples

```bash
# Find auth code
searchgrep search "user authentication and sessions"

# With code preview
searchgrep search "database queries" --content

# Best quality search
searchgrep search --hybrid "vector embeddings"

# Architecture overview
searchgrep ask "what's the project architecture"

# View structure
searchgrep map src/
```

## Contributing

Contributions welcome! Please open an issue or PR.

## License

[MIT](LICENSE)
