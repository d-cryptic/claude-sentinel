#!/usr/bin/env bash
# Alfred Large Type: show token quota remaining
set -euo pipefail

CST="$(command -v cst 2>/dev/null || echo /usr/local/bin/cst)"

if [[ ! -x "$CST" ]]; then
    echo "cst not found"
    exit 0
fi

"$CST" remaining 2>&1
