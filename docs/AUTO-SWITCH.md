# Profile Scheduling & Automatic Context Switching

> For usage-based account switching, see [PIPELINE.md](PIPELINE.md).

> **Disclaimer:** Claude Sentinel is an independent, open-source tool. It is not affiliated with, endorsed by, or associated with Anthropic PBC. "Claude" and "Claude Code" are trademarks of Anthropic PBC.

The Claude Sentinel daemon enables **time-based automatic profile switching** — for example, activating your work account during business hours and your personal account in the evenings.

This feature is designed for **context organization**, not quota management. Each profile you add must be a Claude account you legitimately own and control.

## How It Works

```
1. cst daemon start
2. Daemon evaluates each profile's schedule (active_hours / timezone)
3. When the current time enters a profile's active window, the daemon
   writes a pending-switch file
4. Shell precmd hook picks up the pending switch on the next prompt
5. Optional macOS / Linux / Windows notification on switch
```

## Configuration

```toml
# ~/.claude-sentinel/profiles/work/auto-switch.toml

# Notify on auto-switch (native OS notification)
notify = true

[schedule]
# Activate this profile during these hours
active_hours = "09:00-18:00"
timezone = "America/New_York"
# Switch to this profile outside active_hours
fallback = "personal"
```

## Daemon Commands

```bash
cst daemon start              # start background daemon
cst daemon stop               # stop daemon
cst daemon restart            # restart
cst daemon status             # show running status
cst daemon logs               # tail daemon logs

cst auto-switch configure work  # interactive configuration wizard
cst auto-switch log             # history of all scheduled switches
cst auto-switch test work       # dry-run: show what would happen
cst pause [--minutes 30]        # pause scheduled switching temporarily
cst unpause                     # resume scheduled switching
```

## Shell Integration

Add to `~/.zshrc` (done automatically by `cst init`):

```bash
eval "$(cst shell-init)"
```

This injects a `precmd` hook that checks for pending switches from the daemon every time your prompt refreshes.

## Monitoring with `cst top`

The `cst top` live dashboard shows scheduler state in real time:

- **SCHEDULE** panel: upcoming scheduled activations with profile name and time-until-activation
- **RECENT SWITCHES** panel: last 5 switch events with `from → to | reason`
- **Header**: daemon status indicator (ON/OFF)
- **Per-profile table**: token usage and cost per session

The dashboard refreshes every 1 second.

The interactive TUI (`cst` with no args) also has an AUTO-SWITCH tab showing the same scheduler entries and a HISTORY tab with the last 30 switch events.

## Usage Guidelines

- Each profile must correspond to a Claude account you legitimately own
- The daemon activates profiles on a **time schedule**, not in response to API errors
- Do not configure the daemon to switch accounts in response to rate limit signals — this violates Anthropic's Terms of Service
- Claude Sentinel does not aggregate quota across accounts
