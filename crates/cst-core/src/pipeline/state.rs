//! Persistent runtime state for a profile's pipeline.

use crate::pipeline::config::Weekday;
use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Runtime state stored at `{profile_dir}/pipeline-state.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PipelineState {
    /// Highest notify-pct already announced for the current window.
    pub notified_at_pct: Option<u8>,
    /// When the last advance was performed.
    pub last_advance: Option<DateTime<Utc>>,
    /// When the current usage window started (set by manual advance / weekly reset).
    pub window_started: Option<DateTime<Utc>>,
    /// Last time the weekly counter was zeroed.
    pub last_weekly_reset: Option<DateTime<Utc>>,
    /// `tokens_in` value at window start; used to compute deltas without mutating stats.
    pub baseline_tokens_in: u64,
    /// `tokens_out` value at window start.
    pub baseline_tokens_out: u64,
}

impl PipelineState {
    /// Load from `{profile_dir}/pipeline-state.json`. Returns default if absent.
    pub fn load(profile_dir: &Path) -> Result<Self> {
        let path = profile_dir.join("pipeline-state.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
    }

    /// Atomically save state via temp-file + rename.
    pub fn save(&self, profile_dir: &Path) -> Result<()> {
        let path = profile_dir.join("pipeline-state.json");
        let tmp = path.with_extension("json.tmp");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Returns true if `now` is in a new weekly window vs `last_weekly_reset`.
    ///
    /// A window starts at 00:00 UTC on the most recent occurrence of
    /// `reset_day` at-or-before `now`.
    pub fn should_reset_weekly(&self, reset_day: Weekday, now: DateTime<Utc>) -> bool {
        let window_start = Self::weekly_window_start(reset_day, now);
        match self.last_weekly_reset {
            None => true,
            Some(lr) => lr < window_start,
        }
    }

    /// Computes the start of the current weekly window
    /// (00:00 UTC on the most recent reset_day at-or-before `now`).
    pub fn weekly_window_start(reset_day: Weekday, now: DateTime<Utc>) -> DateTime<Utc> {
        let target = chrono::Weekday::from(reset_day);
        let today_weekday = now.weekday();
        let days_since = (today_weekday.num_days_from_monday() as i64)
            .wrapping_sub(target.num_days_from_monday() as i64)
            .rem_euclid(7);
        let window_date = now.date_naive() - Duration::days(days_since);
        Utc.from_utc_datetime(&window_date.and_hms_opt(0, 0, 0).expect("00:00:00 always valid"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_roundtrip() {
        let dir = TempDir::new().unwrap();
        let state = PipelineState {
            notified_at_pct: Some(80),
            last_advance: Some(Utc::now()),
            baseline_tokens_in: 100,
            baseline_tokens_out: 50,
            ..Default::default()
        };
        state.save(dir.path()).unwrap();
        let loaded = PipelineState::load(dir.path()).unwrap();
        assert_eq!(loaded.notified_at_pct, Some(80));
        assert_eq!(loaded.baseline_tokens_in, 100);
        assert_eq!(loaded.baseline_tokens_out, 50);
    }

    #[test]
    fn test_load_missing_returns_default() {
        let dir = TempDir::new().unwrap();
        let s = PipelineState::load(dir.path()).unwrap();
        assert_eq!(s, PipelineState::default());
    }

    #[test]
    fn test_should_reset_when_none() {
        let state = PipelineState::default();
        let now = Utc.with_ymd_and_hms(2026, 5, 11, 12, 0, 0).unwrap(); // Mon
        assert!(state.should_reset_weekly(Weekday::Monday, now));
    }

    #[test]
    fn test_should_reset_in_current_window_false() {
        let now = Utc.with_ymd_and_hms(2026, 5, 13, 12, 0, 0).unwrap(); // Wed
        // Reset Monday at 00:00 UTC (May 11, 2026 is Mon).
        let monday = Utc.with_ymd_and_hms(2026, 5, 11, 0, 0, 0).unwrap();
        let state = PipelineState {
            last_weekly_reset: Some(monday + Duration::hours(1)), // after window start
            ..Default::default()
        };
        assert!(!state.should_reset_weekly(Weekday::Monday, now));
    }

    #[test]
    fn test_should_reset_in_prior_window_true() {
        let now = Utc.with_ymd_and_hms(2026, 5, 13, 12, 0, 0).unwrap(); // Wed
        let prior_monday = Utc.with_ymd_and_hms(2026, 5, 4, 12, 0, 0).unwrap();
        let state = PipelineState {
            last_weekly_reset: Some(prior_monday),
            ..Default::default()
        };
        assert!(state.should_reset_weekly(Weekday::Monday, now));
    }

    #[test]
    fn test_window_start_on_reset_day() {
        let now = Utc.with_ymd_and_hms(2026, 5, 11, 9, 30, 0).unwrap(); // Mon 09:30
        let start = PipelineState::weekly_window_start(Weekday::Monday, now);
        assert_eq!(start, Utc.with_ymd_and_hms(2026, 5, 11, 0, 0, 0).unwrap());
    }

    #[test]
    fn test_window_start_after_reset_day() {
        let now = Utc.with_ymd_and_hms(2026, 5, 14, 9, 30, 0).unwrap(); // Thu
        let start = PipelineState::weekly_window_start(Weekday::Monday, now);
        assert_eq!(start, Utc.with_ymd_and_hms(2026, 5, 11, 0, 0, 0).unwrap());
    }

    #[test]
    fn test_window_start_before_reset_day_in_week() {
        // Now = Sunday May 10, 2026; reset day = Monday → window should start prev Monday May 4.
        let now = Utc.with_ymd_and_hms(2026, 5, 10, 9, 30, 0).unwrap();
        let start = PipelineState::weekly_window_start(Weekday::Monday, now);
        assert_eq!(start, Utc.with_ymd_and_hms(2026, 5, 4, 0, 0, 0).unwrap());
    }
}
