//! Platform-specific path resolution and utilities.
//!
//! Centralises all `dirs` crate usage so the rest of the codebase
//! never needs platform `#[cfg]` branches for path resolution.

use std::path::PathBuf;

/// Root data directory for claude-sentinel.
///
/// Checks the `CST_DATA_DIR` environment variable first, allowing
/// integration tests to point to a temporary directory.
///
/// Fallback hierarchy:
/// - macOS/Linux: `~/.claude-sentinel/`
/// - Windows:     `%APPDATA%\claude-sentinel\`
pub fn data_dir() -> PathBuf {
    if let Ok(d) = std::env::var("CST_DATA_DIR") {
        return PathBuf::from(d);
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| dirs::home_dir().expect("home dir must exist"))
        .join("claude-sentinel")
}

/// Directory that holds all profiles.
pub fn profiles_dir() -> PathBuf {
    data_dir().join("profiles")
}

/// Directory for a specific profile.
pub fn profile_dir(name: &str) -> PathBuf {
    profiles_dir().join(name)
}

/// Directory for a specific session within a profile.
pub fn session_dir(profile: &str, session: &str) -> PathBuf {
    profile_dir(profile).join("sessions").join(session)
}

/// The `CLAUDE_CONFIG_DIR` target for a given profile:session.
pub fn claude_config_dir(profile: &str, session: &str) -> PathBuf {
    session_dir(profile, session).join(".claude")
}

/// Global `~/.claude/` directory (shared base config).
pub fn global_claude_dir() -> PathBuf {
    dirs::home_dir()
        .expect("home dir must exist")
        .join(".claude")
}

/// Global `~/.claude.json` OAuth credentials file.
pub fn global_claude_json() -> PathBuf {
    dirs::home_dir()
        .expect("home dir must exist")
        .join(".claude.json")
}

/// Global `~/.claude-sentinel/config.toml` — current state.
pub fn global_config_path() -> PathBuf {
    data_dir().join("config.toml")
}

/// IPC socket / named pipe path for daemon communication.
pub fn ipc_socket_path() -> PathBuf {
    data_dir().join("daemon.sock")
}

/// Pending-switch file: daemon writes here, shell precmd reads and evals.
pub fn pending_switch_path() -> PathBuf {
    data_dir().join("pending-switch")
}

/// Create all required directories (called on first run / init).
pub fn ensure_dirs_exist() -> anyhow::Result<()> {
    std::fs::create_dir_all(data_dir())?;
    std::fs::create_dir_all(profiles_dir())?;
    Ok(())
}

/// Create a symlink (or junction on Windows).
pub fn create_link(src: &std::path::Path, dst: &std::path::Path) -> anyhow::Result<()> {
    if dst.exists() || dst.is_symlink() {
        std::fs::remove_file(dst).or_else(|_| std::fs::remove_dir(dst))?;
    }
    _create_link_impl(src, dst)
}

#[cfg(unix)]
fn _create_link_impl(src: &std::path::Path, dst: &std::path::Path) -> anyhow::Result<()> {
    std::os::unix::fs::symlink(src, dst)?;
    Ok(())
}

#[cfg(windows)]
fn _create_link_impl(src: &std::path::Path, dst: &std::path::Path) -> anyhow::Result<()> {
    use anyhow::Context;
    // Try junction for directories, symlink for files
    if src.is_dir() {
        junction::create(src, dst).context("failed to create junction")?;
    } else {
        std::os::windows::fs::symlink_file(src, dst)
            .context("failed to create symlink (try enabling Developer Mode)")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_dir_is_absolute() {
        assert!(data_dir().is_absolute());
    }

    #[test]
    fn test_data_dir_respects_env_override() {
        let _guard = std::env::var("CST_DATA_DIR").ok();
        std::env::set_var("CST_DATA_DIR", "/tmp/cst-test-data");
        assert_eq!(data_dir(), PathBuf::from("/tmp/cst-test-data"));
        std::env::remove_var("CST_DATA_DIR");
    }

    #[test]
    fn test_profile_dir_contains_profile_name() {
        let dir = profile_dir("work");
        assert!(dir.to_string_lossy().contains("work"));
    }

    #[test]
    fn test_session_dir_nesting() {
        let dir = session_dir("work", "backend");
        let s = dir.to_string_lossy();
        assert!(s.contains("work"));
        assert!(s.contains("backend"));
    }

    #[test]
    fn test_claude_config_dir_ends_with_dot_claude() {
        let dir = claude_config_dir("work", "default");
        assert_eq!(dir.file_name().unwrap(), ".claude");
    }
}
