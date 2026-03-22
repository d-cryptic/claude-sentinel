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

# Profiles to try in order when rate limit is hit
fallback_chain = ["api-backup", "personal"]

# Estimated minutes until quota refills (Claude Pro: 5-hour window = 300 min)
estimate_minutes = 300

# Automatically switch back when quota refills
auto_switch_back = true

# Notify on auto-switch (macOS notification)
notify = true

[schedule]
# Only activate this profile during these hours (optional)
active_hours = "09:00-18:00"
timezone = "America/New_York"
# Switch to this profile outside active_hours
fallback = "personal"

[round_robin]
# Distribute usage across a pool of profiles to maximise total uptime.
# The daemon picks the profile with the fewest tokens used before switching.
enabled = true
pool = ["work", "personal", "api-backup"]
# Rotate after this many tokens (0 = only on rate limit)
rotate_after_tokens = 0
```

### Round-Robin Mode

Round-robin distributes usage across a pool of profiles so no single account hits its quota while others remain idle. Enable it with the `[round_robin]` section above.

**How it works:**

1. Daemon detects a rate limit on the active profile
2. Reads `stats.json` (or live `history.jsonl`) for each profile in `pool`
3. Picks the profile with the lowest token usage today
4. Switches via the normal fallback mechanism

**`rotate_after_tokens`**: when set to a non-zero value the daemon proactively rotates before hitting quota. Example: `rotate_after_tokens = 80000` rotates when the current profile has used ~80k tokens in the session, spreading load evenly.

Round-robin and `fallback_chain` are independent — if round-robin is disabled, the explicit `fallback_chain` is used instead.

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
