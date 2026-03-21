use anyhow::Result;
use cst_core::auto_switch::daemon;
use cst_core::auto_switch::switch_log::SwitchLog;

pub async fn dispatch(action: crate::DaemonCommands) -> Result<()> {
    match action {
        crate::DaemonCommands::Start => start().await,
        crate::DaemonCommands::Stop => stop(),
        crate::DaemonCommands::Restart => { stop()?; start().await }
        crate::DaemonCommands::Status => status(),
        crate::DaemonCommands::Logs => logs(),
    }
}

pub async fn start() -> Result<()> {
    if daemon::is_running() {
        println!("Daemon is already running.");
        return Ok(());
    }
    println!("Starting claude-sentinel daemon...");
    // Spawn as a background process by re-invoking with a hidden flag
    // In a real install this would use launchd/systemd/Task Scheduler.
    // For now, run inline (detached via tokio spawn_blocking in the future).
    daemon::run_daemon().await
}

pub fn stop() -> Result<()> {
    daemon::stop_daemon()?;
    println!("Daemon stopped.");
    Ok(())
}

pub fn status() -> Result<()> {
    if daemon::is_running() {
        println!("Daemon: running");
        // Show pending scheduler state
        if let Ok(sched) = cst_core::auto_switch::scheduler::SchedulerState::load() {
            for entry in &sched.entries {
                if !entry.switched_back {
                    println!(
                        "  {} — rate-limited at {}, refill in {}",
                        entry.profile,
                        entry.detected_at.format("%H:%M:%S"),
                        entry.time_until_refill()
                    );
                }
            }
        }
    } else {
        println!("Daemon: not running");
    }
    Ok(())
}

pub fn logs() -> Result<()> {
    let log = SwitchLog::open();
    let events = log.last_n(20)?;
    if events.is_empty() {
        println!("No switch events recorded yet.");
        return Ok(());
    }
    println!("{:<24} {:<20} {:<20} {}", "TIMESTAMP", "FROM", "TO", "REASON");
    println!("{}", "─".repeat(80));
    for ev in &events {
        println!(
            "{:<24} {:<20} {:<20} {}",
            ev.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            format!("{}:{}", ev.from_profile, ev.from_session),
            format!("{}:{}", ev.to_profile, ev.to_session),
            format!("{} — {}", ev.reason, ev.detail),
        );
    }
    Ok(())
}
