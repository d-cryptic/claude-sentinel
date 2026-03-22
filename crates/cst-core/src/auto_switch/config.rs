//! Per-profile auto-switch configuration — `auto-switch.toml`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Auto-switch settings stored at `profiles/{p}/auto-switch.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoSwitchConfig {
    /// Profiles to try on rate limit (in order).
    #[serde(default)]
    pub fallback_chain: Vec<String>,

    /// Quota reset window in minutes (e.g. 300 for Claude Pro 5-hour window).
    #[serde(default = "default_estimate_minutes")]
    pub estimate_minutes: u64,

    /// Automatically switch back to this profile when quota refills.
    #[serde(default = "default_true")]
    pub auto_switch_back: bool,

    /// Show a system notification on auto-switch.
    #[serde(default = "default_true")]
    pub notify: bool,

    /// Optional time-based schedule section.
    #[serde(default)]
    pub schedule: Option<Schedule>,

    /// Optional smart round-robin configuration.
    #[serde(default)]
    pub round_robin: Option<RoundRobin>,
}

/// Smart round-robin: distribute usage across a pool of profiles to maximise
/// total uptime before any single profile hits its quota.
///
/// The daemon picks the profile in `pool` with the fewest tokens used today.
///
/// ```toml
/// [round_robin]
/// enabled = true
/// pool = ["work", "personal", "api-backup"]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundRobin {
    /// Whether round-robin mode is active.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Ordered list of profiles to rotate across.
    #[serde(default)]
    pub pool: Vec<String>,

    /// Switch to the next profile after this many tokens (rough estimate).
    /// Defaults to 0 = switch only on rate limit.
    #[serde(default)]
    pub rotate_after_tokens: u64,
}

fn default_estimate_minutes() -> u64 { 300 }
fn default_true() -> bool { true }

/// Time-based switching schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    /// Active hours range, e.g. "09:00-18:00".
    pub active_hours: String,
    /// IANA timezone string, e.g. "America/New_York".
    #[serde(default = "default_utc")]
    pub timezone: String,
    /// Profile to use outside active hours.
    pub fallback: String,
}

fn default_utc() -> String { "UTC".to_string() }

impl Default for AutoSwitchConfig {
    fn default() -> Self {
        Self {
            fallback_chain: Vec::new(),
            estimate_minutes: default_estimate_minutes(),
            auto_switch_back: true,
            notify: true,
            schedule: None,
            round_robin: None,
        }
    }
}

impl AutoSwitchConfig {
    /// Load from `{profile_dir}/auto-switch.toml`. Returns defaults if file is absent.
    pub fn load(profile_dir: &Path) -> Result<Self> {
        let path = profile_dir.join("auto-switch.toml");
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)?;
        Ok(toml::from_str(&contents)?)
    }

    /// Save to `{profile_dir}/auto-switch.toml`.
    pub fn save(&self, profile_dir: &Path) -> Result<()> {
        let path = profile_dir.join("auto-switch.toml");
        std::fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let cfg = AutoSwitchConfig::default();
        assert_eq!(cfg.estimate_minutes, 300);
        assert!(cfg.auto_switch_back);
        assert!(cfg.fallback_chain.is_empty());
    }

    #[test]
    fn test_load_nonexistent_returns_default() {
        let dir = TempDir::new().unwrap();
        let cfg = AutoSwitchConfig::load(dir.path()).unwrap();
        assert_eq!(cfg.estimate_minutes, 300);
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let mut cfg = AutoSwitchConfig::default();
        cfg.fallback_chain = vec!["backup-oauth".to_string(), "api-key".to_string()];
        cfg.estimate_minutes = 180;
        cfg.save(dir.path()).unwrap();

        let loaded = AutoSwitchConfig::load(dir.path()).unwrap();
        assert_eq!(loaded.fallback_chain, vec!["backup-oauth", "api-key"]);
        assert_eq!(loaded.estimate_minutes, 180);
    }

    #[test]
    fn test_round_robin_serialization() {
        let dir = TempDir::new().unwrap();
        let toml = r#"
fallback_chain = ["personal"]
estimate_minutes = 300

[round_robin]
enabled = true
pool = ["work", "personal", "api-backup"]
rotate_after_tokens = 50000
"#;
        std::fs::write(dir.path().join("auto-switch.toml"), toml).unwrap();
        let cfg = AutoSwitchConfig::load(dir.path()).unwrap();
        let rr = cfg.round_robin.unwrap();
        assert!(rr.enabled);
        assert_eq!(rr.pool, vec!["work", "personal", "api-backup"]);
        assert_eq!(rr.rotate_after_tokens, 50_000);
    }

    #[test]
    fn test_round_robin_absent_is_none() {
        let cfg = AutoSwitchConfig::default();
        assert!(cfg.round_robin.is_none());
    }

    #[test]
    fn test_schedule_fields() {
        let dir = TempDir::new().unwrap();
        let toml = r#"
fallback_chain = ["personal"]
estimate_minutes = 300
auto_switch_back = true
notify = false

[schedule]
active_hours = "09:00-18:00"
timezone = "America/New_York"
fallback = "personal"
"#;
        std::fs::write(dir.path().join("auto-switch.toml"), toml).unwrap();
        let cfg = AutoSwitchConfig::load(dir.path()).unwrap();
        let sched = cfg.schedule.unwrap();
        assert_eq!(sched.active_hours, "09:00-18:00");
        assert_eq!(sched.fallback, "personal");
        assert!(!cfg.notify);
    }
}
