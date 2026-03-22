#!/usr/bin/env bash
# Alfred Run Script: perform the actual profile switch (via osascript to current Terminal)
# {query} is the profile:session to switch to
set -euo pipefail

PROFILE_SESSION="${1:-}"
CST="$(command -v cst 2>/dev/null || echo /usr/local/bin/cst)"

if [[ -z "$PROFILE_SESSION" ]]; then
    echo "No profile specified"
    exit 1
fi

# Write a pending-switch so the next prompt picks it up,
# or run cst _env directly if we can't interact with the terminal.
"$CST" use "$PROFILE_SESSION" 2>&1 || true

# Notify via osascript (macOS)
osascript -e "display notification \"Switched to ${PROFILE_SESSION}\" with title \"Claude Sentinel\" sound name \"Glass\"" 2>/dev/null || true

echo "Switched to ${PROFILE_SESSION}"
