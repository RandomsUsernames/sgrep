class Searchgrep < Formula
  desc "Semantic grep for the AI era - natural language code search with MCP server"
  homepage "https://github.com/RandomsUsernames/Searchgrep"
  version "2.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/RandomsUsernames/Searchgrep/releases/download/v2.1.0/searchgrep-aarch64-apple-darwin.tar.gz"
      sha256 "fcfe295409214b0955d9f7ee95d4947bb52fdc9c0f40c43184013ff8d75513e3"
    end
    on_intel do
      url "https://github.com/RandomsUsernames/Searchgrep/releases/download/v2.1.0/searchgrep-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_X86_SHA"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/RandomsUsernames/Searchgrep/releases/download/v2.1.0/searchgrep-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_LINUX_ARM_SHA"
    end
    on_intel do
      url "https://github.com/RandomsUsernames/Searchgrep/releases/download/v2.1.0/searchgrep-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_LINUX_X86_SHA"
    end
  end

  def install
    bin.install "searchgrep"
  end

  def caveats
    <<~EOS
      searchgrep has been installed!

      Quick setup for AI tools:
        searchgrep setup          # Interactive MCP setup for 15+ AI tools
        searchgrep skill          # Install as skill for OpenCode

      Quick start:
        searchgrep index .        # Index current directory
        searchgrep search "query" # Semantic search
        searchgrep ask "question" # Ask about your code
        searchgrep --help         # See all options
    EOS
  end

  test do
    assert_match "searchgrep", shell_output("#{bin}/searchgrep --version")
  end
end
