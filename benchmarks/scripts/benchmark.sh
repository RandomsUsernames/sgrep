#!/bin/bash

# Searchgrep Benchmarking Suite
# Compares searchgrep against ripgrep, ag, grep, and ast-grep

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'
BOLD='\033[1m'

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BENCHMARK_DIR="$(dirname "$SCRIPT_DIR")"
RESULTS_DIR="$BENCHMARK_DIR/results"

print_header() {
    echo ""
    echo -e "${BOLD}${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BOLD}${CYAN}  $1${NC}"
    echo -e "${BOLD}${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
}

# Check tools
check_tools() {
    echo -e "${YELLOW}▸ Installed tools:${NC}"
    for tool in grep rg ag sg searchgrep; do
        if command -v $tool >/dev/null 2>&1; then
            version=$($tool --version 2>/dev/null | head -1 || echo "installed")
            echo -e "  ${GREEN}✓${NC} $tool: $version"
        else
            echo -e "  ${RED}✗${NC} $tool: not found"
        fi
    done
    echo ""
}

# Benchmark text search
benchmark_text_search() {
    local dataset_path="$1"

    print_header "Benchmark 1: Raw Text Search Speed"

    local file_count=$(find "$dataset_path/src" -type f -name "*.rs" 2>/dev/null | wc -l | tr -d ' ')
    echo "Dataset: $dataset_path/src ($file_count Rust files)"
    echo ""

    local patterns="fn
impl
async
struct
Result
error"

    echo "$patterns" | while IFS= read -r pattern; do
        [ -z "$pattern" ] && continue

        echo -e "${CYAN}Pattern: '$pattern'${NC}"
        echo "─────────────────────────────────────────────────────"

        # grep (exclude hidden dirs)
        local start=$(python3 -c "import time; print(time.time())")
        local grep_count=$(grep -rn --include="*.rs" "$pattern" "$dataset_path/src" 2>/dev/null | wc -l | tr -d ' ')
        local end=$(python3 -c "import time; print(time.time())")
        local grep_time=$(python3 -c "print(f'{($end - $start)*1000:.1f}')")
        printf "  %-12s %8s ms  (%s matches)\n" "grep:" "$grep_time" "$grep_count"

        # ripgrep
        start=$(python3 -c "import time; print(time.time())")
        local rg_count=$(rg -n "$pattern" "$dataset_path/src" --type rust 2>/dev/null | wc -l | tr -d ' ')
        end=$(python3 -c "import time; print(time.time())")
        local rg_time=$(python3 -c "print(f'{($end - $start)*1000:.1f}')")
        printf "  %-12s %8s ms  (%s matches)\n" "ripgrep:" "$rg_time" "$rg_count"

        # ag
        start=$(python3 -c "import time; print(time.time())")
        local ag_count=$(ag --rust "$pattern" "$dataset_path/src" 2>/dev/null | wc -l | tr -d ' ')
        end=$(python3 -c "import time; print(time.time())")
        local ag_time=$(python3 -c "print(f'{($end - $start)*1000:.1f}')")
        printf "  %-12s %8s ms  (%s matches)\n" "ag:" "$ag_time" "$ag_count"

        echo ""
    done
}

# Benchmark semantic search
benchmark_semantic() {
    local dataset_path="$1"

    print_header "Benchmark 2: Semantic Search (searchgrep)"

    echo "Queries that regex CAN'T do - understanding intent:"
    echo ""

    # Index first
    echo -e "${YELLOW}Indexing codebase...${NC}"
    searchgrep index "$dataset_path" 2>&1 | grep -E "(Indexed|complete)" | head -2 || true
    echo ""

    local queries="error handling code
async function with await
parse command line arguments
serialize data to json
read file contents"

    echo "$queries" | while IFS= read -r query; do
        [ -z "$query" ] && continue

        local start=$(python3 -c "import time; print(time.time())")
        searchgrep search "$query" --dir "$dataset_path" --limit 5 >/dev/null 2>&1 || true
        local end=$(python3 -c "import time; print(time.time())")
        local duration=$(python3 -c "print(f'{($end - $start)*1000:.0f}')")

        printf "  %-35s %6s ms\n" "\"$query\"" "$duration"
    done
}

# Benchmark AST search
benchmark_ast() {
    local dataset_path="$1"

    print_header "Benchmark 3: AST Pattern Search (ast-grep)"

    if ! command -v sg >/dev/null 2>&1; then
        echo -e "${RED}ast-grep not found${NC}"
        return
    fi

    echo "Structural code patterns:"
    echo ""

    # Simple patterns that ast-grep understands
    local start=$(python3 -c "import time; print(time.time())")
    sg --pattern 'fn $NAME($$$)' "$dataset_path/src" >/dev/null 2>&1 || true
    local end=$(python3 -c "import time; print(time.time())")
    local t1=$(python3 -c "print(f'{($end - $start)*1000:.0f}')")
    printf "  %-35s %6s ms\n" "fn \$NAME(\$\$\$)" "$t1"

    start=$(python3 -c "import time; print(time.time())")
    sg --pattern 'impl $TYPE' "$dataset_path/src" >/dev/null 2>&1 || true
    end=$(python3 -c "import time; print(time.time())")
    local t2=$(python3 -c "print(f'{($end - $start)*1000:.0f}')")
    printf "  %-35s %6s ms\n" "impl \$TYPE" "$t2"

    start=$(python3 -c "import time; print(time.time())")
    sg --pattern 'struct $NAME { $$$FIELDS }' "$dataset_path/src" >/dev/null 2>&1 || true
    end=$(python3 -c "import time; print(time.time())")
    local t3=$(python3 -c "print(f'{($end - $start)*1000:.0f}')")
    printf "  %-35s %6s ms\n" "struct \$NAME { \$\$\$FIELDS }" "$t3"
}

# Summary
summary() {
    print_header "Results Summary"

    cat << 'EOF'
┌─────────────┬───────────────────┬─────────────────────────────────────┐
│ Tool        │ Type              │ Best For                            │
├─────────────┼───────────────────┼─────────────────────────────────────┤
│ ripgrep     │ Regex             │ Fastest text/regex search           │
│ ag          │ Regex             │ Developer-friendly, fast            │
│ grep        │ Regex             │ Universal, always available         │
│ ast-grep    │ AST patterns      │ Structural matching, refactoring    │
│ searchgrep  │ Semantic + AST    │ Natural language queries            │
└─────────────┴───────────────────┴─────────────────────────────────────┘

Key Takeaways:
• ripgrep is 10-100x faster than grep for regex
• searchgrep understands MEANING, not just patterns
• ast-grep finds code by STRUCTURE
• Each tool has its place - they're complementary!

EOF
}

# Main
main() {
    print_header "Searchgrep Benchmark Suite"

    mkdir -p "$RESULTS_DIR"

    check_tools

    local dataset_path="$(dirname "$BENCHMARK_DIR")"

    benchmark_text_search "$dataset_path"
    benchmark_semantic "$dataset_path"
    benchmark_ast "$dataset_path"
    summary

    echo -e "${GREEN}Benchmark complete!${NC}"
}

main "$@"
