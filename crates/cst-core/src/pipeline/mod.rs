//! Account pipeline — declarative, user-driven profile rotation.
//!
//! A pipeline lets the user say "when this profile has used X tokens or
//! Y hours, advance to the next profile". Pipelines never react to
//! Anthropic rate-limit signals; only to user-declared budgets.

pub mod advance;
pub mod config;
pub mod notify;
pub mod state;
pub mod threshold;

pub use advance::{advance_now, tick, TickReport};
pub use config::{AdvanceWhen, PipelineConfig, Weekday};
pub use state::PipelineState;
pub use threshold::{CheckResult, ThresholdChecker};
