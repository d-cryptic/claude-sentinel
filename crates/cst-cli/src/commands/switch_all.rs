//! `cst switch-all <from> <to>` — broadcast a profile switch to every open shell.
//!
//! Also implements `cst _broadcast-switch` — the hidden command called by the
//! precmd hook in each shell to check whether it should apply a pending broadcast.

use anyhow::Result;
use cst_core::{
    broadcast::{check_broadcast, BroadcastSwitch},
    platform,
    profile::ProfileManager,
    shell::{env_exports, ShellKind},
    validate_profile_name, validate_session_name,
};
use std::collections::HashMap;

/// Write a broadcast switch file and immediately update the global config.
///
/// Every shell running `from` will pick this up on its next prompt via `_cst_check_switch`.
pub fn run(from: &str, to: &str) -> Result<()> {
    // Validate both profiles exist
    let pm = ProfileManager::new(platform::profiles_dir());
    pm.load(from)
        .map_err(|_| anyhow::anyhow!("Profile '{from}' not found"))?;
    pm.load(to)
        .map_err(|_| anyhow::anyhow!("Profile '{to}' not found"))?;

    let broadcast = BroadcastSwitch::write(from, to)?;

    println!("✓ Broadcast switch queued: {from} → {to}");
    println!("  ID        : {}", broadcast.id);
    println!(
        "  Expires   : {} (5 min)",
        broadcast.expires_at.format("%H:%M:%S")
    );
    println!();
    println!("  All open shells running profile '{from}' will switch to '{to}'");
    println!("  at their next shell prompt (precmd hook).");
    println!();
    println!("  To switch the current shell immediately:");
    println!("    cst use {to}");

    Ok(())
}

/// Called by the precmd hook: `cst _broadcast-switch $CST_CURRENT $CST_BROADCAST_ID`
///
/// Prints env exports if the current shell should switch, or nothing if not.
/// The output also sets `CST_BROADCAST_ID` so the shell doesn't re-apply.
pub fn broadcast_switch_check(current: &str, already_applied_id: &str) -> Result<()> {
    let Some((to_profile, session)) = check_broadcast(current, already_applied_id) else {
        return Ok(());
    };

    // Validate names before using them as path components or in shell exports.
    // The broadcast file comes from disk and could be tampered with.
    validate_profile_name(&to_profile)?;
    validate_session_name(&session)?;

    // Load the broadcast to get its ID for setting CST_BROADCAST_ID
    let Some(broadcast) = BroadcastSwitch::load_active() else {
        return Ok(());
    };

    let shell = ShellKind::detect();

    // Build the env vars for the target profile:session
    let claude_config_dir = platform::claude_config_dir(&to_profile, &session);
    let mut vars: HashMap<String, String> = HashMap::new();
    vars.insert(
        "CLAUDE_CONFIG_DIR".to_string(),
        claude_config_dir.to_string_lossy().to_string(),
    );
    vars.insert(
        "CST_CURRENT".to_string(),
        format!("{}:{}", to_profile, session),
    );
    // Mark this broadcast as applied in this shell
    vars.insert("CST_BROADCAST_ID".to_string(), broadcast.id.clone());

    // Load the profile to inject auth-specific vars (best-effort)
    let profile_toml = platform::profile_dir(&to_profile).join("profile.toml");
    if profile_toml.exists() {
        if let Ok(contents) = std::fs::read_to_string(&profile_toml) {
            if let Ok(p) = toml::from_str::<cst_core::profile::Profile>(&contents) {
                inject_auth_vars(&p, &to_profile, &mut vars);
            }
        }
    }

    println!("{}", env_exports(&vars, &shell));
    Ok(())
}

/// Inject auth-specific env vars into the map (mirrors shell.rs env_cmd logic).
fn inject_auth_vars(
    p: &cst_core::profile::Profile,
    profile_name: &str,
    vars: &mut HashMap<String, String>,
) {
    let profile_dir = platform::profile_dir(profile_name);
    match p.auth_type {
        cst_core::profile::AuthType::Api => {
            let keys_path = profile_dir.join("auth").join("api_keys.toml");
            if let Ok(contents) = std::fs::read_to_string(&keys_path) {
                if let Ok(pool) = toml::from_str::<cst_core::auth::apikey::ApiKeyPool>(&contents) {
                    if let Some(&slot) = pool.sorted_slots().first() {
                        if let Ok(evars) = pool.env_vars_for_slot(slot) {
                            vars.extend(evars);
                        }
                    }
                }
            }
        }
        cst_core::profile::AuthType::Bedrock => {
            let aws_path = profile_dir.join("auth").join("aws.toml");
            if let Ok(contents) = std::fs::read_to_string(&aws_path) {
                if let Ok(creds) =
                    toml::from_str::<cst_core::auth::bedrock::BedrockConfig>(&contents)
                {
                    if let Ok(evars) = creds.env_vars() {
                        vars.extend(evars);
                    }
                }
            }
        }
        cst_core::profile::AuthType::Vertex => {
            let vertex_path = profile_dir.join("auth").join("vertex.toml");
            if let Ok(contents) = std::fs::read_to_string(&vertex_path) {
                if let Ok(cfg) = toml::from_str::<cst_core::auth::vertex::VertexConfig>(&contents) {
                    if let Ok(evars) = cfg.env_vars() {
                        vars.extend(evars);
                    }
                }
            }
        }
        cst_core::profile::AuthType::OAuth => {
            // OAuth: symlink swap is handled separately; nothing to inject here
        }
    }
}
