//! Authentication — one module per auth type.
//!
//! Each module is responsible for:
//! 1. Storing credentials securely
//! 2. Injecting the correct environment variables on profile activate
//! 3. Validating credentials (health check)

pub mod apikey;
pub mod bedrock;
pub mod oauth;
pub mod secrets;
pub mod vertex;

pub use secrets::SecretSource;

use anyhow::Result;
use std::collections::HashMap;

/// Environment variables to export when a profile is activated.
pub type EnvMap = HashMap<String, String>;

/// Common interface for all auth providers.
pub trait AuthProvider: Send + Sync {
    /// Produce the environment variables needed to authenticate.
    fn env_vars(&self) -> Result<EnvMap>;

    /// Validate that credentials are present and usable.
    fn validate(&self) -> Result<()>;
}

/// Build auth-specific env vars for activating a profile and handle side effects
/// (OAuth symlink swap, API key injection).
///
/// This is the single canonical auth-activation path used by both `cst _env`
/// and `cst _broadcast-switch`. Keeping it here prevents the two call sites
/// from diverging — previously the broadcast path skipped the OAuth symlink
/// swap, causing silent auth failures on OAuth profiles.
///
/// If `profile` is `Some`, it is used directly (avoids a redundant disk read
/// when the caller has already loaded the profile). If `None`, the profile
/// is read from disk.
///
/// Returns a map of env vars to inject. Side effects (symlink creation) are
/// applied before returning.
pub fn activate_profile_auth(profile_name: &str) -> Result<EnvMap> {
    activate_profile_auth_with(profile_name, None)
}

/// Like `activate_profile_auth` but accepts a pre-loaded `Profile` to avoid
/// a redundant read when the caller has already parsed `profile.toml`.
pub fn activate_profile_auth_with(
    profile_name: &str,
    profile: Option<&crate::profile::Profile>,
) -> Result<EnvMap> {
    use crate::platform;
    use crate::profile::AuthType;

    let profile_dir = platform::profile_dir(profile_name);
    let mut vars = EnvMap::new();

    let profile_toml = profile_dir.join("profile.toml");
    if !profile_toml.exists() && profile.is_none() {
        return Ok(vars);
    }

    let loaded;
    let p: &crate::profile::Profile = match profile {
        Some(p) => p,
        None => {
            let contents = std::fs::read_to_string(&profile_toml)?;
            loaded = toml::from_str(&contents)?;
            &loaded
        }
    };

    match p.auth_type {
        AuthType::Api => {
            let keys_path = profile_dir.join("auth").join("api_keys.toml");
            if keys_path.exists() {
                let contents = std::fs::read_to_string(&keys_path)?;
                let pool: apikey::ApiKeyPool = toml::from_str(&contents)?;
                if let Some(&slot) = pool.sorted_slots().first() {
                    if let Ok(evars) = pool.env_vars_for_slot(slot) {
                        vars.extend(evars);
                    }
                }
            }
            // Remove any stale OAuth symlink
            let _ = oauth::deactivate();
        }
        AuthType::OAuth => {
            if let Err(e) = oauth::activate(&profile_dir.join("auth")) {
                tracing::warn!(
                    "OAuth symlink activation failed for {profile_name} — \
                     ~/.claude.json may be stale: {e}"
                );
            }
            // Ensure API key env var does not override OAuth
            vars.insert("ANTHROPIC_API_KEY".to_string(), String::new());
        }
        AuthType::Bedrock => {
            let bedrock_path = profile_dir.join("auth").join("aws.toml");
            if bedrock_path.exists() {
                let contents = std::fs::read_to_string(&bedrock_path)?;
                let cfg: bedrock::BedrockConfig = toml::from_str(&contents)?;
                if let Ok(evars) = cfg.env_vars() {
                    vars.extend(evars);
                }
            }
            let _ = oauth::deactivate();
        }
        AuthType::Vertex => {
            let vertex_path = profile_dir.join("auth").join("vertex.toml");
            if vertex_path.exists() {
                let contents = std::fs::read_to_string(&vertex_path)?;
                let cfg: vertex::VertexConfig = toml::from_str(&contents)?;
                if let Ok(evars) = cfg.env_vars() {
                    vars.extend(evars);
                }
            }
            let _ = oauth::deactivate();
        }
    }

    Ok(vars)
}
