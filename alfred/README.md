# Claude Sentinel -- Alfred Workflow

Alfred script commands for [claude-sentinel](https://github.com/d-cryptic/claude-sentinel).

## Install

1. Open Alfred Preferences -> Workflows
2. Click `+` -> Import Workflow
3. Import `claude-sentinel.alfredworkflow` (see Releases)

Or build from source: the scripts in this directory are the workflow scripts.

## Commands

| Keyword | Description |
|---------|-------------|
| `cst` | Switch Claude profile (type profile name after keyword) |
| `cst-status` | Show active profile, session, daemon status |
| `cst-remaining` | Token counts, cost estimate, quota timers |
| `cst-profiles` | List all profiles as Alfred results |

## Requirements

- `cst` binary in PATH: `cargo install cst-cli` or via Homebrew tap
- Alfred 5 with Powerpack (for Script Filter)
