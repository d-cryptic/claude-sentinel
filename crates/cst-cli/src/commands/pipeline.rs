//! `cst next` and `cst pipeline ...` commands.

use anyhow::{Context, Result};
use cst_core::pipeline::{
    AdvanceWhen, PipelineConfig, PipelineState, ThresholdChecker, Weekday,
};
use cst_core::stats::SessionStats;
use cst_core::{platform, validate_profile_name, GlobalConfig};
use std::io::{self, Write};

/// Advance the current profile's pipeline to its configured next profile.
pub fn next() -> Result<()> {
    let cfg = GlobalConfig::load()?;
    if cfg.current_profile.is_empty() {
        anyhow::bail!("no current profile — run `cst use <profile>` first");
    }
    let (next_profile, next_session) =
        cst_core::pipeline::advance::advance_now(&cfg.current_profile)?;
    println!(
        "Pipeline advanced: '{}' → '{}:{}'",
        cfg.current_profile, next_profile, next_session
    );
    println!("Next shell prompt will pick up the switch via the precmd hook.");
    Ok(())
}

/// Show pipeline status for the named (or current) profile.
pub fn status(profile: Option<String>) -> Result<()> {
    let profile = match profile {
        Some(p) => p,
        None => {
            let cfg = GlobalConfig::load()?;
            if cfg.current_profile.is_empty() {
                anyhow::bail!("no current profile — pass a profile name or run `cst use` first");
            }
            cfg.current_profile
        }
    };
    validate_profile_name(&profile)?;

    let profile_dir = platform::profile_dir(&profile);
    let cfg = match PipelineConfig::load(&profile_dir)? {
        None => {
            println!("No pipeline configured for profile '{profile}'.");
            println!("Run `cst pipeline configure {profile}` to create one.");
            return Ok(());
        }
        Some(c) => c,
    };
    let state = PipelineState::load(&profile_dir).unwrap_or_default();

    let global = GlobalConfig::load().unwrap_or_default();
    let session = if global.current_profile == profile {
        global.current_session
    } else {
        "default".to_string()
    };
    let stats = SessionStats::load(&platform::session_dir(&profile, &session)).unwrap_or_default();
    let pct = ThresholdChecker::current_pct(&cfg, &state, &stats, chrono::Utc::now());

    println!("Pipeline for '{profile}':");
    println!("  Next profile     : {}", cfg.next);
    println!("  Auto-advance     : {}", cfg.auto_advance);
    println!("  Notify at        : {}%", cfg.notify_at_pct);
    print_threshold(&cfg.advance_when);
    println!("  Current usage    : {pct}%");
    if let Some(la) = state.last_advance {
        println!("  Last advance     : {la}");
    }
    if let Some(ws) = state.window_started {
        println!("  Window started   : {ws}");
    }
    if let Some(lr) = state.last_weekly_reset {
        println!("  Last weekly reset: {lr}");
    }
    Ok(())
}

fn print_threshold(w: &AdvanceWhen) {
    if w.manual_only {
        println!("  Mode             : manual_only");
        return;
    }
    if let Some(t) = w.tokens_used {
        println!("  Threshold        : {t} tokens (per window)");
    }
    if let Some(h) = w.hours_active {
        println!("  Threshold        : {h} hours (per window)");
    }
    if let Some(t) = w.tokens_used_weekly {
        println!("  Threshold        : {t} tokens (weekly)");
        if let Some(d) = w.reset_day {
            println!("  Reset day        : {d:?}");
        }
    }
}

/// Interactively create or edit `pipeline.toml` for a profile.
pub fn configure(profile: &str) -> Result<()> {
    validate_profile_name(profile)?;
    let profile_dir = platform::profile_dir(profile);
    if !profile_dir.exists() {
        anyhow::bail!(
            "profile directory does not exist: {} — create the profile first with `cst new {profile}`",
            profile_dir.display()
        );
    }

    let existing = PipelineConfig::load(&profile_dir).ok().flatten();
    if let Some(ref c) = existing {
        println!("Existing pipeline:");
        println!("  next            = {}", c.next);
        println!("  notify_at_pct   = {}", c.notify_at_pct);
        println!("  auto_advance    = {}", c.auto_advance);
        println!();
    }

    let next = prompt(
        "Next profile (or profile:session)",
        existing.as_ref().map(|c| c.next.as_str()),
    )?;
    if next.is_empty() {
        anyhow::bail!("`next` is required");
    }
    let notify_pct: u8 = prompt_or_default(
        "Notify at percent (0-100)",
        existing.as_ref().map(|c| c.notify_at_pct).unwrap_or(80),
    )?;
    let auto_advance: bool = prompt_or_default(
        "Auto-advance when threshold reached? (true/false)",
        existing.as_ref().map(|c| c.auto_advance).unwrap_or(true),
    )?;

    println!();
    println!("Threshold mode: 1) tokens_used  2) hours_active  3) tokens_used_weekly  4) manual_only");
    let mode = prompt("Choose [1-4]", Some("1"))?;
    let mut advance_when = AdvanceWhen::default();
    match mode.trim() {
        "1" => {
            advance_when.tokens_used = Some(prompt_or_default("Tokens per window", 1_000_000u64)?);
        }
        "2" => {
            advance_when.hours_active = Some(prompt_or_default("Hours per window", 5.0_f64)?);
        }
        "3" => {
            advance_when.tokens_used_weekly =
                Some(prompt_or_default("Tokens per week", 10_000_000u64)?);
            let day = prompt("Reset day (monday..sunday)", Some("monday"))?;
            advance_when.reset_day = Some(parse_weekday(&day)?);
        }
        "4" => {
            advance_when.manual_only = true;
        }
        other => anyhow::bail!("invalid choice: {other}"),
    }

    let cfg = PipelineConfig {
        next: next.trim().to_string(),
        notify_at_pct: notify_pct,
        auto_advance,
        advance_when,
    };
    cfg.validate()?;
    cfg.save(&profile_dir)?;
    println!(
        "\nWrote {}",
        profile_dir.join("pipeline.toml").display()
    );
    Ok(())
}

fn prompt(label: &str, default: Option<&str>) -> Result<String> {
    match default {
        Some(d) => print!("{label} [{d}]: "),
        None => print!("{label}: "),
    }
    io::stdout().flush().ok();
    let mut s = String::new();
    io::stdin().read_line(&mut s).context("reading stdin")?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        if let Some(d) = default {
            return Ok(d.to_string());
        }
        return Ok(String::new());
    }
    Ok(trimmed.to_string())
}

fn prompt_or_default<T>(label: &str, default: T) -> Result<T>
where
    T: std::fmt::Display + std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    let default_str = default.to_string();
    let raw = prompt(label, Some(&default_str))?;
    if raw.trim().is_empty() {
        return Ok(default);
    }
    raw.trim()
        .parse::<T>()
        .map_err(|e| anyhow::anyhow!("invalid value '{raw}': {e}"))
}

fn parse_weekday(s: &str) -> Result<Weekday> {
    Ok(match s.trim().to_ascii_lowercase().as_str() {
        "mon" | "monday" => Weekday::Monday,
        "tue" | "tuesday" => Weekday::Tuesday,
        "wed" | "wednesday" => Weekday::Wednesday,
        "thu" | "thursday" => Weekday::Thursday,
        "fri" | "friday" => Weekday::Friday,
        "sat" | "saturday" => Weekday::Saturday,
        "sun" | "sunday" => Weekday::Sunday,
        other => anyhow::bail!("unknown weekday: {other}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_weekday() {
        assert_eq!(parse_weekday("monday").unwrap(), Weekday::Monday);
        assert_eq!(parse_weekday("Sun").unwrap(), Weekday::Sunday);
        assert!(parse_weekday("notaday").is_err());
    }
}
