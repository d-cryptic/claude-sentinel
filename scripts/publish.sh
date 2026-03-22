#!/usr/bin/env bash
# Publish claude-sentinel to crates.io
# Usage: ./scripts/publish.sh [--dry-run]
set -euo pipefail

DRY_RUN=""
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN="--dry-run"
    echo "=== DRY RUN mode ==="
fi

echo "=== Pre-publish checks ==="
cargo fmt --all -- --check
cargo clippy -p cst-core -p cst-cli -- -D warnings
cargo test -p cst-core --lib
cargo test -p cst-cli --lib

echo ""
echo "=== Publishing cst-core ==="
cargo publish -p cst-core $DRY_RUN

if [[ -z "$DRY_RUN" ]]; then
    echo "Waiting 30s for crates.io to index cst-core..."
    sleep 30
fi

echo ""
echo "=== Publishing cst-cli ==="
cargo publish -p cst-cli $DRY_RUN

echo ""
echo "=== Done ==="
if [[ -z "$DRY_RUN" ]]; then
    echo "Published! View at:"
    echo "  https://crates.io/crates/cst-core"
    echo "  https://crates.io/crates/cst-cli"
fi
