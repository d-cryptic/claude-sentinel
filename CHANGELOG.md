# Changelog

All notable changes to claude-sentinel are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased] — v0.1.0

### Added

#### Core library (`cst-core`)
- **Profile management** — CRUD, clone, rename, import, templates (pro/max/api/bedrock/vertex)
- **Session management** — CRUD, tag, archive, symlink setup for shared global config
- **Auth modules** — OAuth symlink swap, API key pool (Keychain/AES-GCM), AWS Bedrock env injection, Google Vertex AI env injection
- **3-layer settings merge** — global + profile + session overrides deep-merged on activate
- **MCP overrides** — per-profile add/disable of MCP servers vs global `~/.claude.json`
- **env.toml overlay** — per-session extra environment variables
- **ProfileHooks** — pre/post switch_in/out lifecycle hooks (non-fatal `sh -c`)
- **SessionStats** — token counts, cost estimates, rate-limit hit tracking
- **Auto-switch daemon** — tokio async file watcher, rate-limit pattern detection (10 patterns), fallback chain, quota reset scheduler, switch-back timer
- **Switch log** — append-only JSONL event log with reason, from/to, timestamp
- **Broadcast switch** — TTL-based broadcast file for signalling all open shells to switch profiles; per-shell `CST_BROADCAST_ID` prevents duplicate application
- **Platform paths** — cross-platform data/profile/session/claude-config dirs via `dirs` crate

#### CLI (`cst-cli`)
- **Full command surface**: `use`, `status`, `list`, `remaining`, `top`, `history`, `why`, `new`, `import`, `clone`, `rm`, `rename`, `login`, `add-key`, `session *`, `daemon *`, `auto-switch *`, `pause`, `run`, `sync`, `stats`, `doctor`, `validate`, `shell-init`, `starship`, `tmux`, `completions`, `templates`, `init`
- **`cst switch-all <from> <to>`** — broadcast profile switch to all open shells
- **`cst session switch <session> --to <profile>`** — reassign a session to a different profile
- **`cst top`** — htop-style live dashboard (1s refresh): token usage table, quota timers, recent switch events, daemon status, braille spinner
- **`cst doctor`** — 5-group health check: Claude Code install, data dir, profiles/sessions symlinks, daemon PID health, shell rc integration; exits 1 on hard failures
- **`cst remaining`** — token usage for active session + profile totals + rate-limit countdown timers + cross-profile summary table
- **`cst starship`** — Starship custom module output with quota warning; `--config` prints TOML snippet
- **`cst tmux`** — tmux status-right segment; `--config` prints config snippet
- **ratatui TUI** — 4-tab interactive navigator (Profiles, Sessions, Auto-Switch, History); Enter activates via pending-switch; `r` refreshes

#### Shell integration
- `eval "$(cst shell-init)"` — installs `cst` shell function + `_cst_check_switch` precmd hook
- **Precmd hook** — checks both one-shot pending-switch (daemon-initiated) and broadcast-switch file
- Supports: zsh, bash, fish, PowerShell
- `CST_BROADCAST_ID` per-shell env var prevents re-applying the same broadcast

#### Desktop app (`apps/desktop`)
- Tauri v2 app with system tray — left-click toggles window, right-click menu
- 4-tab window: Profiles, Sessions, Auto-Switch, Stats
- **Neubrutalism design system** — pure `#000`/`#fff`, 2-4px solid borders, 4px offset shadows, zero border-radius, monospace throughout, ALL-CAPS labels
- Zustand stores for profiles and daemon state
- Tauri commands wrap `cst-core` for all CRUD and daemon operations

#### Infrastructure
- Cargo workspace: `cst-core`, `cst-cli`, `apps/desktop/src-tauri`
- GitHub Actions CI: test + clippy + release build matrix (ubuntu + macos, x86_64 + aarch64)
- devbox + direnv dev environment
- 87 unit tests

#### Documentation
- `docs/ARCHITECTURE.md` — crate structure, data flows, daemon design, TUI, Tauri app
- `docs/AUTH.md` — all 4 auth types
- `docs/AUTO-SWITCH.md` — daemon config, rate-limit patterns, monitoring with `cst top`
- `docs/CONTRIBUTING.md` — dev setup, TDD workflow, CI pipeline, commit conventions
- `docs/DESIGN.md` — complete neubrutalism design system spec
- `docs/INSTALL.md` — installation guide
- `docs/USAGE.md` — full CLI reference with examples and ASCII layout diagrams

---

## [0.0.0] — 2026-03-22

- Initial commit: Cargo workspace scaffold, directory structure, devbox/direnv config
