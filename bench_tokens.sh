#!/bin/bash
# Token Reduction Benchmark for searchgrep
# Measures how many tokens we save by using searchgrep vs sending full codebase

set -e

cd /Users/kanayochukew/extras/stuff/searchgrep-rs

# Approximate tokens (1 token ≈ 4 chars for code)
chars_to_tokens() {
    echo $(( $1 / 4 ))
}

echo "═══════════════════════════════════════════════════════════════"
echo "  SEARCHGREP TOKEN REDUCTION BENCHMARK"
echo "═══════════════════════════════════════════════════════════════"
echo ""

# 1. Calculate baseline (full codebase)
echo "📊 FULL CODEBASE (baseline - what you'd send without searchgrep)"

# Count all source files
total_files=$(find src -name "*.rs" -o -name "*.ts" 2>/dev/null | wc -l | tr -d ' ')
total_lines=$(find src -name "*.rs" -o -name "*.ts" -exec cat {} \; 2>/dev/null | wc -l | tr -d ' ')
total_chars=$(find src -name "*.rs" -o -name "*.ts" -exec cat {} \; 2>/dev/null | wc -c | tr -d ' ')
baseline_tokens=$(chars_to_tokens $total_chars)

echo "   Files:  $total_files"
echo "   Lines:  $total_lines"
echo "   Chars:  $total_chars"
echo "   Tokens: ~$baseline_tokens (estimated)"
echo ""

# 2. Calculate compiled map size
echo "📦 COMPILED MAP (searchgrep compile --show)"
compiled_output=$(cargo run --release -- compile --show 2>/dev/null)
compiled_chars=$(echo "$compiled_output" | wc -c | tr -d ' ')
compiled_tokens=$(chars_to_tokens $compiled_chars)
echo "   Chars:  $compiled_chars"
echo "   Tokens: ~$compiled_tokens"
compile_reduction=$(echo "scale=1; (1 - $compiled_tokens / $baseline_tokens) * 100" | bc)
echo "   Reduction: ${compile_reduction}%"
echo ""

# 3. Test queries with search
echo "═══════════════════════════════════════════════════════════════"
echo "  QUERY-BY-QUERY TOKEN SAVINGS (using 'search' command)"
echo "═══════════════════════════════════════════════════════════════"
echo ""

queries=(
    "how does embedding work"
    "error handling in search"
    "BM25 scoring algorithm"
    "file indexing process"
    "MCP server implementation"
)

total_search_tokens=0
query_count=0

for query in "${queries[@]}"; do
    echo "🔍 Query: \"$query\""

    # Run searchgrep and capture JSON output
    search_output=$(cargo run --release -- search "$query" -m 5 --content --json 2>/dev/null | grep -v "^Loading\|^✓" || echo "{}")

    # Get character count of results
    search_chars=$(echo "$search_output" | wc -c | tr -d ' ')
    search_tokens=$(chars_to_tokens $search_chars)

    # Calculate reduction
    if [ $baseline_tokens -gt 0 ]; then
        reduction=$(echo "scale=1; (1 - $search_tokens / $baseline_tokens) * 100" | bc)
    else
        reduction=0
    fi

    echo "   Searchgrep tokens: ~$search_tokens"
    echo "   Full codebase:     ~$baseline_tokens"
    echo "   Token reduction:   ${reduction}% fewer tokens needed"
    echo ""

    total_search_tokens=$((total_search_tokens + search_tokens))
    query_count=$((query_count + 1))
done

# 4. Summary
avg_search_tokens=$((total_search_tokens / query_count))
avg_reduction=$(echo "scale=1; (1 - $avg_search_tokens / $baseline_tokens) * 100" | bc)

echo "═══════════════════════════════════════════════════════════════"
echo "  SUMMARY"
echo "═══════════════════════════════════════════════════════════════"
echo ""
echo "  📊 Token Comparison:"
echo "     Full codebase:     ~$baseline_tokens tokens"
echo "     Compiled map:      ~$compiled_tokens tokens (${compile_reduction}% reduction)"
echo "     Avg search query:  ~$avg_search_tokens tokens (${avg_reduction}% reduction)"
echo ""

# Cost calculation (GPT-4 @ $30/1M input tokens)
cost_per_token="0.00003"
full_cost=$(echo "scale=5; $baseline_tokens * $cost_per_token" | bc)
compiled_cost=$(echo "scale=5; $compiled_tokens * $cost_per_token" | bc)
search_cost=$(echo "scale=5; $avg_search_tokens * $cost_per_token" | bc)
saved_compile=$(echo "scale=5; $full_cost - $compiled_cost" | bc)
saved_search=$(echo "scale=5; $full_cost - $search_cost" | bc)

echo "  💰 Cost per query (GPT-4 @ \$30/1M tokens):"
echo "     Full codebase: \$$full_cost"
echo "     Compiled map:  \$$compiled_cost (save \$$saved_compile)"
echo "     Search query:  \$$search_cost (save \$$saved_search)"
echo ""

# Per 1000 queries
echo "  📈 Per 1,000 queries:"
full_1k=$(echo "scale=2; $full_cost * 1000" | bc)
compiled_1k=$(echo "scale=2; $compiled_cost * 1000" | bc)
search_1k=$(echo "scale=2; $search_cost * 1000" | bc)
saved_compile_1k=$(echo "scale=2; $saved_compile * 1000" | bc)
saved_search_1k=$(echo "scale=2; $saved_search * 1000" | bc)

echo "     Full codebase: \$$full_1k"
echo "     Compiled map:  \$$compiled_1k (save \$$saved_compile_1k)"
echo "     Search query:  \$$search_1k (save \$$saved_search_1k)"
echo ""
echo "═══════════════════════════════════════════════════════════════"
