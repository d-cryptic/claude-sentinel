# Claude Sentinel

> **Disclaimer:** Claude Sentinel is an independent, open-source tool. It is not affiliated with, endorsed by, or associated with Anthropic PBC. "Claude" and "Claude Code" are trademarks of Anthropic PBC. This tool interacts with Claude Code through officially documented configuration mechanisms (`CLAUDE_CONFIG_DIR`, `ANTHROPIC_API_KEY`) only.

> Manage multiple Claude Code accounts, profiles, and sessions from one place.

**`cst`** — the CLI tool that manages multiple Claude Code accounts, isolates sessions per project, and lets you switch context instantly between work, personal, and client setups.

## Features

- **All auth types**: Claude Pro/Max (OAuth), API key, AWS Bedrock, Google Vertex AI
- **Multiple accounts**: Separate work, personal, and client accounts in one tool
- **Instant switching**: `cst use work:backend` switches context in milliseconds
- **Session isolation**: Each session gets its own conversation history, settings, and MCP config
- **Scheduled rotation**: Time-based profile activation ("work account 9am–6pm, personal otherwise")
- **Account Pipeline**: Declare your own usage thresholds per profile. Advance to the next account automatically or with `cst next`.
- **Per-session env overlays**: Different `ANTHROPIC_BASE_URL`, model, or settings per session
- **3-layer settings merge**: Global base + profile overrides + session overrides
- **Shared global config**: agents, rules, skills, commands auto-symlinked to all sessions
- **Beautiful TUI**: Interactive profile/session navigator
- **Desktop app**: Menu bar (macOS) / system tray — neubrutalism B&W design
- **Cross-platform**: macOS, Linux, Windows

## Install

```bash
# macOS / Linux
curl -fsSL https://raw.githubusercontent.com/d-cryptic/ccsentinel/main/install.sh | sh

# Cargo
cargo install cst-cli

# First run (imports existing Claude Code config)
cst init
```

## Quick Start

```bash
# Create profiles for different contexts
cst new work --auth oauth       # work Claude subscription
cst new personal --auth oauth   # personal Claude subscription
cst new api-project --auth api  # API key for a specific project

# Switch context instantly
cst use work
cst use work:backend            # named session within work profile

# Schedule automatic switching (time-based, not rate-limit-based)
cst auto-switch configure work  # set active_hours = "09:00-18:00"
cst daemon start

# Set up an account pipeline
cst pipeline configure work   # set threshold + next account
cst next                      # manually advance pipeline
cst pipeline status           # show usage % and ETA

# Check status
cst status
cst top                         # live dashboard

# Interactive TUI
cst
```

## Documentation

- [Install Guide](docs/INSTALL.md)
- [Usage Reference](docs/USAGE.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Auth Types](docs/AUTH.md)
- [Profile Scheduling](docs/AUTO-SWITCH.md)
- [Account Pipeline](docs/PIPELINE.md)

## License

MIT
