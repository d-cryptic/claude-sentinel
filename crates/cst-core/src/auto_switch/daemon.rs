//! Background daemon — performs time-based profile switches.
//!
//! NOTE (compliance): rate-limit-triggered profile switching has been removed.
//! Anthropic's Acceptable Use Policy prohibits bypassing rate limit guardrails,
//! so the daemon no longer monitors `history.jsonl` for HTTP 429 / rate-limit
//! patterns. Switches are driven exclusively by the user-configured
//! `active_hours` schedule and explicit `cst use` invocations.

use anyhow::Result;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time;

use crate::platform;
use crate::shell::shell_escape_single_quote;

/// PID file path.
pub fn pid_file() -> PathBuf {
    platform::data_dir().join("daemon.pid")
}

/// Pending-switch file — the shell precmd hook reads this.
pub fn pending_switch_file() -> PathBuf {
    platform::data_dir().join("pending-switch")
}

/// Write the pending-switch file so the shell can pick it up.
pub fn write_pending_switch(profile: &str, session: &str) -> Result<()> {
    let path = pending_switch_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Single-quote all values so the shell cannot break out of them.
    // Profile/session names are [a-zA-Z0-9\-_] by validate_name, but the
    // config_dir path (on Windows or non-standard setups) may contain quotes.
    let config_dir =
        shell_escape_single_quote(&platform::claude_config_dir(profile, session).display().to_string());
    let content = format!(
        "export CST_CURRENT='{profile}:{session}'\nexport CLAUDE_CONFIG_DIR='{config_dir}'\n"
    );
    std::fs::write(path, content)?;
    Ok(())
}


/// Core daemon loop. Runs until cancelled.
///
/// Strategy (time-based only):
/// 1. Honour the pause file written by `cst pause` / `cst unpause`.
/// 2. Sleep in short ticks so the process remains responsive to signals.
///
/// Future time-based scheduling (active_hours / timezone) plugs in here.
pub async fn run_daemon() -> Result<()> {
    // Switches are time-based only — rate limit signals do not trigger profile changes.
    tracing::info!("claude-sentinel daemon starting (time-based scheduling only)");

    // Write PID file
    let pid = std::process::id();
    let pid_path = pid_file();
    if let Some(parent) = pid_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&pid_path, pid.to_string())?;

    loop {
        // Switches are time-based only — rate limit signals do not trigger profile changes.

        // Honour the pause file written by `cst pause`.
        let pause_path = platform::data_dir().join("auto-switch-paused");
        let _paused = if pause_path.exists() {
            match std::fs::read_to_string(&pause_path) {
                Ok(content) if content.trim() == "indefinite" => true,
                Ok(content) => {
                    // Check if the resume timestamp has passed.
                    if let Ok(dt) = content.trim().parse::<chrono::DateTime<chrono::Utc>>() {
                        chrono::Utc::now() < dt
                    } else {
                        false
                    }
                }
                Err(_) => false,
            }
        } else {
            false
        };

        // TODO: evaluate `active_hours` schedule for the current profile and
        // switch to its `fallback` profile when outside the active window.
        // This is intentionally a no-op until the time-based scheduler lands.

        // Pipeline threshold check — time-based and user-declared usage gates only.
        if !_paused {
            if let Ok(cfg) = crate::GlobalConfig::load() {
                if !cfg.current_profile.is_empty() {
                    if let Err(e) = crate::pipeline::advance::tick(&cfg.current_profile) {
                        tracing::warn!("pipeline tick error: {e}");
                    }
                }
            }
        }

        time::sleep(Duration::from_secs(30)).await;
    }
}

/// Check if the daemon is running by inspecting the PID file.
pub fn is_running() -> bool {
    let path = pid_file();
    if !path.exists() {
        return false;
    }
    let Ok(pid_str) = std::fs::read_to_string(&path) else {
        return false;
    };
    let Ok(pid) = pid_str.trim().parse::<u32>() else {
        return false;
    };
    // Check if process exists
    #[cfg(unix)]
    {
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        // On non-Unix platforms, assume running if PID file exists
        let _ = pid;
        true
    }
}

/// Stop the daemon by sending SIGTERM and waiting for the process to exit.
///
/// The PID file is only removed after the process has actually terminated
/// (or a 5-second timeout elapses), preventing a TOCTOU race where a
/// concurrent `cst daemon start` sees no PID file and spawns a second daemon
/// before the first one has exited.
pub fn stop_daemon() -> Result<()> {
    let path = pid_file();
    if !path.exists() {
        anyhow::bail!("daemon is not running (no PID file)");
    }
    let pid_str = std::fs::read_to_string(&path)?;
    let pid: u32 = pid_str.trim().parse()?;

    #[cfg(unix)]
    {
        unsafe {
            if libc::kill(pid as libc::pid_t, libc::SIGTERM) != 0 {
                anyhow::bail!("failed to send SIGTERM to PID {pid}");
            }
        }

        // Poll until the process exits or a 5-second timeout elapses.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let still_running = unsafe { libc::kill(pid as libc::pid_t, 0) == 0 };
            if !still_running {
                break;
            }
            if std::time::Instant::now() >= deadline {
                tracing::warn!("daemon PID {pid} did not exit within 5 s after SIGTERM");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    #[cfg(not(unix))]
    {
        tracing::warn!("daemon stop not fully implemented on this platform (PID: {pid})");
    }

    std::fs::remove_file(&path)?;
    tracing::info!("daemon stopped (PID {pid})");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    // Serialise tests that mutate CST_DATA_DIR to prevent parallel-test UB.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_write_pending_switch_normal_path() {
        let dir = TempDir::new().unwrap();
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: we hold ENV_LOCK.
        unsafe { std::env::set_var("CST_DATA_DIR", dir.path().to_str().unwrap()) }
        let result = write_pending_switch("work", "backend");
        let content = std::fs::read_to_string(dir.path().join("pending-switch")).ok();
        unsafe { std::env::remove_var("CST_DATA_DIR") }
        result.unwrap();
        let content = content.unwrap();
        assert!(content.contains("CST_CURRENT='work:backend'"));
        assert!(content.contains("CLAUDE_CONFIG_DIR='"));
        assert!(!content.contains('"'), "should use single-quoted values");
    }

    #[test]
    fn test_write_pending_switch_no_injection_via_path() {
        let dir = TempDir::new().unwrap();
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: we hold ENV_LOCK.
        unsafe { std::env::set_var("CST_DATA_DIR", dir.path().to_str().unwrap()) }
        let result = write_pending_switch("my-profile", "default");
        let content = std::fs::read_to_string(dir.path().join("pending-switch")).ok();
        unsafe { std::env::remove_var("CST_DATA_DIR") }
        result.unwrap();
        assert!(content.unwrap().starts_with("export CST_CURRENT='"));
    }
}
