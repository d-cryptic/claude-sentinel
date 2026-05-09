//! Quota reset scheduler — tracks when rate limits hit and when quota should refill.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::platform;

/// Persistent scheduler state stored in `~/.claude-sentinel/scheduler.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SchedulerState {
    /// Entries for each profile that has an active rate-limit timer.
    #[serde(default)]
    pub entries: Vec<RateLimitEntry>,
}

/// One rate-limit record for a profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitEntry {
    /// Profile that hit the rate limit.
    pub profile: String,
    /// When the rate limit was detected.
    pub detected_at: DateTime<Utc>,
    /// When quota is estimated to refill.
    pub refill_at: DateTime<Utc>,
    /// Whether we should auto-switch back at refill time.
    pub auto_switch_back: bool,
    /// Has the switch-back already been executed?
    pub switched_back: bool,
}

impl RateLimitEntry {
    /// Returns true if refill time has passed and switch-back hasn't happened.
    pub fn is_ready_for_switchback(&self) -> bool {
        !self.switched_back && self.auto_switch_back && Utc::now() >= self.refill_at
    }

    /// Human-readable time remaining until refill.
    pub fn time_until_refill(&self) -> String {
        let now = Utc::now();
        if now >= self.refill_at {
            return "now (refilled)".to_string();
        }
        let remaining = self.refill_at - now;
        let total_secs = remaining.num_seconds();
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        if hours > 0 {
            format!("{}h {}m", hours, mins)
        } else {
            format!("{}m", mins)
        }
    }
}

impl SchedulerState {
    fn path() -> PathBuf {
        platform::data_dir().join("scheduler.json")
    }

    pub fn load() -> Result<Self> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&contents)?)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Record a rate-limit hit for `profile`. Replaces any existing entry.
    pub fn record_rate_limit(
        &mut self,
        profile: &str,
        estimate_minutes: u64,
        auto_switch_back: bool,
    ) {
        let now = Utc::now();
        // Cap to 1 year to prevent hostile auto-switch.toml values from wrapping
        // to negative i64 and producing a refill_at in the past.
        const MAX_ESTIMATE_MINUTES: u64 = 60 * 24 * 365;
        let capped = estimate_minutes.min(MAX_ESTIMATE_MINUTES) as i64;
        let refill_at = now + Duration::minutes(capped);
        // Remove any existing entry for this profile
        self.entries.retain(|e| e.profile != profile);
        self.entries.push(RateLimitEntry {
            profile: profile.to_string(),
            detected_at: now,
            refill_at,
            auto_switch_back,
            switched_back: false,
        });
    }

    /// Return all entries due for switch-back (refill time passed, not yet switched).
    pub fn pending_switchbacks(&self) -> Vec<&RateLimitEntry> {
        self.entries
            .iter()
            .filter(|e| e.is_ready_for_switchback())
            .collect()
    }

    /// Mark a profile's entry as switched-back.
    pub fn mark_switched_back(&mut self, profile: &str) {
        for entry in &mut self.entries {
            if entry.profile == profile {
                entry.switched_back = true;
            }
        }
    }

    /// Get the entry for a profile, if any.
    pub fn entry_for(&self, profile: &str) -> Option<&RateLimitEntry> {
        self.entries.iter().find(|e| e.profile == profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_entry_for() {
        let mut state = SchedulerState::default();
        state.record_rate_limit("work", 300, true);
        let entry = state.entry_for("work").unwrap();
        assert_eq!(entry.profile, "work");
        assert!(!entry.switched_back);
    }

    #[test]
    fn test_no_pending_switchback_when_not_yet_elapsed() {
        let mut state = SchedulerState::default();
        state.record_rate_limit("work", 300, true);
        // 300 minutes haven't passed
        assert!(state.pending_switchbacks().is_empty());
    }

    #[test]
    fn test_pending_switchback_when_elapsed() {
        let mut state = SchedulerState::default();
        // Simulate a rate limit detected 301 minutes ago
        let detected_at = Utc::now() - Duration::minutes(301);
        let refill_at = detected_at + Duration::minutes(300);
        state.entries.push(RateLimitEntry {
            profile: "work".to_string(),
            detected_at,
            refill_at,
            auto_switch_back: true,
            switched_back: false,
        });
        assert_eq!(state.pending_switchbacks().len(), 1);
    }

    #[test]
    fn test_mark_switched_back_clears_pending() {
        let mut state = SchedulerState::default();
        let detected_at = Utc::now() - Duration::minutes(301);
        let refill_at = detected_at + Duration::minutes(300);
        state.entries.push(RateLimitEntry {
            profile: "work".to_string(),
            detected_at,
            refill_at,
            auto_switch_back: true,
            switched_back: false,
        });
        state.mark_switched_back("work");
        assert!(state.pending_switchbacks().is_empty());
    }

    #[test]
    fn test_time_until_refill_past_shows_refilled() {
        let entry = RateLimitEntry {
            profile: "work".to_string(),
            detected_at: Utc::now() - Duration::minutes(400),
            refill_at: Utc::now() - Duration::minutes(100),
            auto_switch_back: true,
            switched_back: false,
        };
        assert!(entry.time_until_refill().contains("refilled"));
    }

    #[test]
    fn test_replacing_existing_entry_on_new_rate_limit() {
        let mut state = SchedulerState::default();
        state.record_rate_limit("work", 300, true);
        state.record_rate_limit("work", 180, false); // second hit replaces first
        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.entries[0].auto_switch_back, false);
    }
}
