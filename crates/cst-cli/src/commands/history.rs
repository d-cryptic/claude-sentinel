use anyhow::Result;
use cst_core::auto_switch::switch_log::{SwitchLog, SwitchReason};

pub fn run() -> Result<()> {
    let log = SwitchLog::open();
    let events = log.last_n(50)?;
    if events.is_empty() {
        println!("No switch history recorded yet.");
        println!("Run `cst use <profile>` to start tracking switches.");
        return Ok(());
    }
    println!("{:<24} {:<20} {:<20} {}", "TIMESTAMP", "FROM", "TO", "REASON");
    println!("{}", "─".repeat(80));
    for ev in &events {
        println!(
            "{:<24} {:<20} {:<20} {}",
            ev.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            format!("{}:{}", ev.from_profile, ev.from_session),
            format!("{}:{}", ev.to_profile, ev.to_session),
            ev.reason,
        );
    }
    Ok(())
}

pub fn why() -> Result<()> {
    let cfg = cst_core::GlobalConfig::load()?;
    if cfg.current_profile.is_empty() {
        println!("No active profile. Run `cst use <profile>` to switch.");
        return Ok(());
    }

    // Find the most recent event targeting the current profile
    let log = SwitchLog::open();
    if let Ok(events) = log.read_all() {
        if let Some(ev) = events.iter().rev().find(|e| e.to_profile == cfg.current_profile) {
            let reason_detail = match ev.reason {
                SwitchReason::Manual => "manually activated via `cst use`".to_string(),
                SwitchReason::RateLimit => format!("auto-switched due to rate limit ({})", ev.detail),
                SwitchReason::QuotaRefill => "auto-switched back after quota refill".to_string(),
                SwitchReason::Schedule => "activated by time-based schedule".to_string(),
                SwitchReason::AutoDetect => "auto-detected from .cstrc in project directory".to_string(),
            };
            println!("Active: {}", cfg.current_ref());
            println!("Reason: {}", reason_detail);
            println!("Since:  {}", ev.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
            return Ok(());
        }
    }

    println!("Active: {} (reason unknown — no history recorded)", cfg.current_ref());
    Ok(())
}
