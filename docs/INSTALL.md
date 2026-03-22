# Installation

## Quick Install

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/d-cryptic/claude-sentinel/main/install.sh | sh
```

### Windows (PowerShell)

```powershell
iwr https://raw.githubusercontent.com/d-cryptic/claude-sentinel/main/install.ps1 | iex
```

### Via Cargo

```bash
cargo install cst-cli
```

## Pre-Built Binaries (GitHub Releases)

Each tagged release publishes ready-to-run binaries via GitHub Actions:

| Platform | Archive |
|----------|---------|
| macOS Apple Silicon (M1/M2/M3) | `cst-vX.Y.Z-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `cst-vX.Y.Z-x86_64-apple-darwin.tar.gz` |
| Linux x86_64 | `cst-vX.Y.Z-x86_64-linux.tar.gz` |
| Linux ARM64 | `cst-vX.Y.Z-aarch64-linux.tar.gz` |

Each archive ships a `.sha256` checksum file. Download and install manually:

```bash
# Example: macOS Apple Silicon
VERSION=v0.1.0
curl -LO https://github.com/d-cryptic/claude-sentinel/releases/download/${VERSION}/cst-${VERSION}-aarch64-apple-darwin.tar.gz
curl -LO https://github.com/d-cryptic/claude-sentinel/releases/download/${VERSION}/cst-${VERSION}-aarch64-apple-darwin.tar.gz.sha256

# Verify checksum
sha256sum -c cst-${VERSION}-aarch64-apple-darwin.tar.gz.sha256

# Install
tar xzf cst-${VERSION}-aarch64-apple-darwin.tar.gz
sudo mv cst /usr/local/bin/cst
cst --version
```

## First Run

```bash
cst init
```

The wizard will:
1. Detect your OS and Claude Code installation
2. Import your existing `~/.claude.json` as the `default` profile
3. Configure your shell (adds `eval "$(cst shell-init)"` to your rc file)
4. Optionally start the auto-switch daemon
5. Optionally open the desktop app

## Non-Interactive Install

```bash
cst init --yes --shell zsh --no-daemon --no-app
```

## Desktop App

The Claude Sentinel desktop app provides a menu bar (macOS) or system tray (Linux/Windows) for quick profile switching.

**macOS**: Installed automatically with `cst init --app`
**Linux/Windows**: Download from GitHub Releases

## Requirements

- Claude Code (`claude`) must be installed
- macOS 12+, Linux (glibc 2.31+), or Windows 10+
- For Bedrock: AWS CLI configured
- For Vertex AI: Google Cloud CLI configured

## Uninstall

```bash
cst uninstall           # removes ~/.claude-sentinel/ and shell init
cargo uninstall cst-cli  # removes binary
```
