//! Threshold evaluation — purely functional given config + state + stats + now.

use crate::pipeline::config::PipelineConfig;
use crate::pipeline::state::PipelineState;
use crate::stats::SessionStats;
use chrono::{DateTime, Utc};

/// Outcome of a threshold check.
#[derive(Debug, Clone, PartialEq)]
pub enum CheckResult {
    /// Below the notify threshold — nothing to do.
    BelowNotify { pct: u8 },
    /// Cross the notify threshold for the first time in this window.
    Notify { pct: u8 },
    /// At or above 100% — caller should advance.
    Advance { pct: u8 },
    /// `manual_only` set; the daemon should never auto-act.
    ManualOnly,
}

/// Stateless threshold computation.
pub struct ThresholdChecker;

impl ThresholdChecker {
    /// Evaluate the current pipeline state and decide on next action.
    pub fn evaluate(
        cfg: &PipelineConfig,
        state: &PipelineState,
        stats: &SessionStats,
        now: DateTime<Utc>,
    ) -> CheckResult {
        if cfg.advance_when.manual_only {
            return CheckResult::ManualOnly;
        }
        let pct = Self::current_pct(cfg, state, stats, now);
        if pct >= 100 {
            CheckResult::Advance { pct }
        } else if pct >= cfg.notify_at_pct
            && state.notified_at_pct.is_none_or(|n| n < cfg.notify_at_pct)
        {
            CheckResult::Notify { pct }
        } else {
            CheckResult::BelowNotify { pct }
        }
    }

    /// Compute the current percentage of the threshold consumed (capped at 100).
    pub fn current_pct(
        cfg: &PipelineConfig,
        state: &PipelineState,
        stats: &SessionStats,
        now: DateTime<Utc>,
    ) -> u8 {
        let w = &cfg.advance_when;
        if let Some(threshold) = w.tokens_used.or(w.tokens_used_weekly) {
            let total = stats.tokens_in.saturating_add(stats.tokens_out);
            let baseline = state
                .baseline_tokens_in
                .saturating_add(state.baseline_tokens_out);
            let used = total.saturating_sub(baseline);
            if threshold == 0 {
                return 0;
            }
            return ((used as f64 / threshold as f64 * 100.0).min(100.0)) as u8;
        }
        if let Some(hours) = w.hours_active {
            let start = state.window_started.or(stats.first_used).unwrap_or(now);
            let elapsed_secs = (now - start).num_seconds().max(0) as f64;
            let elapsed_hours = elapsed_secs / 3600.0;
            if hours <= 0.0 {
                return 0;
            }
            return ((elapsed_hours / hours * 100.0).min(100.0)) as u8;
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::config::{AdvanceWhen, Weekday};
    use chrono::TimeZone;

    fn cfg_tokens(threshold: u64, notify: u8) -> PipelineConfig {
        PipelineConfig {
            next: "backup".into(),
            notify_at_pct: notify,
            auto_advance: true,
            advance_when: AdvanceWhen {
                tokens_used: Some(threshold),
                ..Default::default()
            },
        }
    }

    fn cfg_hours(hours: f64, notify: u8) -> PipelineConfig {
        PipelineConfig {
            next: "backup".into(),
            notify_at_pct: notify,
            auto_advance: true,
            advance_when: AdvanceWhen {
                hours_active: Some(hours),
                ..Default::default()
            },
        }
    }

    fn cfg_weekly(threshold: u64, notify: u8) -> PipelineConfig {
        PipelineConfig {
            next: "backup".into(),
            notify_at_pct: notify,
            auto_advance: true,
            advance_when: AdvanceWhen {
                tokens_used_weekly: Some(threshold),
                reset_day: Some(Weekday::Monday),
                ..Default::default()
            },
        }
    }

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 10, 12, 0, 0).unwrap()
    }

    #[test]
    fn test_manual_only_returns_manual() {
        let cfg = PipelineConfig {
            next: "x".into(),
            notify_at_pct: 80,
            auto_advance: false,
            advance_when: AdvanceWhen {
                manual_only: true,
                ..Default::default()
            },
        };
        let r = ThresholdChecker::evaluate(
            &cfg,
            &PipelineState::default(),
            &SessionStats::default(),
            now(),
        );
        assert_eq!(r, CheckResult::ManualOnly);
    }

    #[test]
    fn test_below_notify_tokens() {
        let cfg = cfg_tokens(1000, 80);
        let stats = SessionStats {
            tokens_in: 100,
            tokens_out: 100,
            ..Default::default()
        };
        let r = ThresholdChecker::evaluate(&cfg, &PipelineState::default(), &stats, now());
        match r {
            CheckResult::BelowNotify { pct } => assert_eq!(pct, 20),
            _ => panic!("expected BelowNotify, got {r:?}"),
        }
    }

    #[test]
    fn test_notify_first_crossing() {
        let cfg = cfg_tokens(1000, 80);
        let stats = SessionStats {
            tokens_in: 500,
            tokens_out: 350,
            ..Default::default()
        };
        let r = ThresholdChecker::evaluate(&cfg, &PipelineState::default(), &stats, now());
        match r {
            CheckResult::Notify { pct } => assert_eq!(pct, 85),
            _ => panic!("expected Notify, got {r:?}"),
        }
    }

    #[test]
    fn test_notify_already_announced_returns_below() {
        let cfg = cfg_tokens(1000, 80);
        let stats = SessionStats {
            tokens_in: 500,
            tokens_out: 350,
            ..Default::default()
        };
        let state = PipelineState {
            notified_at_pct: Some(80),
            ..Default::default()
        };
        let r = ThresholdChecker::evaluate(&cfg, &state, &stats, now());
        // Already notified → BelowNotify (no re-fire), even though pct >= notify_at_pct
        match r {
            CheckResult::BelowNotify { pct } => assert_eq!(pct, 85),
            _ => panic!("expected BelowNotify, got {r:?}"),
        }
    }

    #[test]
    fn test_advance_at_100() {
        let cfg = cfg_tokens(1000, 80);
        let stats = SessionStats {
            tokens_in: 700,
            tokens_out: 400,
            ..Default::default()
        };
        let r = ThresholdChecker::evaluate(&cfg, &PipelineState::default(), &stats, now());
        assert!(matches!(r, CheckResult::Advance { pct: 100 }));
    }

    #[test]
    fn test_advance_capped_at_100() {
        let cfg = cfg_tokens(1000, 80);
        let stats = SessionStats {
            tokens_in: 5000,
            tokens_out: 5000,
            ..Default::default()
        };
        let r = ThresholdChecker::evaluate(&cfg, &PipelineState::default(), &stats, now());
        assert!(matches!(r, CheckResult::Advance { pct: 100 }));
    }

    #[test]
    fn test_baseline_subtracts_correctly() {
        let cfg = cfg_tokens(1000, 80);
        let stats = SessionStats {
            tokens_in: 1000,
            tokens_out: 100,
            ..Default::default()
        };
        let state = PipelineState {
            baseline_tokens_in: 800,
            baseline_tokens_out: 100,
            ..Default::default()
        };
        // used = (1000+100) - (800+100) = 200; pct = 20
        let r = ThresholdChecker::evaluate(&cfg, &state, &stats, now());
        match r {
            CheckResult::BelowNotify { pct } => assert_eq!(pct, 20),
            _ => panic!("expected BelowNotify, got {r:?}"),
        }
    }

    #[test]
    fn test_hours_below_notify() {
        let cfg = cfg_hours(10.0, 80);
        let start = now() - chrono::Duration::hours(1);
        let state = PipelineState {
            window_started: Some(start),
            ..Default::default()
        };
        let r = ThresholdChecker::evaluate(&cfg, &state, &SessionStats::default(), now());
        match r {
            CheckResult::BelowNotify { pct } => assert_eq!(pct, 10),
            _ => panic!("expected BelowNotify, got {r:?}"),
        }
    }

    #[test]
    fn test_hours_advance() {
        let cfg = cfg_hours(2.0, 80);
        let start = now() - chrono::Duration::hours(3);
        let state = PipelineState {
            window_started: Some(start),
            ..Default::default()
        };
        let r = ThresholdChecker::evaluate(&cfg, &state, &SessionStats::default(), now());
        assert!(matches!(r, CheckResult::Advance { pct: 100 }));
    }

    #[test]
    fn test_hours_uses_first_used_when_window_not_set() {
        let cfg = cfg_hours(2.0, 80);
        let stats = SessionStats {
            first_used: Some(now() - chrono::Duration::hours(1)),
            ..Default::default()
        };
        let r = ThresholdChecker::evaluate(&cfg, &PipelineState::default(), &stats, now());
        match r {
            CheckResult::BelowNotify { pct } => assert_eq!(pct, 50),
            _ => panic!("expected BelowNotify pct=50, got {r:?}"),
        }
    }

    #[test]
    fn test_weekly_threshold_uses_baseline() {
        let cfg = cfg_weekly(1000, 80);
        let stats = SessionStats {
            tokens_in: 1500,
            tokens_out: 0,
            ..Default::default()
        };
        let state = PipelineState {
            baseline_tokens_in: 1000,
            baseline_tokens_out: 0,
            ..Default::default()
        };
        // used = 1500 - 1000 = 500; pct = 50
        let r = ThresholdChecker::evaluate(&cfg, &state, &stats, now());
        match r {
            CheckResult::BelowNotify { pct } => assert_eq!(pct, 50),
            _ => panic!("expected BelowNotify, got {r:?}"),
        }
    }

    #[test]
    fn test_no_threshold_returns_zero() {
        let cfg = PipelineConfig {
            next: "x".into(),
            notify_at_pct: 80,
            auto_advance: true,
            advance_when: AdvanceWhen::default(),
        };
        let pct = ThresholdChecker::current_pct(
            &cfg,
            &PipelineState::default(),
            &SessionStats::default(),
            now(),
        );
        assert_eq!(pct, 0);
    }

    #[test]
    fn test_notify_at_zero_pct() {
        let cfg = cfg_tokens(1000, 0);
        let stats = SessionStats::default();
        // pct = 0, notify_at_pct = 0, never notified before → first cross
        let r = ThresholdChecker::evaluate(&cfg, &PipelineState::default(), &stats, now());
        match r {
            CheckResult::Notify { pct } => assert_eq!(pct, 0),
            _ => panic!("expected Notify, got {r:?}"),
        }
    }
}
