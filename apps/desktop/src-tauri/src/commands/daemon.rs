//! Daemon management Tauri commands.

use cst_core::auto_switch::daemon;
use cst_core::auto_switch::scheduler::SchedulerState;
use cst_core::auto_switch::switch_log::SwitchLog;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub running: bool,
    pub active_timers: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwitchEventDto {
    pub timestamp: String,
    pub from_profile: String,
    pub from_session: String,
    pub to_profile: String,
    pub to_session: String,
    pub reason: String,
    pub detail: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SchedulerEntryDto {
    pub profile: String,
    pub detected_at: String,
    pub refill_at: String,
    pub time_until_refill: String,
    pub auto_switch_back: bool,
    pub switched_back: bool,
}

#[tauri::command]
pub fn daemon_status() -> DaemonStatus {
    let running = daemon::is_running();
    let active_timers = if running {
        SchedulerState::load()
            .map(|s| s.entries.iter().filter(|e| !e.switched_back).count())
            .unwrap_or(0)
    } else { 0 };
    DaemonStatus { running, active_timers }
}

#[tauri::command]
pub async fn daemon_start() -> Result<(), String> {
    if daemon::is_running() {
        return Ok(());
    }
    // Spawn daemon in background task
    tokio::spawn(async {
        let _ = daemon::run_daemon().await;
    });
    Ok(())
}

#[tauri::command]
pub fn daemon_stop() -> Result<(), String> {
    daemon::stop_daemon().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_switch_log() -> Result<Vec<SwitchEventDto>, String> {
    let log = SwitchLog::open();
    let events = log.last_n(50).map_err(|e| e.to_string())?;
    Ok(events.into_iter().map(|ev| SwitchEventDto {
        timestamp: ev.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        from_profile: ev.from_profile,
        from_session: ev.from_session,
        to_profile: ev.to_profile,
        to_session: ev.to_session,
        reason: ev.reason.to_string(),
        detail: ev.detail,
    }).collect())
}

#[tauri::command]
pub fn get_scheduler_state() -> Result<Vec<SchedulerEntryDto>, String> {
    let state = SchedulerState::load().map_err(|e| e.to_string())?;
    Ok(state.entries.into_iter().map(|e| SchedulerEntryDto {
        time_until_refill: e.time_until_refill(),
        profile: e.profile,
        detected_at: e.detected_at.format("%H:%M:%S UTC").to_string(),
        refill_at: e.refill_at.format("%H:%M:%S UTC").to_string(),
        auto_switch_back: e.auto_switch_back,
        switched_back: e.switched_back,
    }).collect())
}
