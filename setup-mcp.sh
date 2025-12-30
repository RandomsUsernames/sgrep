#!/bin/bash
# Setup searchgrep as an MCP server for Claude Code
# Usage: ./setup-mcp.sh

set -e

CLAUDE_CONFIG_DIR="$HOME/.claude"
MCP_CONFIG="$CLAUDE_CONFIG_DIR/mcp_servers.json"

echo "Setting up searchgrep MCP server for Claude Code..."

# Check if searchgrep is installed
if ! command -v searchgrep &> /dev/null; then
    echo "Error: searchgrep is not installed."
    echo "Install it first with: cargo install --path ."
    echo "Or: curl -fsSL https://raw.githubusercontent.com/RandomsUsernames/Searchgrep/rust-rewrite/install.sh | bash"
    exit 1
fi

SEARCHGREP_PATH=$(which searchgrep)
echo "Found searchgrep at: $SEARCHGREP_PATH"

# Create .claude directory if it doesn't exist
mkdir -p "$CLAUDE_CONFIG_DIR"

# Create or update mcp_servers.json
if [ -f "$MCP_CONFIG" ]; then
    echo "Updating existing $MCP_CONFIG..."

    # Check if searchgrep is already configured
    if grep -q '"searchgrep"' "$MCP_CONFIG"; then
        echo "searchgrep is already configured in MCP servers."
        echo "Current configuration:"
        cat "$MCP_CONFIG"
        exit 0
    fi

    # Add searchgrep to existing config using jq if available, otherwise manual
    if command -v jq &> /dev/null; then
        jq '.mcpServers.searchgrep = {"command": "'"$SEARCHGREP_PATH"'", "args": ["mcp-server"], "env": {}}' "$MCP_CONFIG" > "$MCP_CONFIG.tmp"
        mv "$MCP_CONFIG.tmp" "$MCP_CONFIG"
    else
        echo "Warning: jq not found, please manually add searchgrep to $MCP_CONFIG"
        echo ""
        echo "Add this to your mcpServers:"
        echo '    "searchgrep": {'
        echo '      "command": "'"$SEARCHGREP_PATH"'",'
        echo '      "args": ["mcp-server"],'
        echo '      "env": {}'
        echo '    }'
        exit 0
    fi
else
    echo "Creating $MCP_CONFIG..."
    cat > "$MCP_CONFIG" << EOF
{
  "mcpServers": {
    "searchgrep": {
      "command": "$SEARCHGREP_PATH",
      "args": ["mcp-server"],
      "env": {}
    }
  }
}
EOF
fi

echo ""
echo "MCP server configured successfully!"
echo ""
echo "Configuration at $MCP_CONFIG:"
cat "$MCP_CONFIG"
echo ""
echo "Restart Claude Code to use searchgrep tools:"
echo "  - semantic_search: Search code using natural language"
echo "  - index_directory: Index a directory for search"
