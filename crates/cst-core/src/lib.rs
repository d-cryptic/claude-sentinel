//! `cst-core` — shared library for claude-sentinel.
//!
//! All domain logic lives here. No direct CLI I/O in this crate.
//! The CLI (`cst-cli`) and Tauri app both depend on this crate.

pub mod auth;
pub mod config;
pub mod env_overlay;
pub mod hooks;
pub mod merge;
pub mod mcp;
pub mod platform;
pub mod profile;
pub mod session;
pub mod shell;
pub mod stats;
pub mod templates;

// Re-export top-level types
pub use config::GlobalConfig;
pub use profile::{Profile, ProfileManager};
pub use session::{Session, SessionManager};
