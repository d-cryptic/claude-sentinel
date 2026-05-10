//! Pipeline advancement logic — daemon `tick` and manual `advance_now`.

use crate::pipeline::{
    config::PipelineConfig,
    state::PipelineState,
    threshold::{CheckResult, ThresholdChecker},
};
use crate::stats::SessionStats;
use crate::{platform, GlobalConfig};
use anyhow::Result;
use chrono::Utc;

/// Per-tick aggregate report (useful for tests and metrics).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TickReport {
    pub notified: u32,
    pub advanced: u32,
    pub errors: u32,
}

/// Manually advance the named profile's pipeline to its `next` profile.
///
/// Resets baseline counters so the new window starts at zero, writes the
/// pending-switch file the shell precmd hook reads on its next prompt, and
/// returns `(next_profile, next_session)`.
pub fn advance_now(profile: &str) -> Result<(String, String)> {
    let profile_dir = platform::profile_dir(profile);
    let cfg = PipelineConfig::load(&profile_dir)?
        .ok_or_else(|| anyhow::anyhow!("no pipeline.toml for profile '{profile}'"))?;

    let (next_profile, next_session) = parse_profile_session(&cfg.next);

    let stats = current_session_dir(profile)
        .ok()
        .and_then(|d| SessionStats::load(&d).ok())
        .unwrap_or_default();
    let mut state = PipelineState::load(&profile_dir).unwrap_or_default();
    state.baseline_tokens_in = stats.tokens_in;
    state.baseline_tokens_out = stats.tokens_out;
    state.notified_at_pct = None;
    let now = Utc::now();
    state.last_advance = Some(now);
    state.window_started = Some(now);
    state.last_weekly_reset = Some(now);
    state.save(&profile_dir)?;

    write_pending_switch(&next_profile, &next_session)?;

    Ok((next_profile, next_session))
}

/// Daemon tick: evaluate the current profile's pipeline and act.
///
/// Time-based and user-declared usage gates only — never reads HTTP 429
/// signals (Anthropic AUP compliance).
pub fn tick(current_profile: &str) -> Result<TickReport> {
    let mut report = TickReport::default();
    let profile_dir = platform::profile_dir(current_profile);

    let cfg = match PipelineConfig::load(&profile_dir)? {
        None => return Ok(report),
        Some(c) => c,
    };

    let mut state = PipelineState::load(&profile_dir).unwrap_or_default();
    let now = Utc::now();

    // Weekly reset handling
    if let Some(reset_day) = cfg.advance_when.reset_day {
        if state.should_reset_weekly(reset_day, now) {
            let stats = current_session_dir(current_profile)
                .ok()
                .and_then(|d| SessionStats::load(&d).ok())
                .unwrap_or_default();
            state.baseline_tokens_in = stats.tokens_in;
            state.baseline_tokens_out = stats.tokens_out;
            state.last_weekly_reset = Some(now);
            state.notified_at_pct = None;
            if let Err(e) = state.save(&profile_dir) {
                tracing::warn!("pipeline weekly reset save failed: {e}");
            }
        }
    }

    let stats = current_session_dir(current_profile)
        .ok()
        .and_then(|d| SessionStats::load(&d).ok())
        .unwrap_or_default();
    let result = ThresholdChecker::evaluate(&cfg, &state, &stats, now);

    match result {
        CheckResult::ManualOnly | CheckResult::BelowNotify { .. } => {}

        CheckResult::Notify { pct } => {
            super::notify::send(
                "Pipeline threshold approaching",
                &format!(
                    "Profile '{}' at {}% — next: '{}'",
                    current_profile, pct, cfg.next
                ),
            )?;
            state.notified_at_pct = Some(cfg.notify_at_pct);
            if let Err(e) = state.save(&profile_dir) {
                tracing::warn!("pipeline state save failed: {e}");
            }
            report.notified += 1;
        }

        CheckResult::Advance { pct } => {
            if cfg.auto_advance {
                match advance_now(current_profile) {
                    Ok((next_p, _)) => {
                        super::notify::send(
                            "Pipeline advanced",
                            &format!(
                                "Switched from '{}' to '{}' ({}% used)",
                                current_profile, next_p, pct
                            ),
                        )?;
                        report.advanced += 1;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "pipeline auto-advance failed for '{current_profile}': {e}"
                        );
                        report.errors += 1;
                    }
                }
            } else {
                super::notify::send(
                    "Pipeline threshold reached",
                    &format!(
                        "Profile '{}' at {}% — run `cst next` to advance to '{}'",
                        current_profile, pct, cfg.next
                    ),
                )?;
                state.notified_at_pct = Some(100);
                if let Err(e) = state.save(&profile_dir) {
                    tracing::warn!("pipeline state save failed: {e}");
                }
                report.notified += 1;
            }
        }
    }

    Ok(report)
}

/// Parse `"profile"` or `"profile:session"` into a tuple, defaulting session to "default".
pub fn parse_profile_session(s: &str) -> (String, String) {
    if let Some((p, sess)) = s.split_once(':') {
        (p.to_string(), sess.to_string())
    } else {
        (s.to_string(), "default".to_string())
    }
}

fn current_session_dir(profile: &str) -> Result<std::path::PathBuf> {
    let cfg = GlobalConfig::load().unwrap_or_default();
    let session = if cfg.current_profile == profile {
        cfg.current_session
    } else {
        "default".to_string()
    };
    Ok(platform::profile_dir(profile)
        .join("sessions")
        .join(session))
}

fn write_pending_switch(profile: &str, session: &str) -> Result<()> {
    let path = platform::data_dir().join("pending-switch.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let payload = serde_json::json!({ "profile": profile, "session": session });
    std::fs::write(path, serde_json::to_string(&payload)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_profile_session_with_colon() {
        let (p, s) = parse_profile_session("work:backend");
        assert_eq!(p, "work");
        assert_eq!(s, "backend");
    }

    #[test]
    fn test_parse_profile_session_without_colon() {
        let (p, s) = parse_profile_session("work");
        assert_eq!(p, "work");
        assert_eq!(s, "default");
    }

    #[test]
    fn test_tick_no_pipeline_returns_empty() {
        // Use a profile dir that doesn't exist; tick must short-circuit cleanly.
        let dir = tempfile::TempDir::new().unwrap();
        // SAFETY: serial test relying on env not being mutated elsewhere.
        unsafe {
            std::env::set_var("CST_DATA_DIR", dir.path());
        }
        let report = tick("nonexistent-profile-xyz").unwrap();
        unsafe {
            std::env::remove_var("CST_DATA_DIR");
        }
        assert_eq!(report, TickReport::default());
    }
}
