use anyhow::Result;
use cst_core::shell::{env_exports, parse_profile_session, shell_init_code, ShellKind};
use cst_core::{platform, merge};
use cst_core::env_overlay::EnvOverlay;
use cst_core::profile::Profile;
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
    let shell = ShellKind::detect();

    let claude_config_dir = platform::claude_config_dir(&profile, &session);
    let profile_dir = platform::profile_dir(&profile);
    let session_dir = platform::session_dir(&profile, &session);

    // Build env vars to export
    let mut vars: HashMap<String, String> = HashMap::new();
    vars.insert("CLAUDE_CONFIG_DIR".to_string(), claude_config_dir.to_string_lossy().to_string());
    vars.insert("CST_CURRENT".to_string(), format!("{}:{}", profile, session));

    // Load profile to determine auth type and inject auth-specific vars
    // (best-effort — if profile doesn't exist yet, just export CLAUDE_CONFIG_DIR)
    if profile_dir.exists() {
        let profile_toml = platform::profile_dir(&profile).join("profile.toml");
        if profile_toml.exists() {
            let contents = std::fs::read_to_string(&profile_toml)?;
            let p: Profile = toml::from_str(&contents)?;

            // Fire pre-switch-in hook (non-fatal)
            let _ = p.hooks.run_pre_switch_in();

            match p.auth_type {
                cst_core::profile::AuthType::Api => {
                    // Load key from keychain (slot 1 by default)
                    let keys_path = profile_dir.join("auth").join("api_keys.toml");
                    if keys_path.exists() {
                        let contents = std::fs::read_to_string(&keys_path)?;
                        let pool: cst_core::auth::apikey::ApiKeyPool = toml::from_str(&contents)?;
                        if let Some(&slot) = pool.sorted_slots().first() {
                            if let Ok(evars) = pool.env_vars_for_slot(slot) {
                                vars.extend(evars);
                            }
                        }
                    }
                    // Ensure OAuth symlink is removed
                    let _ = cst_core::auth::oauth::deactivate();
                }
                cst_core::profile::AuthType::OAuth => {
                    let _ = cst_core::auth::oauth::activate(&profile_dir.join("auth"));
                    // Clear API key if set
                    vars.insert("ANTHROPIC_API_KEY".to_string(), String::new());
                }
                cst_core::profile::AuthType::Bedrock => {
                    let bedrock_path = profile_dir.join("auth").join("aws.toml");
                    if bedrock_path.exists() {
                        let contents = std::fs::read_to_string(&bedrock_path)?;
                        let cfg: cst_core::auth::bedrock::BedrockConfig = toml::from_str(&contents)?;
                        if let Ok(evars) = cfg.env_vars() {
                            vars.extend(evars);
                        }
                    }
                    let _ = cst_core::auth::oauth::deactivate();
                }
                cst_core::profile::AuthType::Vertex => {
                    let vertex_path = profile_dir.join("auth").join("vertex.toml");
                    if vertex_path.exists() {
                        let contents = std::fs::read_to_string(&vertex_path)?;
                        let cfg: cst_core::auth::vertex::VertexConfig = toml::from_str(&contents)?;
                        if let Ok(evars) = cfg.env_vars() {
                            vars.extend(evars);
                        }
                    }
                    let _ = cst_core::auth::oauth::deactivate();
                }
            }

            // Fire post-switch-in hook (non-fatal)
            let _ = p.hooks.run_post_switch_in();
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
        let _ = merge::merge_and_write(&global_settings, &profile_override, &session_override, &output);
    }

    // Update global config
    let mut cfg = cst_core::GlobalConfig::load().unwrap_or_default();
    cfg.current_profile = profile;
    cfg.current_session = session;
    let _ = cfg.save();

    // Output exports
    print!("{}", env_exports(&vars, &shell));
    Ok(())
}
