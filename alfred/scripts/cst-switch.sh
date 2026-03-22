#!/usr/bin/env bash
# Alfred Script Filter: lists profiles for switching
# Input: {query}
set -euo pipefail

CST="$(command -v cst 2>/dev/null || echo /usr/local/bin/cst)"

if [[ ! -x "$CST" ]]; then
    echo '{"items":[{"title":"cst not found","subtitle":"Install: cargo install cst-cli","valid":false}]}'
    exit 0
fi

QUERY="${1:-}"
PROFILES_JSON=$("$CST" list --json 2>/dev/null || echo "[]")

# Build Alfred JSON items from profile list
python3 - "$QUERY" "$PROFILES_JSON" "$CST" <<'PYEOF'
import json, sys, subprocess

query = sys.argv[1].lower()
cst_bin = sys.argv[3]
try:
    profiles_raw = json.loads(sys.argv[2])
except Exception:
    profiles_raw = []

# Fallback: parse text output
if not profiles_raw:
    try:
        out = subprocess.check_output([cst_bin, "list"], text=True, timeout=5)
        profiles_raw = [{"name": l.strip().split()[0], "sessions": []}
                        for l in out.splitlines() if l.strip()]
    except Exception:
        profiles_raw = []

items = []
for p in profiles_raw:
    name = p.get("name", str(p)) if isinstance(p, dict) else str(p)
    sessions = p.get("sessions", ["default"]) if isinstance(p, dict) else ["default"]
    if query and query not in name.lower():
        continue
    items.append({
        "title": name,
        "subtitle": f"Switch to {name}:default -- {len(sessions)} session(s)",
        "arg": name,
        "autocomplete": name,
        "icon": {"path": "icon.png"},
    })
    for s in sessions:
        if isinstance(s, dict):
            s = s.get("name", str(s))
        if s == "default":
            continue
        ps = f"{name}:{s}"
        if query and query not in ps.lower():
            continue
        items.append({
            "title": ps,
            "subtitle": f"Switch to session {s} in profile {name}",
            "arg": ps,
            "autocomplete": ps,
            "icon": {"path": "icon.png"},
        })

print(json.dumps({"items": items if items else [{"title": f"No profiles matching '{query}'", "valid": False}]}))
PYEOF
