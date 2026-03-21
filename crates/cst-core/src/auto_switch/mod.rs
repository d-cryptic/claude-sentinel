//! Auto-switch pipeline: rate-limit detection → profile fallback → quota reset scheduler.

pub mod config;
pub mod daemon;
pub mod detector;
pub mod scheduler;
pub mod switch_log;

pub use config::AutoSwitchConfig;
pub use switch_log::{SwitchEvent, SwitchLog, SwitchReason};
