//! Profile management Tauri commands.

use cst_core::auto_switch::daemon::write_pending_switch;
use cst_core::config::GlobalConfig;
use cst_core::platform;
use cst_core::profile::{AuthType, Profile, ProfileManager};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileDto {
    pub name: String,
    pub auth_type: String,
    pub description: String,
    pub is_active: bool,
    pub sessions: Vec<String>,
    pub color: String,
}

#[tauri::command]
pub fn list_profiles() -> Result<Vec<ProfileDto>, String> {
    let mgr = ProfileManager::new(platform::profiles_dir());
    let cfg = GlobalConfig::load().unwrap_or_default();
    let profiles = mgr.list().map_err(|e| e.to_string())?;

    Ok(profiles.into_iter().map(|p| {
        let profile_dir = platform::profile_dir(&p.name);
        let smgr = cst_core::session::SessionManager::new(profile_dir.join("sessions"));
        let sessions = smgr.list().unwrap_or_default()
            .into_iter().map(|s| s.name).collect();
        ProfileDto {
            is_active: p.name == cfg.current_profile,
            name: p.name,
            auth_type: p.auth_type.to_string(),
            description: p.description,
            sessions,
            color: p.color,
        }
    }).collect())
}

#[tauri::command]
pub fn get_active() -> Result<serde_json::Value, String> {
    let cfg = GlobalConfig::load().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "profile": cfg.current_profile,
        "session": cfg.current_session,
    }))
}

#[tauri::command]
pub fn switch_profile(profile: String, session: String) -> Result<(), String> {
    // Write pending switch for shell
    write_pending_switch(&profile, &session).map_err(|e| e.to_string())?;

    // Update global config
    let mut cfg = GlobalConfig::load().unwrap_or_default();
    cfg.current_profile = profile;
    cfg.current_session = session;
    cfg.save().map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn create_profile(name: String, auth_type: String, template: Option<String>) -> Result<ProfileDto, String> {
    let at = AuthType::from_str(&auth_type).map_err(|e| e.to_string())?;
    let mgr = ProfileManager::new(platform::profiles_dir());
    let profile = mgr.create(&name, at).map_err(|e| e.to_string())?;

    // Apply template settings override if requested
    if let Some(tpl_name) = template {
        if let Some(tpl) = cst_core::templates::find(&tpl_name) {
            let override_path = platform::profile_dir(&name).join("settings-override.json");
            let _ = std::fs::write(override_path, serde_json::to_string_pretty(&tpl.settings_override).unwrap());
        }
    }

    Ok(ProfileDto {
        name: profile.name,
        auth_type: profile.auth_type.to_string(),
        description: profile.description,
        is_active: false,
        sessions: vec!["default".to_string()],
        color: profile.color,
    })
}

#[tauri::command]
pub fn delete_profile(name: String) -> Result<(), String> {
    let mgr = ProfileManager::new(platform::profiles_dir());
    mgr.delete(&name).map_err(|e| e.to_string())
}
