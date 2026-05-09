use anyhow::Result;
use cst_core::auth::{activate_profile_auth, activate_profile_auth_with};
use cst_core::env_overlay::EnvOverlay;
use cst_core::profile::Profile;
use cst_core::shell::{env_exports, parse_profile_session, shell_init_code, ShellKind};
use cst_core::{merge, platform, validate_profile_name, validate_session_name};
use std::collections::HashMap;

pub fn shell_init(shell_arg: Option<String>) -> Result<()> {
    let shell = match shell_arg.as_deref() {
        Some("zsh") => ShellKind::Zsh,
        Some("bash") => ShellKind::Bash,
        Some("fish") => ShellKind::Fish,
        Some("powershell") | Some("ps") => ShellKind::PowerShell,
        _ => ShellKind::detect(),
    };
    println!("{}", shell_init_code(&shell));
    Ok(())
}

pub fn env_cmd(profile_session: &str) -> Result<()> {
    let (profile, session) = parse_profile_session(profile_session);
    // Validate before using profile/session as path components or in shell exports.
    validate_profile_name(&profile)?;
    validate_session_name(&session)?;
    let shell = ShellKind::detect();

    let claude_config_dir = platform::claude_config_dir(&profile, &session);
    let profile_dir = platform::profile_dir(&profile);
    let session_dir = platform::session_dir(&profile, &session);

    // Build env vars to export
    let mut vars: HashMap<String, String> = HashMap::new();
    vars.insert(
        "CLAUDE_CONFIG_DIR".to_string(),
        claude_config_dir.to_string_lossy().to_string(),
    );
    vars.insert(
        "CST_CURRENT".to_string(),
        format!("{}:{}", profile, session),
    );

    // Load profile once, then fire lifecycle hooks and inject auth-specific vars.
    // Parsing profile.toml once avoids TOCTOU and three redundant disk reads.
    // activate_profile_auth_with handles OAuth symlink swap, API key injection, etc.
    // (best-effort — if profile doesn't exist yet, just export CLAUDE_CONFIG_DIR)
    if profile_dir.exists() {
        let profile_toml = profile_dir.join("profile.toml");
        if let Ok(contents) = std::fs::read_to_string(&profile_toml) {
            if let Ok(p) = toml::from_str::<Profile>(&contents) {
                let _ = p.hooks.run_pre_switch_in();

                match activate_profile_auth_with(&profile, Some(&p)) {
                    Ok(auth_vars) => vars.extend(auth_vars),
                    Err(e) => tracing::warn!("auth activation failed for {profile}: {e}"),
                }

                let _ = p.hooks.run_post_switch_in();
            } else {
                // Profile file exists but couldn't be parsed — fall back to name-only auth
                match activate_profile_auth(&profile) {
                    Ok(auth_vars) => vars.extend(auth_vars),
                    Err(e) => tracing::warn!("auth activation failed for {profile}: {e}"),
                }
            }
        }
    }

    // Load per-session env.toml overlay
    if session_dir.exists() {
        let overlay = EnvOverlay::load(&session_dir)?;
        vars.extend(overlay.env);
    }

    // Run settings merge if session dir exists
    if session_dir.exists() {
        let global_settings = platform::global_claude_dir().join("settings.json");
        let profile_override = profile_dir.join("settings-override.json");
        let session_override = session_dir.join("settings-override.json");
        let output = platform::claude_config_dir(&profile, &session).join("settings.json");
        if let Err(e) = merge::merge_and_write(
            &global_settings,
            &profile_override,
            &session_override,
            &output,
        ) {
            tracing::warn!("settings merge failed for {profile}:{session} — Claude Code may use stale settings: {e}");
        }
    }

    // Update global config
    let mut cfg = cst_core::GlobalConfig::load().unwrap_or_default();
    cfg.current_profile = profile.clone();
    cfg.current_session = session.clone();
    if let Err(e) = cfg.save() {
        tracing::warn!("failed to save active profile/session to config: {e}");
    }

    // Output exports
    print!("{}", env_exports(&vars, &shell));
    Ok(())
}
