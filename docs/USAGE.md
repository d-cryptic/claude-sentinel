# CLI Usage Reference

Full CLI reference for `cst` -- the Claude Sentinel command-line tool.

## Quick Start

```bash
# First-time setup
cst init

# Add to ~/.zshrc or ~/.bashrc (done automatically by init)
eval "$(cst shell-init)"

# Create your first profile (imports current ~/.claude.json)
cst import --as personal

# Switch to it
cst use personal

# Check status
cst status
```

## Interactive Interfaces

### TUI (Terminal UI)

```bash
cst         # launch TUI (default when no subcommand given)
cst tui     # explicit launch
```

The TUI has 4 tabs:

| Tab | Contents |
|-----|----------|
| PROFILES | Profile list (40%) with detail panel (60%) showing name, auth type, status, sessions |
| SESSIONS | Session list for the selected profile with active marker |
| AUTO-SWITCH | Active rate-limit timers, scheduler entries, countdown to refill |
| HISTORY | Last 30 switch events with timestamps and reasons |

**Keybindings**:

| Key | Action |
|-----|--------|
| `Tab` / `Right` | Next tab |
| `Shift+Tab` / `Left` | Previous tab |
| `j` / `Down` | Move down in list |
| `k` / `Up` | Move up in list |
| `Enter` | Activate selected profile:session |
| `r` | Refresh data from disk |
| `q` / `Ctrl+C` | Quit |

Selecting a profile with `Enter` writes a pending-switch file and updates global config. The shell precmd hook picks it up on the next prompt.

### Live Dashboard (`cst top`)

```bash
cst top
```

htop-style real-time dashboard that auto-refreshes every 1 second. Layout:

```
+----------------------------------------------------------------------+
| CST TOP | ACTIVE: work:backend          DAEMON ON                    |
+----------------------------------------------------------------------+
| PROFILE   SESSION   AUTH   IN     OUT    RATE LIMITS  COST $  LAST   |
| work      backend   oauth  15.2k  4.5k   0           0.0124  03-22  |
| personal  default   oauth  8.1k   2.3k   1           0.0000  03-21  |
| api-work  deploy    api    120.5k 45.2k  3           1.2340  03-22  |
+----------------------------------------------------------------------+
| QUOTA TIMERS            | RECENT SWITCHES                           |
| work -> refills in 2h3m | personal -> work | quota_refill            |
+----------------------------------------------------------------------+
| q quit  r refresh  (refreshes every 1s)                              |
+----------------------------------------------------------------------+
```

Columns in the profile table:
- **PROFILE** / **SESSION** -- name (active row shown with bold marker)
- **AUTH** -- oauth, api, bedrock, vertex
- **IN** / **OUT** -- token counts (formatted as k/M)
- **RATE LIMITS** -- total rate limit hits for this session
- **COST $** -- estimated API cost in USD
- **LAST USED** -- timestamp (MM-DD HH:MM)

**Keybindings**: `q` quit, `r` force refresh.

## Profile Management

### `cst new <name>`

Create a new profile.

```bash
cst new personal                           # OAuth (default)
cst new work --auth oauth --template max   # Max plan template
cst new api-backup --auth api              # API key (stored in Keychain)
cst new bedrock-work --auth bedrock        # AWS Bedrock
cst new vertex-work --auth vertex          # Google Vertex AI
```

**Auth types**: `oauth`, `api`, `bedrock`, `vertex`

**Templates** (applied as base settings override): `pro`, `max`, `api`, `bedrock`, `vertex`

### `cst import [--as <name>]`

Import current `~/.claude.json` as a named profile.

```bash
cst import --as personal
cst import           # defaults to "default"
```

### `cst clone <src> <dst>`

Clone a profile (copies config, not credentials).

```bash
cst clone work work-staging
```

### `cst rm <name>`

Delete a profile and all its sessions.

### `cst rename <old> <new>`

Rename a profile.

### `cst login [<profile>]`

Re-run OAuth login for a profile. Defaults to current profile.

### `cst add-key <profile> [--slot N]`

Add an API key to a profile's key pool. Prompts securely; stores in macOS Keychain / libsecret / WinCred.

```bash
cst add-key api-work            # slot 1 (default)
cst add-key api-work --slot 2   # second key for rotation
```

### `cst validate <profile>`

Validate a profile's config and credentials.

### `cst templates`

List built-in profile templates.

## Switching Profiles

### `cst use <profile>[:<session>]`

Switch to a profile. When called through the shell function (via `eval "$(cst shell-init)"`), updates the current shell's env vars.

```bash
cst use work
cst use work:backend
cst use personal:default
```

### `cst switch-all <from> <to>`

Broadcast a profile switch to **every open shell** currently running profile `from`. Each shell picks it up at its next prompt via the `_cst_check_switch` precmd hook.

```bash
cst switch-all work personal
# ✓ Broadcast queued: work → personal (expires in 5 min)
# All shells on 'work' will switch at their next prompt

# To also switch the current shell immediately:
cst use personal
```

How it works:
1. Writes `~/.claude-sentinel/broadcast-switch.json` with a 5-minute TTL and a unique ID
2. Every shell's precmd hook calls `cst _broadcast-switch $CST_CURRENT $CST_BROADCAST_ID`
3. If the shell's current profile matches `from` and the broadcast hasn't been applied yet, it gets the env exports
4. Each shell tracks `CST_BROADCAST_ID` so it never applies the same broadcast twice
5. The file expires after 5 minutes and is cleaned up automatically

### `cst run <profile:session> -- <cmd>`

Run a command with a specific profile without changing the current shell state.

```bash
cst run work:backend -- claude
cst run api-backup -- claude --dangerously-skip-permissions
```

## Session Management

### `cst session new <name> [--tag <desc>]`

Create a new session under the current profile.

```bash
cst session new backend --tag "API work"
cst session new frontend
```

### `cst session list [<profile>]`

List sessions for a profile (defaults to current).

### `cst session rm <name>`

Delete a session.

### `cst session tag <name> <description>`

Update a session's description tag.

### `cst session archive <name>`

Archive a session (hides from active list, keeps history).

### `cst session switch <session> --to <profile>`

Activate a specific session under a **different profile**. Creates the session in the target profile if it doesn't exist, then writes a pending-switch so the current shell picks it up.

```bash
# Run 'backend' session under api-backup's credentials instead of work's
cst session switch backend --to api-backup
# ✓ Created session 'backend' in profile 'api-backup'  (if new)
# ✓ Session 'backend' switched: work → api-backup

# Apply immediately in the current shell
cst use api-backup:backend
```

Use case: you're on `work:backend` and hit a rate limit. Switch just the `backend` session to `api-backup:backend` to get different credentials, while other sessions stay on `work`.

## Switch History

### `cst history`

Show switch history with reasons (manual, rate-limit, quota-refill, schedule, auto-detect).

### `cst why`

Explain why the current profile is active.

## Status and Diagnostics

### `cst status`

Show current profile:session, auth type, and quota status.

```
Profile : work
Session : backend
Auth    : oauth
Daemon  : running
```

### `cst list`

List all profiles and their sessions.

### `cst remaining`

Show quota usage for the active profile — token counts, estimated cost, rate-limit timers with countdown, and a cross-profile summary.

```
Profile  : work:backend

── Token Usage (current session) ──────────────────────────
  Tokens in   : 15.2k
  Tokens out  : 4.5k
  Total       : 19.7k
  Rate limits : 0
  Last used   : 2026-03-22 19:00 UTC

── All Sessions (work) ─────────────────────────────────────
  Tokens in   : 45.3k
  Tokens out  : 12.1k

── Quota Status ────────────────────────────────────────────
  No active rate limits detected.

── All Profiles ────────────────────────────────────────────
  work [ACTIVE]        in:   45.3k  out:   12.1k
  personal             in:    8.1k  out:    2.3k
  api-backup           in:  120.5k  out:   45.2k  ⚠ 3 rate limits
```

### `cst stats [<profile:session>]`

Show detailed usage statistics. Includes: session count, rate limit hits, tokens in/out, estimated API cost.

```bash
cst stats
cst stats work:backend
```

### `cst doctor`

Full health check — 5 check groups with pass/fail output:

| Group | What is checked |
|-------|----------------|
| Claude Code | `claude` binary in PATH, `~/.claude/`, `~/.claude.json` |
| Data Directory | `~/.claude-sentinel/`, profiles dir, `config.toml` |
| Profiles & Sessions | Profile config, auth files, session `.claude/` symlinks |
| Daemon | PID file health, running state, stale broadcast files |
| Shell Integration | `eval "$(cst shell-init)"` present in rc file |

```bash
cst doctor           # full check, exits 1 on failures
cst validate <name>  # per-profile credential + session detail
```

### `cst sync`

Rebuild symlinks from `~/.claude/` to all sessions.

## Auto-Switch Daemon

### Daemon lifecycle

```bash
cst daemon start
cst daemon stop
cst daemon restart
cst daemon status
cst daemon logs              # tail daemon log output
```

### Auto-switch configuration

```bash
cst auto-switch configure work        # interactive wizard
cst auto-switch log                   # history of all auto-switches
cst auto-switch test work             # dry-run: show what would happen
```

### Pause auto-switching

```bash
cst pause                    # pause indefinitely
cst pause --minutes 60       # pause for 1 hour
```

See [AUTO-SWITCH.md](AUTO-SWITCH.md) for the full configuration reference.

## Shell Integration

### `cst shell-init [--shell <shell>]`

Outputs shell init code. Add to your rc file:

```bash
# ~/.zshrc or ~/.bashrc
eval "$(cst shell-init)"
```

Supports: `zsh`, `bash`, `fish`, `powershell`. Auto-detects if `--shell` is omitted.

The init code installs:
1. A `cst` shell function that wraps `cst use` to eval env exports in the current shell
2. A `precmd` hook that checks for pending daemon-initiated switches
3. `CST_CURRENT` variable showing active `profile:session`

### Starship Prompt Module

```bash
cst starship                # output for Starship custom module
cst starship --config       # print starship.toml config snippet
```

Shows `profile:session` and a warning indicator when rate-limited (e.g., `work:backend !! 2h3m`).

Add to `~/.config/starship.toml`:

```toml
[custom.cst]
command = "cst starship"
when = true
format = "[$output]($style) "
style = "bold white"
shell = ["sh"]
```

### tmux Status Bar Segment

```bash
cst tmux                    # output for tmux status-right
cst tmux --config           # print tmux.conf config snippet
```

Shows the active profile:session with tmux color markup and quota warnings.

Add to `~/.config/tmux/tmux.conf`:

```
set -g status-right "#(cst tmux) | %H:%M"
set -g status-interval 5
```

## Tab Completions

### `cst completions <shell>`

Generate shell tab completions.

```bash
cst completions zsh   > ~/.zfunc/_cst
cst completions bash  > /usr/local/etc/bash_completion.d/cst
cst completions fish  > ~/.config/fish/completions/cst.fish
```

## First-Run Setup

### `cst init [--yes] [--shell <s>] [--no-daemon]`

Interactive first-run wizard. Detects existing Claude Code install, imports `~/.claude.json`, configures shell, and optionally starts the daemon.

```bash
cst init                              # interactive
cst init --yes --shell zsh            # non-interactive, accept defaults
cst init --yes --shell zsh --no-daemon   # skip daemon start
```

## Environment Variables

| Variable | Set by | Purpose |
|----------|--------|---------|
| `CLAUDE_CONFIG_DIR` | `cst use` | Points Claude Code to per-session config dir |
| `CST_CURRENT` | `cst use` | Current `profile:session` (for prompts and scripts) |
| `ANTHROPIC_API_KEY` | `cst use` (api profiles) | API key loaded from Keychain |
| `AWS_ACCESS_KEY_ID` | `cst use` (bedrock) | AWS credential for Bedrock |
| `AWS_SECRET_ACCESS_KEY` | `cst use` (bedrock) | AWS credential for Bedrock |
| `AWS_DEFAULT_REGION` | `cst use` (bedrock) | AWS region |
| `ANTHROPIC_MODEL` | `cst use` (bedrock) | Bedrock model ID |
| `CLAUDE_CODE_USE_VERTEX` | `cst use` (vertex) | Enables Vertex AI mode |
| `ANTHROPIC_VERTEX_PROJECT_ID` | `cst use` (vertex) | GCP project ID |
| `CLOUD_ML_REGION` | `cst use` (vertex) | GCP region |
| `RUST_LOG` | User | Control log verbosity (e.g., `RUST_LOG=cst=debug`) |
