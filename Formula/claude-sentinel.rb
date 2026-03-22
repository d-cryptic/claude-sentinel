# typed: false
# frozen_string_literal: true

# Homebrew formula for claude-sentinel (cst).
#
# Install via tap:
#   brew tap d-cryptic/claude-sentinel
#   brew install claude-sentinel
#
# Or directly:
#   brew install d-cryptic/claude-sentinel/claude-sentinel
class ClaudeSentinel < Formula
  desc "Intelligent Claude Code account, profile, and session manager"
  homepage "https://github.com/d-cryptic/claude-sentinel"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/d-cryptic/claude-sentinel/releases/download/v#{version}/cst-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_AARCH64_MACOS_SHA256"
    else
      url "https://github.com/d-cryptic/claude-sentinel/releases/download/v#{version}/cst-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_X86_64_MACOS_SHA256"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/d-cryptic/claude-sentinel/releases/download/v#{version}/cst-v#{version}-aarch64-linux.tar.gz"
      sha256 "PLACEHOLDER_AARCH64_LINUX_SHA256"
    else
      url "https://github.com/d-cryptic/claude-sentinel/releases/download/v#{version}/cst-v#{version}-x86_64-linux.tar.gz"
      sha256 "PLACEHOLDER_X86_64_LINUX_SHA256"
    end
  end

  def install
    bin.install "cst"
  end

  def post_install
    # Generate shell completions
    (bash_completion/"cst").write Utils.safe_popen_read(bin/"cst", "completions", "bash")
    (zsh_completion/"_cst").write Utils.safe_popen_read(bin/"cst", "completions", "zsh")
    (fish_completion/"cst.fish").write Utils.safe_popen_read(bin/"cst", "completions", "fish")
  end

  def caveats
    <<~EOS
      To get started, run:
        cst init

      Add to your shell rc file (~/.zshrc or ~/.bashrc):
        eval "$(cst shell-init)"

      Documentation:
        https://github.com/d-cryptic/claude-sentinel/blob/main/docs/USAGE.md
    EOS
  end

  test do
    assert_match "claude-sentinel", shell_output("#{bin}/cst --version")
    assert_match "cst", shell_output("#{bin}/cst --help")
  end
end
