//! System tray / menu bar setup.

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    App, Manager, Runtime,
};

use cst_core::config::GlobalConfig;

pub fn setup_tray<R: Runtime>(app: &mut App<R>) -> tauri::Result<()> {
    let cfg = GlobalConfig::load().unwrap_or_default();
    let active = if cfg.current_profile.is_empty() {
        "No profile".to_string()
    } else {
        format!("{}:{}", cfg.current_profile, cfg.current_session)
    };

    // Build tray menu
    let active_item = MenuItem::with_id(app, "active", format!("Active: {}", active), false, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let open_item = MenuItem::with_id(app, "open", "Open Sentinel", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&active_item, &separator, &open_item, &separator, &quit_item])?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, .. } = event {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}
