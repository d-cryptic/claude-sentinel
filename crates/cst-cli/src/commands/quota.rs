//! `cst remaining` — show quota usage, estimated time to reset, and token counts.
//!
//! Token counts are read from `history.jsonl` (live, parsed on the fly) when
//! available, falling back to the cached `stats.json` values.

use anyhow::Result;
use cst_core::{
    auto_switch::scheduler::SchedulerState,
    config::GlobalConfig,
    history_parser,
    platform,
    profile::ProfileManager,
    session::SessionManager,
    stats::SessionStats,
};

pub fn remaining() -> Result<()> {
    let cfg = GlobalConfig::load().unwrap_or_default();

    if cfg.current_profile.is_empty() {
        println!("No active profile. Run: cst use <profile>");
        return Ok(());
    }

    let profile = &cfg.current_profile;
    let session = &cfg.current_session;

    println!("Profile  : {}:{}", profile, session);
    println!();

    // ── Token usage for active session ───────────────────────────────────
    let session_dir = platform::session_dir(profile, session);
    let stats = SessionStats::load(&session_dir).unwrap_or_default();

    // Prefer live history.jsonl counts; fall back to stats.json.
    let claude_dir = platform::claude_config_dir(profile, session);
    let live = history_parser::parse_tokens(&claude_dir.join("history.jsonl")).ok();
    let (tokens_in, tokens_out, cost) = if let Some(h) = &live {
        (h.input_tokens, h.output_tokens, h.estimated_cost_usd())
    } else {
        (stats.tokens_in, stats.tokens_out, stats.estimated_cost_usd)
    };
    let live_label = if live.is_some() { " (live)" } else { "" };

    println!("── Token Usage (current session){} ──────────────────────────", live_label);
    println!("  Tokens in   : {}", format_tokens(tokens_in));
    println!("  Tokens out  : {}", format_tokens(tokens_out));
    println!("  Total       : {}", format_tokens(tokens_in + tokens_out));
    if cost > 0.0 {
        println!("  Est. cost   : ${:.4}", cost);
    }
    println!("  Rate limits : {}", stats.rate_limit_hits);
    if let Some(last) = stats.last_used {
        println!("  Last used   : {}", last.format("%Y-%m-%d %H:%M UTC"));
    }
    println!();

    // ── All sessions for this profile combined ───────────────────────────
    let sm = SessionManager::new(platform::profile_dir(profile));
    if let Ok(sessions) = sm.list() {
        if sessions.len() > 1 {
            let (total_in, total_out, total_rl, total_cost) = sessions.iter().fold(
                (0u64, 0u64, 0u64, 0f64),
                |(tin, tout, trl, tcost), s| {
                    let sd = platform::session_dir(profile, &s.name);
                    let st = SessionStats::load(&sd).unwrap_or_default();
                    (tin + st.tokens_in, tout + st.tokens_out, trl + st.rate_limit_hits, tcost + st.estimated_cost_usd)
                },
            );
            println!("── All Sessions ({}) ───────────────────────────────────────", profile);
            println!("  Tokens in   : {}", format_tokens(total_in));
            println!("  Tokens out  : {}", format_tokens(total_out));
            println!("  Total       : {}", format_tokens(total_in + total_out));
            if total_cost > 0.0 {
                println!("  Est. cost   : ${:.4}", total_cost);
            }
            println!("  Rate limits : {}", total_rl);
            println!();
        }
    }

    // ── Rate limit / quota reset timers ─────────────────────────────────
    let sched = SchedulerState::load().unwrap_or_default();
    let active: Vec<_> = sched.entries.iter().filter(|e| !e.switched_back).collect();

    if active.is_empty() {
        println!("── Quota Status ────────────────────────────────────────────");
        println!("  No active rate limits detected.");
        println!("  Daemon will alert when a rate limit is hit.");
    } else {
        println!("── Rate Limit Timers ───────────────────────────────────────");
        for entry in &active {
            println!(
                "  Profile     : {}",
                entry.profile
            );
            println!(
                "  Hit at      : {}",
                entry.detected_at.format("%Y-%m-%d %H:%M UTC")
            );
            println!(
                "  Est. refill : {} ({})",
                entry.refill_at.format("%H:%M UTC"),
                entry.time_until_refill()
            );
            if entry.auto_switch_back {
                println!("  Auto-back   : yes — will switch back when quota refills");
            }
            println!();
        }
    }

    // ── Cross-profile summary ────────────────────────────────────────────
    let pm = ProfileManager::new(platform::profiles_dir());
    if let Ok(profiles) = pm.list() {
        if profiles.len() > 1 {
            println!("── All Profiles ────────────────────────────────────────────");
            for p in &profiles {
                let sm2 = SessionManager::new(platform::profile_dir(&p.name));
                let (tin, tout, rl) = sm2
                    .list()
                    .unwrap_or_default()
                    .iter()
                    .fold((0u64, 0u64, 0u64), |(tin, tout, rl), s| {
                        let sd = platform::session_dir(&p.name, &s.name);
                        let st = SessionStats::load(&sd).unwrap_or_default();
                        (tin + st.tokens_in, tout + st.tokens_out, rl + st.rate_limit_hits)
                    });
                let active_marker = if p.name == *profile { " [ACTIVE]" } else { "" };
                let rl_str = if rl > 0 { format!("  ⚠ {} rate limits", rl) } else { String::new() };
                println!(
                    "  {:<20} in:{:>8}  out:{:>8}{}{}",
                    format!("{}{}", p.name, active_marker),
                    format_tokens(tin),
                    format_tokens(tout),
                    rl_str,
                    ""
                );
            }
        }
    }

    Ok(())
}

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
