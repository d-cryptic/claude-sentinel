# Architecture

## Overview

Claude Sentinel is a Cargo workspace with two crates and one Tauri app:

```
cst-core    Shared library — all domain logic, no I/O side effects in tests
cst-cli     CLI binary (cst) — thin layer over cst-core + TUI
desktop     Tauri v2 app — menu bar/tray + management window
```

## Data Flow: Profile Switch

```
cst use work:backend
  │
  ├── shell function calls: cst _env work:backend
  │     ├── cst-core: loads profile "work", session "backend"
  │     ├── cst-core: runs 3-layer settings merge → writes .claude/settings.json
  │     ├── cst-core: validates/creates symlinks (agents/, rules/, skills/, etc.)
  │     ├── cst-core: auth.activate() → OAuth symlink OR sets ANTHROPIC_API_KEY
  │     └── outputs: export CLAUDE_CONFIG_DIR=... CST_CURRENT=...
  │
  └── shell function eval's the exports → env vars set in current shell
```

## Data Directory

```
~/.claude-sentinel/
  config.toml              Current profile:session state
  profiles/
    {name}/
      profile.toml         Metadata (auth_type, created_at, color)
      auth/                Credentials (encrypted or keychain refs)
      sessions/
        {name}/
          .claude/         CLAUDE_CONFIG_DIR target
          settings-override.json
          stats.json
      settings-override.json
      auto-switch.toml
```

## Auth Architecture

Each auth type is handled by a dedicated module in `cst-core::auth`:

| Type | Module | Mechanism |
|------|--------|-----------|
| OAuth | `oauth.rs` | Symlink `~/.claude.json → profile/auth/oauth.json` |
| API key | `apikey.rs` | `ANTHROPIC_API_KEY` from Keychain / AES-GCM encrypted file |
| Bedrock | `bedrock.rs` | `AWS_*` env vars injected from `aws.toml` |
| Vertex AI | `vertex.rs` | `CLOUD_ML_REGION`, `ANTHROPIC_VERTEX_PROJECT_ID` etc. |

## Auto-Switch Daemon

```
cst daemon start → tokio background process
  │
  ├── FileWatcher: notify crate watches history.jsonl
  │     └── on write → detector.rs scans for rate limit patterns
  │
  ├── IPC server: named pipe / Unix socket
  │     └── cst exec wrapper writes rate limit signals here
  │
  ├── Scheduler: chrono-based timer
  │     └── fires auto-switch-back at rate_limit_time + estimate_minutes
  │
  └── On rate limit detected:
        1. key rotation: try next API key in pool
        2. if all keys exhausted: switch to next profile in fallback_chain
        3. write pending-switch file → shell precmd picks it up
        4. macOS notification
        5. schedule switch-back timer
```

## Settings Merge

Three TOML/JSON layers, deep-merged in order (later wins):

1. `~/.claude/settings.json` — global base (managed by main dotfiles)
2. `~/.claude-sentinel/profiles/{p}/settings-override.json` — profile level
3. `~/.claude-sentinel/profiles/{p}/sessions/{s}/settings-override.json` — session level

Result written to `…/sessions/{s}/.claude/settings.json` on each activate.

## Interactive TUI

Running `cst` with no subcommand (or `cst tui`) launches a full-screen ratatui terminal UI.

```
cst (no args) → tui::run()
  │
  ├── AppState::load()
  │     ├── GlobalConfig::load() → current profile:session
  │     ├── ProfileManager::list() → all profiles with auth types
  │     ├── SessionManager::list() → sessions per profile
  │     ├── SchedulerState::load() → active rate-limit timers
  │     └── SwitchLog::last_n(30) → recent switch history
  │
  ├── Render loop (100ms poll interval)
  │     ├── Tab bar: PROFILES | SESSIONS | AUTO-SWITCH | HISTORY
  │     ├── Content area (varies by tab)
  │     └── Footer: active profile + keybinding help
  │
  └── Key handling
        ├── Tab/Right → next tab, BackTab/Left → prev tab
        ├── j/k/Down/Up → navigate lists
        ├── Enter → write pending-switch file + update GlobalConfig
        ├── r/R → refresh all state from disk
        └── q/Q / Ctrl+C → quit
```

### Tab content

| Tab | Layout | Data source |
|-----|--------|-------------|
| PROFILES | 40% list + 60% detail panel | `ProfileManager::list()` |
| SESSIONS | Full-width list with active marker | `SessionManager::list()` for selected profile |
| AUTO-SWITCH | Scheduler entries with countdown | `SchedulerState::load()` |
| HISTORY | Last 30 switch events | `SwitchLog::last_n(30)` |

The Profiles tab detail panel shows: name, auth type, active/inactive status, and session list.

When the user presses Enter on a profile, the TUI writes a pending-switch file via `daemon::write_pending_switch()` and updates `GlobalConfig`. The shell's precmd hook picks up the pending switch on the next prompt.

**Key files**:
- `crates/cst-cli/src/tui/mod.rs` — terminal setup, event loop, key handling
- `crates/cst-cli/src/tui/model.rs` — `AppState`, `Tab` enum, `ProfileRow`, navigation logic
- `crates/cst-cli/src/tui/view.rs` — ratatui widget rendering (tab bar, profiles, sessions, auto-switch, history, footer)

## Live Dashboard (`cst top`)

`cst top` is a read-only, htop-style dashboard that auto-refreshes every 1 second.

```
cst top → top::run()
  │
  ├── TopState::load()
  │     ├── GlobalConfig → active profile:session
  │     ├── daemon_core::is_running() → daemon status
  │     ├── SchedulerState::load() → quota countdown timers
  │     ├── SwitchLog::last_n(5) → recent switch events
  │     └── Per-profile loop:
  │           ProfileManager::list() → profiles
  │           SessionManager::list() → sessions per profile
  │           SessionStats::load() → tokens_in, tokens_out, rate_limit_hits, cost
  │
  ├── Layout (4 vertical sections):
  │     ├── Header (3 lines): braille spinner + "CST TOP" + active profile + daemon indicator
  │     ├── Body (flex): table with PROFILE, SESSION, AUTH, IN, OUT, RATE LIMITS, COST $, LAST USED
  │     ├── Bottom (5 lines): 50/50 horizontal split — QUOTA TIMERS | RECENT SWITCHES
  │     └── Footer (1 line): "q quit  r refresh  (refreshes every 1s)"
  │
  └── Refresh cycle: TopState::refresh() called every 1 second via Instant::elapsed check
```

The header includes a braille spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) that rotates each tick as a liveness indicator. The active profile row is bolded with a `▶` marker. Token counts are formatted as `k` (thousands) or `M` (millions). The daemon status shows `● DAEMON ON` (bold) or `○ DAEMON OFF` (gray).

**Key file**: `crates/cst-cli/src/commands/top.rs`

## Terminal Integrations

Both integrations load `GlobalConfig` and `SchedulerState` to output compact profile status.

### Starship

`cst starship` outputs a single line for Starship's `custom.cst` module:

```
🛡 work:backend           (normal — no rate limits)
🛡 work:backend ⚠ 2h3m   (rate-limit timer active)
(empty output)             (no profile active — module hidden)
```

The `build_quota_indicator()` helper checks `SchedulerState` for entries where `switched_back == false` and returns the first entry's `time_until_refill()`.

`cst starship --config` prints the TOML snippet to add to `starship.toml`.

### tmux

`cst tmux` outputs a tmux-markup string:

```
#[fg=colour255,bold]work:backend#[default]⚠ 2h3m
#[fg=colour240]no profile#[default]
```

`cst tmux --config` prints the `tmux.conf` snippet with `status-right` and `status-interval 5`.

**Key file**: `crates/cst-cli/src/commands/integrations.rs`

## Tauri Desktop App

```
apps/desktop/
  index.html              Vite entry point
  package.json            React + Tauri deps (bun)
  vite.config.ts          Vite config
  tsconfig.json           TypeScript config
  src/
    main.tsx              React entry point
    App.tsx               Root component: tab bar (4 tabs), content router, status bar
    styles/
      neubrutalism.css    Complete design system (see docs/DESIGN.md)
    components/
      ProfileManager.tsx  Split-pane: 260px profile list + flex detail panel
      SessionGrid.tsx     Card grid, click-to-switch
      AutoSwitchConfig.tsx  Daemon control, rate-limit timers, switch log
      StatsPanel.tsx      Token usage table, cost estimates
    store/
      profiles.ts         Zustand store — profiles, active, CRUD actions
      daemon.ts           Zustand store — daemon status, switch log, scheduler
    hooks/                Custom React hooks
  src-tauri/
    src/
      main.rs             Tauri app entry point (calls lib::run())
      lib.rs              Tauri builder: plugins (shell, notification), tray setup, invoke handler
      tray.rs             System tray: left-click toggles window, right-click menu
      commands/
        mod.rs            Command module re-exports
        profiles.rs       list_profiles, get_active, switch_profile, create_profile, delete_profile
        sessions.rs       list_sessions, create_session, delete_session
        daemon.rs         daemon_status, daemon_start, daemon_stop, get_switch_log, get_scheduler_state
        stats.rs          get_stats
```

### Data flow

```
React Component → invoke("command_name", args) → Tauri IPC
  → Rust command handler → cst_core:: function
    → reads/writes ~/.claude-sentinel/
  → returns Result<T> → JSON → React state update → re-render
```

### System tray

The tray menu contains:
- **Active: {profile}:{session}** — disabled informational item
- **Open Sentinel** — shows/focuses the main window
- **Quit** — exits the app

Left-click on the tray icon toggles window visibility (show/hide). The tray is configured with `show_menu_on_left_click(false)` so left-click is reserved for the toggle.

### Plugins

- `tauri-plugin-shell` — run external commands
- `tauri-plugin-notification` — native notifications (macOS/Linux/Windows)

### Frontend refresh

The app polls `list_profiles` and `daemon_status` every 30 seconds via `setInterval` to keep the UI current without requiring WebSocket or event subscriptions.

The Tauri backend is a thin adapter: all logic delegates to `cst-core`. Tauri commands call the same functions used by `cst-cli`.

## Crate Dependencies

```
cst-core  ←── cst-cli
          ←── apps/desktop/src-tauri (via Tauri commands)
```

`cst-core` has no dependency on `cst-cli`. All domain logic is in `cst-core` for testability.
