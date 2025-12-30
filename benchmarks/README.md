# Searchgrep Benchmarks

Comparing searchgrep against other popular code search tools.

## Tools Compared

| Tool | Version | Type | Description |
|------|---------|------|-------------|
| grep | 2.6.0 | Regex | Universal, always available |
| ripgrep | 14.1.1 | Regex | Fast parallel regex search |
| ag | 2.2.0 | Regex | The Silver Searcher |
| ast-grep | 0.40.3 | AST | Structural pattern matching |
| searchgrep | 0.1.0 | Semantic | AI-powered semantic search |

## Benchmark Results

### Test 1: Raw Text Search Speed

**Dataset:** searchgrep-rs source code (30 Rust files)

| Pattern | grep | ripgrep | ag |
|---------|------|---------|-----|
| `fn` | 60.6 ms (237) | 80.2 ms (237) | 125.4 ms (237) |
| `impl` | 59.9 ms (75) | 77.9 ms (75) | 126.4 ms (75) |
| `async` | 56.3 ms (35) | 79.2 ms (35) | 124.3 ms (35) |
| `struct` | 58.7 ms (100) | 76.8 ms (100) | 120.5 ms (105) |
| `Result` | 57.6 ms (206) | 77.3 ms (206) | 130.4 ms (206) |
| `error` | 127.9 ms (68) | 89.1 ms (68) | 133.0 ms (74) |

**Note:** On this small dataset, grep and ripgrep perform similarly. ripgrep's advantage becomes more pronounced on larger codebases (10-100x faster).

### Test 2: Semantic Search (searchgrep only)

These queries demonstrate searchgrep's unique capability - understanding **intent**, not just patterns:

| Query | What It Finds |
|-------|---------------|
| "error handling code" | Functions that handle errors, Result types, error propagation |
| "async function with await" | Async functions that actually await, not just the keyword |
| "parse command line arguments" | CLI argument parsing logic |
| "serialize data to json" | JSON serialization code |
| "read file contents" | File I/O operations |

**Regex tools cannot do this.** Try searching `grep "error handling"` - you'll get nothing useful.

### Test 3: AST Pattern Search (ast-grep)

Structural patterns that match code shape:

| Pattern | Description |
|---------|-------------|
| `fn $NAME($$$)` | Any function definition |
| `impl $TYPE` | Any impl block |
| `struct $NAME { $$$FIELDS }` | Any struct with fields |

## When to Use Each Tool

| Use Case | Best Tool | Why |
|----------|-----------|-----|
| Quick keyword search | **ripgrep** | Fastest for regex, respects .gitignore |
| Find by meaning | **searchgrep** | Understands what you're looking for |
| Code refactoring | **ast-grep** | Matches code structure |
| Universal fallback | **grep** | Available everywhere |
| Interactive search | **ag** | Great defaults for developers |

## Key Insights

1. **ripgrep** is the king of raw text search - 10-100x faster than grep on large codebases
2. **searchgrep** fills a different niche - it understands code semantics
3. **ast-grep** is invaluable for large-scale refactoring
4. These tools are **complementary**, not competing

## Running Benchmarks

```bash
cd benchmarks/scripts
./benchmark.sh
```

## Machine Info

- Platform: macOS (Darwin)
- Architecture: arm64 (Apple Silicon)
