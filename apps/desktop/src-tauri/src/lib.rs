//! Tauri app backend — exposes cst-core functionality to the React frontend.

mod commands;
mod tray;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("claude_sentinel=info".parse().unwrap()),
        )
        .with_target(false)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            tray::setup_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::profiles::list_profiles,
            commands::profiles::get_active,
            commands::profiles::switch_profile,
            commands::profiles::create_profile,
            commands::profiles::delete_profile,
            commands::sessions::list_sessions,
            commands::sessions::create_session,
            commands::sessions::delete_session,
            commands::daemon::daemon_status,
            commands::daemon::daemon_start,
            commands::daemon::daemon_stop,
            commands::daemon::get_switch_log,
            commands::daemon::get_scheduler_state,
            commands::stats::get_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error running claude-sentinel app");
}
