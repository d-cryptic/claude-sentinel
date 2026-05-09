use anyhow::Result;
use cst_core::auth::oauth;
use cst_core::platform;
use cst_core::profile::{AuthType, ProfileManager};
use std::str::FromStr;

pub async fn new(name: &str, auth: &str, template: Option<&str>) -> Result<()> {
    let auth_type = AuthType::from_str(auth)?;
    let mgr = ProfileManager::default();
    mgr.create(name, auth_type.clone())?;
    println!("✓ Created profile '{name}' [{auth_type}]");

    // Apply template settings if specified
    if let Some(tmpl_name) = template {
        if let Some(tmpl) = cst_core::templates::find(tmpl_name) {
            let override_path = platform::profile_dir(name).join("settings-override.json");
            std::fs::write(
                &override_path,
                serde_json::to_string_pretty(&tmpl.settings_override)?,
            )?;
            println!("✓ Applied template '{tmpl_name}'");
        } else {
            eprintln!("⚠ Template '{tmpl_name}' not found. Run: cst templates");
        }
    }

    // Create default session with symlinks
    let session_mgr = cst_core::session::SessionManager::new(platform::profile_dir(name));
    let global = platform::global_claude_dir();
    session_mgr.create("default", &global)?;
    println!("✓ Created default session");

    if matches!(auth_type, cst_core::profile::AuthType::OAuth) {
        println!("\nNext: run `cst login {name}` to authenticate");
    }
    Ok(())
}

pub fn import(alias: Option<&str>) -> Result<()> {
    let name = alias.unwrap_or("imported");
    let mgr = ProfileManager::default();
    let _ = mgr.create(name, cst_core::profile::AuthType::OAuth)?;
    let auth_dir = platform::profile_dir(name).join("auth");
    oauth::import_current(&auth_dir)?;

    let session_mgr = cst_core::session::SessionManager::new(platform::profile_dir(name));
    session_mgr.create("default", &platform::global_claude_dir())?;

    println!("✓ Imported current ~/.claude.json as profile '{name}'");
    println!("  Run: cst use {name}");
    Ok(())
}

pub fn clone(src: &str, dst: &str) -> Result<()> {
    ProfileManager::default().clone_profile(src, dst)?;
    println!("✓ Cloned '{src}' → '{dst}'");
    Ok(())
}

pub fn remove(name: &str) -> Result<()> {
    ProfileManager::default().delete(name)?;
    println!("✓ Deleted profile '{name}'");
    Ok(())
}

pub fn rename(old: &str, new: &str) -> Result<()> {
    ProfileManager::default().rename(old, new)?;
    println!("✓ Renamed '{old}' → '{new}'");
    Ok(())
}

pub async fn login(profile: Option<&str>) -> Result<()> {
    let name = profile.unwrap_or("default");
    println!("Starting OAuth login for profile '{name}'...");
    // Activate the profile's CLAUDE_CONFIG_DIR, then run `claude /login`
    let config_dir = platform::claude_config_dir(name, "default");
    std::fs::create_dir_all(&config_dir)?;
    let status = tokio::process::Command::new("claude")
        .arg("/login")
        .env("CLAUDE_CONFIG_DIR", &config_dir)
        .status()
        .await?;
    if status.success() {
        // Copy the newly-written ~/.claude.json to the profile's auth dir
        let auth_dir = platform::profile_dir(name).join("auth");
        oauth::import_current(&auth_dir)?;
        println!("✓ Login successful for '{name}'");
    } else {
        eprintln!("✗ Login failed");
    }
    Ok(())
}

pub fn add_key(
    profile: &str,
    slot: u8,
    source_flag: Option<&str>,
    note: Option<&str>,
) -> Result<()> {
    use cst_core::auth::apikey::ApiKeyPool;
    use cst_core::auth::secrets::SecretSource;
    cst_core::profile::validate_profile_name(profile)?;

    let keys_path = platform::profile_dir(profile)
        .join("auth")
        .join("api_keys.toml");
    if let Some(parent) = keys_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut pool: ApiKeyPool = if keys_path.exists() {
        let c = std::fs::read_to_string(&keys_path)?;
        toml::from_str(&c)?
    } else {
        ApiKeyPool::default()
    };

    let note_str = note.unwrap_or("");

    // If --source was given, parse it non-interactively
    if let Some(src) = source_flag {
        let secret_source = parse_source_flag(src)?;
        // For external sources, validate the tool is available before saving
        secret_source.check_tool_available()?;
        pool.add_external_key(slot, secret_source, note_str)?;
        std::fs::write(&keys_path, toml::to_string_pretty(&pool)?)?;
        println!("✓ Registered external key in slot {slot} for '{profile}'");
        return Ok(());
    }

    // Interactive provider menu
    println!("\nSelect secret provider for '{profile}' slot {slot}:");
    println!("  [1] Keychain  — macOS Keychain / libsecret / WinCred (default)");
    println!("  [2] 1Password — op://vault/item/field");
    println!("  [3] Doppler   — secret name from Doppler project");
    println!("  [4] Env var   — read from environment variable");
    print!("\nChoice [1-4, default 1]: ");
    use std::io::Write;
    std::io::stdout().flush()?;

    let choice = read_line()?.trim().to_string();
    let choice = if choice.is_empty() {
        "1".to_string()
    } else {
        choice
    };

    match choice.as_str() {
        "1" | "keychain" => {
            print!("Enter API key: ");
            std::io::stdout().flush()?;
            let key = read_password()?;
            if key.trim().is_empty() {
                anyhow::bail!("API key cannot be empty");
            }
            pool.add_key(profile, slot, key.trim(), note_str)?;
            std::fs::write(&keys_path, toml::to_string_pretty(&pool)?)?;
            println!("✓ Stored key in Keychain, slot {slot} for '{profile}'");
        }
        "2" | "1password" | "op" => {
            print!("1Password reference (e.g. op://Personal/Claude API Key/credential): ");
            std::io::stdout().flush()?;
            let reference = read_line()?.trim().to_string();
            if !reference.starts_with("op://") {
                anyhow::bail!("1Password reference must start with op://");
            }
            let src = SecretSource::OnePassword { reference };
            src.check_tool_available()?;
            pool.add_external_key(slot, src, note_str)?;
            std::fs::write(&keys_path, toml::to_string_pretty(&pool)?)?;
            println!("✓ Registered 1Password key in slot {slot} for '{profile}'");
        }
        "3" | "doppler" => {
            print!("Doppler secret name (e.g. ANTHROPIC_API_KEY): ");
            std::io::stdout().flush()?;
            let secret_name = read_line()?.trim().to_string();
            if secret_name.is_empty() {
                anyhow::bail!("secret name cannot be empty");
            }
            print!("Doppler project (optional, press Enter to use default): ");
            std::io::stdout().flush()?;
            let project_input = read_line()?.trim().to_string();
            let project = if project_input.is_empty() {
                None
            } else {
                Some(project_input)
            };

            print!("Doppler config (optional, press Enter to use default): ");
            std::io::stdout().flush()?;
            let config_input = read_line()?.trim().to_string();
            let config = if config_input.is_empty() {
                None
            } else {
                Some(config_input)
            };

            let src = SecretSource::Doppler {
                secret_name,
                project,
                config,
            };
            src.check_tool_available()?;
            pool.add_external_key(slot, src, note_str)?;
            std::fs::write(&keys_path, toml::to_string_pretty(&pool)?)?;
            println!("✓ Registered Doppler key in slot {slot} for '{profile}'");
        }
        "4" | "env" | "envvar" => {
            print!("Environment variable name (e.g. ANTHROPIC_API_KEY_BACKUP): ");
            std::io::stdout().flush()?;
            let var_name = read_line()?.trim().trim_start_matches('$').to_string();
            if var_name.is_empty() {
                anyhow::bail!("variable name cannot be empty");
            }
            let src = SecretSource::EnvVar { var_name };
            pool.add_external_key(slot, src, note_str)?;
            std::fs::write(&keys_path, toml::to_string_pretty(&pool)?)?;
            println!("✓ Registered env var key in slot {slot} for '{profile}'");
        }
        other => {
            anyhow::bail!("invalid choice '{}' — enter 1, 2, 3, or 4", other);
        }
    }

    Ok(())
}

/// Parse the `--source` flag into a `SecretSource`.
///
/// Accepted formats:
/// - `"keychain"` or `"keychain:account-name"` -> `SecretSource::Keychain`
/// - `"op://vault/item/field"` -> `SecretSource::OnePassword`
/// - `"doppler:SECRET_NAME"` -> `SecretSource::Doppler`
/// - `"$VAR"` or `"env:VAR"` -> `SecretSource::EnvVar`
fn parse_source_flag(src: &str) -> Result<cst_core::auth::secrets::SecretSource> {
    use cst_core::auth::secrets::SecretSource;

    if src.starts_with("op://") {
        return Ok(SecretSource::OnePassword {
            reference: src.to_string(),
        });
    }
    if src.starts_with("doppler:") {
        let name = src.trim_start_matches("doppler:").to_string();
        return Ok(SecretSource::Doppler {
            secret_name: name,
            project: None,
            config: None,
        });
    }
    if src.starts_with('$') {
        return Ok(SecretSource::EnvVar {
            var_name: src.trim_start_matches('$').to_string(),
        });
    }
    if src.starts_with("env:") {
        return Ok(SecretSource::EnvVar {
            var_name: src.trim_start_matches("env:").to_string(),
        });
    }
    if src == "keychain" || src.starts_with("keychain:") {
        let account = src.trim_start_matches("keychain:").to_string();
        return Ok(SecretSource::Keychain { account });
    }
    anyhow::bail!(
        "unrecognised --source format '{src}'\n\
         Valid formats:\n\
         keychain         (OS credential store — prompts for key)\n\
         op://vault/item/field  (1Password CLI)\n\
         doppler:SECRET   (Doppler CLI)\n\
         $ENV_VAR         (environment variable)\n\
         env:ENV_VAR      (environment variable)"
    )
}

fn read_line() -> Result<String> {
    use std::io::BufRead;
    let stdin = std::io::stdin();
    stdin
        .lock()
        .lines()
        .next()
        .ok_or_else(|| anyhow::anyhow!("unexpected end of stdin — this command requires interactive input"))?
        .map_err(Into::into)
}

fn read_password() -> Result<String> {
    rpassword::prompt_password("API key (input hidden): ")
        .map_err(|e| anyhow::anyhow!("failed to read password: {e}"))
}
