#!/usr/bin/env bash
# Alfred Large Type output: show current Claude Sentinel status
set -euo pipefail

CST="$(command -v cst 2>/dev/null || echo /usr/local/bin/cst)"

if [[ ! -x "$CST" ]]; then
    echo "cst not found -- install: cargo install cst-cli"
    exit 0
fi

echo "--- CLAUDE SENTINEL STATUS ---"
echo ""
"$CST" status 2>&1 || echo "(not initialized)"
echo ""
echo "--- QUOTA ---"
echo ""
"$CST" remaining 2>&1 || echo "(no quota data)"
