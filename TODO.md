# TODO — claude-sentinel

## IN PROGRESS

(nothing — backlog items complete)

## NEXT

- [ ] Homebrew tap formula
- [ ] 1Password / Doppler integration for API keys

## BACKLOG

- [ ] Team profile sharing (git-based config sync)
- [ ] Raycast / Alfred extension for quick switching
- [ ] Windows installer (.msi)

## DONE

- [x] Plan: architecture, data model, feature set, tech stack
- [x] Bootstrap: repo directories, Cargo.toml workspace, .gitignore, devbox.json, .envrc, Makefile
- [x] CLAUDE.md, project .claude/ skills (cst-domain, rust-tdd)
- [x] cst-core: platform paths (data_dir, profiles_dir, session_dir, etc.)
- [x] cst-core: GlobalConfig (current profile:session), load/save
- [x] cst-core: Profile/ProfileManager CRUD (new/list/rm/clone/rename/import)
- [x] cst-core: Session/SessionManager CRUD + symlink setup for shared config
- [x] cst-core: AuthType (OAuth/Api/Bedrock/Vertex) + all auth modules
- [x] cst-core: OAuth symlink swap (activate/deactivate/import)
- [x] cst-core: API key pool with Keychain storage (add/retrieve/rotate)
- [x] cst-core: AWS Bedrock env injection
- [x] cst-core: Google Vertex AI env injection
- [x] cst-core: 3-layer settings deep merge (global + profile + session → write)
- [x] cst-core: MCP overrides (disable/add per profile)
- [x] cst-core: env.toml overlay (per-session extra env vars)
- [x] cst-core: ProfileHooks (pre/post switch_in/out, non-fatal sh -c execution)
- [x] cst-core: SessionStats (session_count, rate_limit_hits, tokens, cost estimate)
- [x] cst-core: built-in profile templates (pro/max/api/bedrock/vertex)
- [x] cst-core: auto_switch/config.rs — AutoSwitchConfig with fallback chain + schedule + round_robin
- [x] cst-core: auto_switch/detector.rs — rate-limit pattern detection
- [x] cst-core: auto_switch/switch_log.rs — persistent JSONL event log
- [x] cst-core: auto_switch/scheduler.rs — quota reset scheduler + time_until_refill
- [x] cst-core: auto_switch/daemon.rs — tokio async watcher, trigger_switch, switch-back
- [x] cst-cli: full clap CLI (all commands: use/status/list/new/rm/session/daemon/etc.)
- [x] cst-cli: shell-init (zsh/bash/fish/powershell), _env, precmd auto-switch check
- [x] cst-cli: daemon start/stop/status/logs wired to cst-core
- [x] cst-cli: auto-switch configure/log/test-chain/pause
- [x] cst-cli: history run() + why() reading switch log
- [x] cst-cli: ratatui TUI (4 tabs: Profiles, Sessions, Auto-Switch, History)
- [x] apps/desktop: Tauri v2 app — system tray, neubrutalism B&W design system
- [x] apps/desktop: 4-tab window (Profiles, Sessions, Auto-Switch, Stats)
- [x] apps/desktop: Zustand stores, ProfileManager, SessionGrid, StatsPanel
- [x] cst doctor: full 5-group health check (claude binary, data dir, profiles/sessions, daemon, shell)
- [x] cst remaining: token usage, rate-limit timers, cross-profile summary
- [x] cst-cli: cst top live real-time dashboard (htop-style, 1s refresh)
- [x] cst-cli: cst starship — Starship prompt module + --config
- [x] cst-cli: cst tmux — tmux status bar segment + --config
- [x] .github/workflows/ci.yml — test + clippy + release build (ubuntu + macos)
- [x] .github/workflows/release.yml — release binaries on tag push (4 targets)
- [x] docs: USAGE.md, DESIGN.md, CONTRIBUTING.md, ARCHITECTURE.md updated
- [x] cst-core: broadcast.rs — BroadcastSwitch, check_broadcast (TTL-based, per-shell ID tracking)
- [x] cst-cli: cst switch-all <from> <to> — broadcast switch to all open shells
- [x] cst-cli: cst session switch <session> --to <profile> — per-session profile reassignment
- [x] shell-init: precmd hook updated for broadcast check + .cstrc auto-detect (zsh/bash/fish)
- [x] cst-core: auto_detect.rs — .cstrc walk-up + git remote URL pattern matching
- [x] cst-core: history_parser.rs — parse history.jsonl for live token counts
- [x] cst-cli: _auto-detect hidden command + auto-detect-status
- [x] cst remaining: prefers live history.jsonl counts over cached stats.json
- [x] cst-core: RoundRobin config section in AutoSwitchConfig
- [x] 113 unit tests passing; binary runs correctly
