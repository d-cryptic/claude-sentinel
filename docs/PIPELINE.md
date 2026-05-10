# Account Pipeline

> **Disclaimer:** Claude Sentinel is an independent, open-source tool. It is not affiliated with, endorsed by, or associated with Anthropic PBC. "Claude" and "Claude Code" are trademarks of Anthropic PBC. This tool interacts with Claude Code through officially documented configuration mechanisms only.

## What Account Pipeline Is

**Account Pipeline** lets you declare a sequence of profiles that the daemon (or you) will advance through based on usage thresholds **you set yourself**.

You know your own plan. You know roughly how much usage you have on each account. You declare a threshold — say, "switch after 40,000 tokens" or "switch after 5 hours of activity" — and Claude Sentinel counts your local token/time usage. When you hit your declared threshold, the pipeline advances to the next profile.

Two modes are supported:

- **`auto_advance = true`** — the daemon switches profiles automatically when the threshold is hit.
- **`auto_advance = false`** — Claude Sentinel sends a notification when you hit your threshold; you advance manually with `cst next`.

A configurable warning at `notify_at_pct` (default 80%) gives you a heads-up so you can finish your current task before the swap.

### What this is not

- It is **not** triggered by 429 / rate-limit errors. The pipeline never reads API error signals.
- It is **not** an attempt to circumvent any account's quota. Each profile must be a Claude account you legitimately own.
- It does **not** aggregate quotas across accounts. It only reads its own local counters.

## Configuration

Each profile that participates in a pipeline has a `pipeline.toml`:

```
~/.claude-sentinel/profiles/<profile>/pipeline.toml
```

### Fields

| Field             | Type    | Description                                                              |
| ----------------- | ------- | ------------------------------------------------------------------------ |
| `next`            | string  | Profile name to advance to when the threshold is reached.                |
| `notify_at_pct`   | int     | Percent of threshold at which to fire a warning notification (default 80). |
| `auto_advance`    | bool    | If `true`, daemon switches automatically. If `false`, notify-only.       |
| `[advance_when]`  | section | Threshold definition. Pick **one** of the threshold types below.         |

### Threshold types

`[advance_when]` accepts exactly one of:

- `tokens_used` — total tokens since the current window started.
- `hours_active` — wall-clock hours since the current window started.
- `tokens_used_weekly` — total tokens within a rolling weekly window. Requires `reset_day`.

For weekly thresholds:

- `reset_day` — one of `monday`, `tuesday`, `wednesday`, `thursday`, `friday`, `saturday`, `sunday`. The window starts at **00:00 UTC** on the most recent occurrence of that day.

### Example: Pro user (5-hour window)

```toml
# ~/.claude-sentinel/profiles/work/pipeline.toml
next = "personal"
notify_at_pct = 80
auto_advance = true

[advance_when]
tokens_used = 40000      # your own estimate of your 5-hour window
```

### Example: Max user (weekly)

```toml
# ~/.claude-sentinel/profiles/work/pipeline.toml
next = "personal"
notify_at_pct = 85
auto_advance = true

[advance_when]
tokens_used_weekly = 500000
reset_day = "monday"
```

### Example: API-key budget

```toml
# ~/.claude-sentinel/profiles/api-prod/pipeline.toml
next = "api-staging"
notify_at_pct = 75
auto_advance = false      # notify only — you advance manually

[advance_when]
tokens_used_weekly = 2000000
reset_day = "sunday"
```

### Example: Manual-only

```toml
# ~/.claude-sentinel/profiles/personal/pipeline.toml
next = "client"
notify_at_pct = 90
auto_advance = false

[advance_when]
hours_active = 6
```

## CLI

```bash
cst next                          # manually advance pipeline (current -> next)
cst pipeline status               # show usage %, threshold, and ETA for current profile
cst pipeline configure <profile>  # interactive setup wizard
```

### `cst next`

Advances the current profile's pipeline by one step. Reads `pipeline.toml` for the active profile, switches to `next`, resets local counters for the new profile's window.

```bash
$ cst next
> work -> personal
> tokens_used reset to 0
```

### `cst pipeline status`

Prints the current profile, threshold, current usage, percent consumed, and an ETA based on recent usage rate.

```bash
$ cst pipeline status
profile:        work
threshold:      40000 tokens_used
used:           32140 (80.4%)
warning at:     80% (reached 4m ago)
auto_advance:   true
next:           personal
eta to advance: ~14m
```

### `cst pipeline configure <profile>`

Interactive wizard. Walks through:

1. Choose threshold type (`tokens_used`, `hours_active`, `tokens_used_weekly`).
2. Enter threshold value.
3. Pick `next` profile from existing profiles.
4. Set `notify_at_pct` and `auto_advance`.
5. Writes `pipeline.toml` to disk.

## Weekly reset semantics

For `tokens_used_weekly`, the window:

- Starts at **00:00 UTC** on the most recent occurrence of `reset_day` (inclusive).
- Ends 7 days later.
- Counters reset to zero at the boundary; previous-window usage is archived.

Examples (assuming `reset_day = "monday"`):

| Now (UTC)            | Window start         | Window end           |
| -------------------- | -------------------- | -------------------- |
| Wed 2026-05-13 14:00 | Mon 2026-05-11 00:00 | Mon 2026-05-18 00:00 |
| Mon 2026-05-11 00:00 | Mon 2026-05-11 00:00 | Mon 2026-05-18 00:00 |
| Mon 2026-05-11 00:01 | Mon 2026-05-11 00:00 | Mon 2026-05-18 00:00 |

## FAQ

**Is this rate-limit circumvention?**

No. You declare your own threshold based on your knowledge of your plan. The tool counts your own local tokens; it does not react to API error signals from Anthropic. Each profile in your pipeline must be a Claude account you legitimately own and control. The tool does not aggregate quotas — it simply switches the active context based on counters you defined.

**Why not just react to 429 errors?**

Reacting to rate-limit errors to keep prompts flowing across accounts would be quota-circumvention, which violates Anthropic's Terms of Service. Claude Sentinel deliberately avoids reading rate-limit signals. Pipeline thresholds are user-declared local counters only.

**Can I have a circular pipeline (work -> personal -> work)?**

Yes. Each profile's `next` is independent. A cycle is allowed; the daemon simply advances one step per threshold.

**What happens if `next` points to a profile that no longer exists?**

`cst next` and the daemon will refuse to advance and emit an error. Run `cst pipeline configure <profile>` to update.

**Does this replace `auto-switch` (time-based scheduling)?**

No. They coexist. `auto-switch` is time-based ("work hours 9-6"); pipelines are usage-based. You can use both — the daemon respects whichever fires first, with explicit precedence rules documented in the source.
