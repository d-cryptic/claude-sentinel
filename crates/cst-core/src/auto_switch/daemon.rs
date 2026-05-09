//! Background daemon — watches history.jsonl files, detects rate limits,
//! triggers profile switches, and schedules switch-backs.

use anyhow::Result;
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;
use tokio::time;

use crate::auto_switch::config::AutoSwitchConfig;
use crate::auto_switch::detector;
use crate::auto_switch::scheduler::SchedulerState;
use crate::auto_switch::switch_log::{SwitchEvent, SwitchLog, SwitchReason};
use crate::config::GlobalConfig;
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

/// Find all `history.jsonl` files under the sentinel data dir to watch.
fn find_history_files() -> Vec<PathBuf> {
    let profiles_dir = platform::profiles_dir();
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&profiles_dir) {
        for entry in entries.flatten() {
            let profile = entry.file_name().to_string_lossy().to_string();
            let sessions_dir = entry.path().join("sessions");
            if let Ok(sessions) = std::fs::read_dir(&sessions_dir) {
                for s_entry in sessions.flatten() {
                    let history = s_entry.path().join(".claude").join("history.jsonl");
                    if history.exists() {
                        files.push(history);
                    }
                    // Also watch the session dir for new files
                    let _ = profile.as_str(); // used in closure below via captured var
                }
            }
        }
    }
    files
}

/// Core daemon loop. Runs until cancelled.
///
/// Strategy:
/// 1. Watch all `history.jsonl` files for writes.
/// 2. On change: tail new lines, scan for rate-limit patterns.
/// 3. On detection: look up current profile's fallback chain, switch to next.
/// 4. Every 60 s: check scheduler for pending switch-backs.
pub async fn run_daemon() -> Result<()> {
    tracing::info!("claude-sentinel daemon starting");

    // Write PID file
    let pid = std::process::id();
    let pid_path = pid_file();
    if let Some(parent) = pid_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&pid_path, pid.to_string())?;

    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.send(res);
        },
        NotifyConfig::default().with_poll_interval(Duration::from_secs(2)),
    )?;

    // Watch the entire data dir recursively so new sessions are picked up
    let data_dir = platform::data_dir();
    std::fs::create_dir_all(&data_dir)?;
    watcher.watch(&data_dir, RecursiveMode::Recursive)?;

    // Also watch ~/.claude/ for global history
    let global_claude = platform::global_claude_dir();
    if global_claude.exists() {
        let _ = watcher.watch(&global_claude, RecursiveMode::NonRecursive);
    }

    let switch_log = SwitchLog::open();
    let mut scheduler = SchedulerState::load().unwrap_or_default();
    let mut last_scheduler_check = std::time::Instant::now();

    tracing::info!("daemon watching for rate limits");

    loop {
        // Poll file-system events (non-blocking)
        while let Ok(Ok(event)) = rx.try_recv() {
            for path in &event.paths {
                if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                    if let Err(e) = handle_file_change(path, &mut scheduler, &switch_log) {
                        tracing::warn!("error processing change to {}: {e}", path.display());
                    }
                }
            }
        }

        // Periodic scheduler check (every 60 s)
        if last_scheduler_check.elapsed() >= Duration::from_secs(60) {
            if let Err(e) = check_scheduler_switchbacks(&mut scheduler, &switch_log) {
                tracing::warn!("scheduler check error: {e}");
            }
            last_scheduler_check = std::time::Instant::now();
        }

        time::sleep(Duration::from_millis(500)).await;
    }
}

/// Read the last `max_bytes` of a file, returning a String.
///
/// History files can grow to hundreds of MB; we only need the most recent
/// lines, so we seek near the end rather than loading the whole file.
///
/// Non-UTF-8 bytes (e.g. from user source files embedded in tool results)
/// are replaced with the Unicode replacement character so detection is never
/// silenced by a bad byte sequence.
fn read_tail(path: &std::path::Path, max_bytes: u64) -> Result<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path)?;
    let len = f.metadata()?.len();
    let start = len.saturating_sub(max_bytes);
    f.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Called when a `.jsonl` file changes — scan new lines for rate-limit patterns.
fn handle_file_change(
    path: &std::path::Path,
    scheduler: &mut SchedulerState,
    switch_log: &SwitchLog,
) -> Result<()> {
    // Read only the last 8 KiB — avoids loading large history files into memory.
    // At ~200 bytes per JSON line this covers ~40 recent lines, well beyond the 20 we inspect.
    let tail = read_tail(path, 8 * 1024)?;
    // Scan last 20 complete lines (skip the possibly-partial first line at the seek boundary)
    let lines: Vec<&str> = tail.lines().rev().take(20).collect();
    for line in lines {
        if detector::is_rate_limit_line(line) {
            let reason = detector::extract_reason(line);
            tracing::info!("rate limit detected in {}: {reason}", path.display());
            trigger_switch(scheduler, switch_log, &reason)?;
            break;
        }
    }
    Ok(())
}

/// Perform the actual profile switch when a rate limit is detected.
fn trigger_switch(
    scheduler: &mut SchedulerState,
    switch_log: &SwitchLog,
    reason: &str,
) -> Result<()> {
    let cfg = GlobalConfig::load()?;
    let current_profile = cfg.current_profile.clone();
    let current_session = cfg.current_session.clone();

    if current_profile.is_empty() {
        tracing::warn!("no active profile, cannot auto-switch");
        return Ok(());
    }

    // Load this profile's auto-switch config
    let profile_dir = platform::profile_dir(&current_profile);
    let as_cfg = AutoSwitchConfig::load(&profile_dir)?;

    if as_cfg.fallback_chain.is_empty() {
        tracing::info!("no fallback chain configured for {current_profile}, pausing");
        return Ok(());
    }

    // Pick next profile in chain (skip if it's the current one)
    let target = as_cfg
        .fallback_chain
        .iter()
        .find(|p| p.as_str() != current_profile)
        .cloned();

    let Some(target_profile) = target else {
        tracing::warn!("fallback chain has no alternative to {current_profile}");
        return Ok(());
    };

    // Record rate limit in scheduler for future switch-back
    scheduler.record_rate_limit(
        &current_profile,
        as_cfg.estimate_minutes,
        as_cfg.auto_switch_back,
    );
    let _ = scheduler.save();

    // Write pending-switch for shell precmd hook
    write_pending_switch(&target_profile, "default")?;

    // Log the event
    let event = SwitchEvent {
        timestamp: chrono::Utc::now(),
        from_profile: current_profile.clone(),
        from_session: current_session,
        to_profile: target_profile.clone(),
        to_session: "default".to_string(),
        reason: SwitchReason::RateLimit,
        detail: reason.to_string(),
    };
    switch_log.append(&event)?;

    // Update global config
    let mut new_cfg = cfg;
    new_cfg.current_profile = target_profile.clone();
    new_cfg.current_session = "default".to_string();
    new_cfg.save()?;

    tracing::info!("auto-switched from {current_profile} to {target_profile}: {reason}");

    // macOS notification (best-effort)
    #[cfg(target_os = "macos")]
    if as_cfg.notify {
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(format!(
                r#"display notification "Switched to {target_profile}" with title "Claude Sentinel" subtitle "Rate limit on {current_profile}""#
            ))
            .spawn();
    }

    Ok(())
}

/// Check scheduler for profiles whose quota has refilled — switch back if needed.
fn check_scheduler_switchbacks(
    scheduler: &mut SchedulerState,
    switch_log: &SwitchLog,
) -> Result<()> {
    let pending: Vec<String> = scheduler
        .pending_switchbacks()
        .iter()
        .map(|e| e.profile.clone())
        .collect();

    for profile in pending {
        tracing::info!("quota refilled for {profile}, switching back");

        let cfg = GlobalConfig::load()?;
        let current = cfg.current_profile.clone();

        // Write pending switch back to original profile
        write_pending_switch(&profile, "default")?;

        let event = SwitchEvent {
            timestamp: chrono::Utc::now(),
            from_profile: current,
            from_session: cfg.current_session.clone(),
            to_profile: profile.clone(),
            to_session: "default".to_string(),
            reason: SwitchReason::QuotaRefill,
            detail: format!("estimated quota refill for {profile}"),
        };
        switch_log.append(&event)?;

        let mut new_cfg = cfg;
        new_cfg.current_profile = profile.clone();
        new_cfg.current_session = "default".to_string();
        new_cfg.save()?;

        scheduler.mark_switched_back(&profile);
        scheduler.save()?;
    }

    Ok(())
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
