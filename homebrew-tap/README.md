# Homebrew Tap Setup

The Homebrew formula is at `Formula/claude-sentinel.rb`.

## Create the tap repository

```bash
# Create github.com/d-cryptic/homebrew-claude-sentinel
gh repo create d-cryptic/homebrew-claude-sentinel --public --description "Homebrew tap for Claude Sentinel"

# Push the formula
mkdir -p /tmp/homebrew-tap/Formula
cp Formula/claude-sentinel.rb /tmp/homebrew-tap/Formula/
cd /tmp/homebrew-tap
git init && git add . && git commit -m "feat: add claude-sentinel formula v0.1.0"
git remote add origin git@github.com:d-cryptic/homebrew-claude-sentinel.git
git push -u origin main
```

## Update sha256 checksums after release

After `git push --tags` triggers the release workflow:
1. Download the macOS Apple Silicon binary from GitHub Releases
2. Run `sha256sum cst-v0.1.0-aarch64-apple-darwin.tar.gz`
3. Update `Formula/claude-sentinel.rb` with the real checksums
4. Push to the tap repo

## Test locally before publishing

```bash
brew tap d-cryptic/claude-sentinel /tmp/homebrew-tap
brew install claude-sentinel
cst --version
```
