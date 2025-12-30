#!/bin/bash
# searchgrep installer
# Usage: curl -fsSL https://raw.githubusercontent.com/RandomsUsernames/Searchgrep/rust-rewrite/install.sh | bash

set -e

REPO="RandomsUsernames/Searchgrep"
INSTALL_DIR="$HOME/.cargo/bin"

echo "⚡ Installing searchgrep..."

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$ARCH" in
    x86_64) ARCH="x86_64" ;;
    arm64|aarch64) ARCH="aarch64" ;;
    *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

case "$OS" in
    darwin) TARGET="${ARCH}-apple-darwin" ;;
    linux) TARGET="${ARCH}-unknown-linux-gnu" ;;
    *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

# Create install directory
mkdir -p "$INSTALL_DIR"

# Download latest release
LATEST=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST}/searchgrep-${TARGET}.tar.gz"

echo "📦 Downloading searchgrep ${LATEST} for ${TARGET}..."

curl -fsSL "$DOWNLOAD_URL" | tar -xz -C "$INSTALL_DIR"
chmod +x "$INSTALL_DIR/searchgrep"

echo "✓ Installed to $INSTALL_DIR/searchgrep"
echo ""

# Setup MCP for Claude Code
CLAUDE_CONFIG_DIR="$HOME/.claude"
MCP_CONFIG="$CLAUDE_CONFIG_DIR/mcp_servers.json"

setup_mcp() {
    mkdir -p "$CLAUDE_CONFIG_DIR"

    if [ -f "$MCP_CONFIG" ]; then
        if grep -q '"searchgrep"' "$MCP_CONFIG"; then
            echo "✓ searchgrep already configured in Claude Code"
            return
        fi
        # Try to add to existing config
        if command -v jq &> /dev/null; then
            jq '.mcpServers.searchgrep = {"command": "searchgrep", "args": ["mcp-server"], "env": {}}' "$MCP_CONFIG" > "$MCP_CONFIG.tmp"
            mv "$MCP_CONFIG.tmp" "$MCP_CONFIG"
            echo "✓ Added searchgrep to Claude Code MCP config"
        else
            echo "Note: Install jq to auto-configure Claude Code, or manually add to $MCP_CONFIG"
        fi
    else
        cat > "$MCP_CONFIG" << 'MCPEOF'
{
  "mcpServers": {
    "searchgrep": {
      "command": "searchgrep",
      "args": ["mcp-server"],
      "env": {}
    }
  }
}
MCPEOF
        echo "✓ Created Claude Code MCP config"
    fi
}

# Auto-setup MCP if Claude Code directory exists or user wants it
if [ -d "$CLAUDE_CONFIG_DIR" ] || [ -f "$HOME/.claude.json" ]; then
    echo "🔧 Setting up Claude Code integration..."
    setup_mcp
fi

echo ""
echo "Make sure $INSTALL_DIR is in your PATH:"
echo "  export PATH=\"\$HOME/.cargo/bin:\$PATH\""
echo ""
echo "Quick start:"
echo "  searchgrep index .           # Index current directory"
echo "  searchgrep search 'query'    # Search your code"
echo "  searchgrep --help            # See all commands"
echo ""
echo "For Claude Code: Restart Claude Code to use searchgrep tools"
