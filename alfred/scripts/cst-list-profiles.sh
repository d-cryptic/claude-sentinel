#!/usr/bin/env bash
# Alfred Script Filter: list all profiles as results
set -euo pipefail

CST="$(command -v cst 2>/dev/null || echo /usr/local/bin/cst)"

if [[ ! -x "$CST" ]]; then
    echo '{"items":[{"title":"cst not found","subtitle":"Install: cargo install cst-cli","valid":false}]}'
    exit 0
fi

OUTPUT=$("$CST" list 2>&1 || echo "")
CURRENT=$("$CST" status 2>&1 | grep "^Profile" | awk '{print $3}' || echo "")

python3 - "$OUTPUT" "$CURRENT" <<'PYEOF'
import sys, json

lines = sys.argv[1].strip().splitlines()
current = sys.argv[2].strip()

items = []
for line in lines:
    line = line.strip()
    if not line or line.startswith("-") or line.startswith("No"):
        continue
    is_active = "●" in line or (current and current.split(":")[0] in line)
    name = line.replace("●", "").replace("○", "").strip().split()[0]
    items.append({
        "title": ("▶ " if is_active else "  ") + name,
        "subtitle": "Active" if is_active else f"cst use {name}",
        "arg": name,
        "icon": {"path": "icon.png"},
    })

if not items:
    items = [{"title": "No profiles found", "subtitle": "Run: cst new <name>", "valid": False}]

print(json.dumps({"items": items}))
PYEOF
