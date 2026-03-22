# Auto-Switch

The auto-switch daemon monitors your Claude usage and automatically switches profiles on rate limits.

## How It Works

```
1. cst daemon start
2. Daemon watches ~/.claude-sentinel/.../history.jsonl for rate limit patterns
3. Rate limit detected → try next API key in pool
4. All keys exhausted → switch to next profile in fallback_chain
5. Sends shell notification (precmd hook) + macOS notification
6. Schedules auto-switch-back at: rate_limit_time + estimate_minutes
```

## Configuration

```toml
# ~/.claude-sentinel/profiles/work/auto-switch.toml

[on_rate_limit]
# Profiles to try in order when work profile hits rate limit
fallback_chain = ["api-backup", "personal"]

# Try rotating API keys within this profile first (before switching profiles)
rotate_keys_first = true

[quota_reset]
# Estimated minutes until quota refills (Claude Pro: 5-hour window = 300 min)
estimate_minutes = 300

# Automatically switch back to this profile when quota refills
auto_switch_back = true

# Notify on auto-switch (macOS notification)
notify = true

[schedule]
# Only use this profile during these hours (optional)
active_hours = "09:00-18:00"
days = ["Mon", "Tue", "Wed", "Thu", "Fri"]
timezone = "America/New_York"
# Switch to this profile outside active_hours
fallback = "personal"

[cost_guard]
# Switch to subscription profile when daily API spend exceeds this
daily_limit_usd = 5.00
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
cst auto-switch log            # history of all auto-switches
cst auto-switch test work      # dry-run: show what would happen
cst pause [--minutes 30]       # pause auto-switch temporarily
```

## Rate Limit Detection

Sentinel uses multiple layers:

1. **File watcher**: Monitors `history.jsonl` for rate limit JSON entries
2. **Wrapper mode**: `cst exec -- claude` intercepts stderr/stdout
3. **IPC**: Claude Code `PostToolUse` hook can signal the daemon directly

## Shell Integration

Add to `~/.zshrc` (done automatically by `cst init`):

```bash
eval "$(cst shell-init)"
```

This injects a `precmd` hook that checks for pending switches from the daemon every time your prompt refreshes.

## Monitoring with `cst top`

The `cst top` live dashboard shows auto-switch state in real time:

- **QUOTA TIMERS** panel: active rate-limit countdown timers with profile name and time until refill
- **RECENT SWITCHES** panel: last 5 switch events with `from → to | reason`
- **Header**: daemon status indicator (ON/OFF)
- **Per-profile table**: rate limit hit count per session

The dashboard refreshes every 1 second. Run `cst top` to watch auto-switch activity as it happens.

The interactive TUI (`cst` with no args) also has an AUTO-SWITCH tab showing the same scheduler entries and a HISTORY tab with the last 30 switch events.

## Quota Reset Estimates

| Plan | Estimate |
|------|----------|
| Claude Pro | 300 min (5-hour rolling window) |
| Claude Max | 300 min (higher quota, same window) |
| API (Tier 1) | 60 min |
| API (Tier 2+) | Configure manually |
| AWS Bedrock | Configure manually |
