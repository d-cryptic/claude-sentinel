//! Per-profile pipeline configuration — `pipeline.toml`.
//!
//! A pipeline declares which profile to advance to (`next`) when this
//! profile's user-declared usage gates are exceeded. Pipelines are NOT
//! a rate-limit-bypass mechanism — they only fire on time elapsed or
//! explicitly self-reported token budgets the user has set.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

fn default_notify_pct() -> u8 {
    80
}
fn default_auto_advance() -> bool {
    true
}

/// Pipeline configuration for a single profile.
///
/// # Example
/// ```no_run
/// use cst_core::pipeline::PipelineConfig;
/// use std::path::Path;
/// let cfg = PipelineConfig::load(Path::new("/tmp/profile")).unwrap();
/// assert!(cfg.is_none() || cfg.unwrap().notify_at_pct <= 100);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PipelineConfig {
    /// Next profile (and optionally session: `"profile:session"`).
    pub next: String,
    /// Percentage at which to notify the user before advancing.
    #[serde(default = "default_notify_pct")]
    pub notify_at_pct: u8,
    /// If true, advance automatically when threshold is reached.
    #[serde(default = "default_auto_advance")]
    pub auto_advance: bool,
    /// User-declared advance criteria.
    pub advance_when: AdvanceWhen,
}

/// User-declared conditions that trigger pipeline advancement.
///
/// At least one of `tokens_used`, `hours_active`, `tokens_used_weekly`
/// must be set unless `manual_only` is true.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AdvanceWhen {
    /// Total tokens (in + out) since window start.
    pub tokens_used: Option<u64>,
    /// Wall-clock hours since window start.
    pub hours_active: Option<f64>,
    /// Total tokens (in + out) since the most recent weekly reset.
    pub tokens_used_weekly: Option<u64>,
    /// Day of week the weekly window resets on (UTC).
    pub reset_day: Option<Weekday>,
    /// If true, only `cst next` (manual) advances. No threshold checks.
    #[serde(default)]
    pub manual_only: bool,
}

/// Day of week for weekly reset windows.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl From<Weekday> for chrono::Weekday {
    fn from(w: Weekday) -> Self {
        match w {
            Weekday::Monday => chrono::Weekday::Mon,
            Weekday::Tuesday => chrono::Weekday::Tue,
            Weekday::Wednesday => chrono::Weekday::Wed,
            Weekday::Thursday => chrono::Weekday::Thu,
            Weekday::Friday => chrono::Weekday::Fri,
            Weekday::Saturday => chrono::Weekday::Sat,
            Weekday::Sunday => chrono::Weekday::Sun,
        }
    }
}

impl PipelineConfig {
    /// Load from `{profile_dir}/pipeline.toml`. Returns `Ok(None)` if absent.
    pub fn load(profile_dir: &Path) -> Result<Option<Self>> {
        let path = profile_dir.join("pipeline.toml");
        if !path.exists() {
            return Ok(None);
        }
        let s = std::fs::read_to_string(&path)?;
        let cfg: Self = toml::from_str(&s)?;
        cfg.validate()?;
        Ok(Some(cfg))
    }

    /// Save to `{profile_dir}/pipeline.toml`.
    pub fn save(&self, profile_dir: &Path) -> Result<()> {
        let path = profile_dir.join("pipeline.toml");
        std::fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Validate semantic correctness beyond what serde enforces.
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(self.notify_at_pct <= 100, "notify_at_pct must be 0-100");
        let w = &self.advance_when;
        if w.manual_only {
            return Ok(());
        }
        let set = [
            w.tokens_used.is_some(),
            w.hours_active.is_some(),
            w.tokens_used_weekly.is_some(),
        ];
        let n = set.iter().filter(|&&b| b).count();
        anyhow::ensure!(
            n >= 1,
            "advance_when: set at least one of tokens_used, hours_active, tokens_used_weekly, or manual_only = true"
        );
        if w.tokens_used_weekly.is_some() {
            anyhow::ensure!(
                w.reset_day.is_some(),
                "advance_when.reset_day required when tokens_used_weekly is set"
            );
        }
        if let Some(t) = w.tokens_used {
            anyhow::ensure!(t > 0, "tokens_used must be > 0");
        }
        if let Some(h) = w.hours_active {
            anyhow::ensure!(h > 0.0, "hours_active must be > 0");
        }
        if let Some(t) = w.tokens_used_weekly {
            anyhow::ensure!(t > 0, "tokens_used_weekly must be > 0");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn base_cfg() -> PipelineConfig {
        PipelineConfig {
            next: "backup".into(),
            notify_at_pct: 80,
            auto_advance: true,
            advance_when: AdvanceWhen {
                tokens_used: Some(1_000_000),
                ..Default::default()
            },
        }
    }

    #[test]
    fn test_roundtrip_toml() {
        let dir = TempDir::new().unwrap();
        let cfg = base_cfg();
        cfg.save(dir.path()).unwrap();
        let loaded = PipelineConfig::load(dir.path()).unwrap().unwrap();
        assert_eq!(loaded, cfg);
    }

    #[test]
    fn test_load_missing_returns_none() {
        let dir = TempDir::new().unwrap();
        assert!(PipelineConfig::load(dir.path()).unwrap().is_none());
    }

    #[test]
    fn test_validate_missing_threshold_errors() {
        let cfg = PipelineConfig {
            next: "x".into(),
            notify_at_pct: 80,
            auto_advance: true,
            advance_when: AdvanceWhen::default(),
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_weekly_without_reset_day_errors() {
        let cfg = PipelineConfig {
            next: "x".into(),
            notify_at_pct: 80,
            auto_advance: true,
            advance_when: AdvanceWhen {
                tokens_used_weekly: Some(10_000_000),
                ..Default::default()
            },
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_weekly_with_reset_day_ok() {
        let cfg = PipelineConfig {
            next: "x".into(),
            notify_at_pct: 80,
            auto_advance: true,
            advance_when: AdvanceWhen {
                tokens_used_weekly: Some(10_000_000),
                reset_day: Some(Weekday::Monday),
                ..Default::default()
            },
        };
        cfg.validate().unwrap();
    }

    #[test]
    fn test_validate_notify_pct_over_100_errors() {
        let mut cfg = base_cfg();
        cfg.notify_at_pct = 101;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_manual_only_skips_threshold_requirement() {
        let cfg = PipelineConfig {
            next: "x".into(),
            notify_at_pct: 80,
            auto_advance: false,
            advance_when: AdvanceWhen {
                manual_only: true,
                ..Default::default()
            },
        };
        cfg.validate().unwrap();
    }

    #[test]
    fn test_validate_zero_tokens_errors() {
        let cfg = PipelineConfig {
            next: "x".into(),
            notify_at_pct: 80,
            auto_advance: true,
            advance_when: AdvanceWhen {
                tokens_used: Some(0),
                ..Default::default()
            },
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_weekday_to_chrono() {
        assert_eq!(chrono::Weekday::from(Weekday::Monday), chrono::Weekday::Mon);
        assert_eq!(chrono::Weekday::from(Weekday::Sunday), chrono::Weekday::Sun);
    }
}
