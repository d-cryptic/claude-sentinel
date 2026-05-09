//! `cst-core` — shared library for claude-sentinel.
//!
//! All domain logic lives here. No direct CLI I/O in this crate.
//! The CLI (`cst-cli`) and Tauri app both depend on this crate.

pub mod auth;
pub mod auto_detect;
pub mod auto_switch;
pub mod broadcast;
pub mod config;
pub mod env_overlay;
pub mod history_parser;
pub mod hooks;
pub mod mcp;
pub mod merge;
pub mod platform;
pub mod profile;
pub mod session;
pub mod shell;
pub mod stats;
pub mod team_sync;
pub mod templates;

// Re-export top-level types
pub use config::GlobalConfig;
pub use profile::{validate_profile_name, Profile, ProfileManager};
pub use session::{validate_session_name, Session, SessionManager};
