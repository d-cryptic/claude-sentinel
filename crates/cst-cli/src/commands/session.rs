use anyhow::Result;
use cst_core::{platform, session::SessionManager, validate_profile_name, validate_session_name, GlobalConfig};

pub async fn dispatch(action: crate::SessionCommands) -> Result<()> {
    match action {
        crate::SessionCommands::New { name, tag } => new(&name, tag.as_deref()),
        crate::SessionCommands::List { profile } => list(profile.as_deref()),
        crate::SessionCommands::Rm { name } => remove(&name),
        crate::SessionCommands::Tag { name, description } => tag(&name, &description),
        crate::SessionCommands::Archive { name } => archive(&name),
        crate::SessionCommands::Switch { name, to_profile } => switch(&name, &to_profile),
    }
}

fn current_profile() -> Result<String> {
    let cfg = GlobalConfig::load()?;
    if cfg.current_profile.is_empty() {
        anyhow::bail!("No active profile. Run: cst use <profile>");
    }
    Ok(cfg.current_profile)
}

pub fn new(name: &str, tag: Option<&str>) -> Result<()> {
    validate_session_name(name)?;
    let profile = current_profile()?;
    let mgr = SessionManager::new(platform::profile_dir(&profile));
    let session = mgr.create(name, &platform::global_claude_dir())?;
    if let Some(desc) = tag {
        mgr.tag(name, desc)?;
    }
    println!("✓ Created session '{name}' in profile '{profile}'");
    Ok(())
}

pub fn list(profile: Option<&str>) -> Result<()> {
    let profile = match profile {
        Some(p) => p.to_string(),
        None => current_profile()?,
    };
    let mgr = SessionManager::new(platform::profile_dir(&profile));
    let sessions = mgr.list()?;
    let current = GlobalConfig::load().unwrap_or_default();
    for s in &sessions {
        let active = current.current_profile == profile && current.current_session == s.name;
        let marker = if active { "✓" } else { " " };
        let tag = if s.description.is_empty() {
            String::new()
        } else {
            format!(" — {}", s.description)
        };
        println!("[{marker}] {}{tag}", s.name);
    }
    Ok(())
}

pub fn remove(name: &str) -> Result<()> {
    validate_session_name(name)?;
    let profile = current_profile()?;
    SessionManager::new(platform::profile_dir(&profile)).delete(name)?;
    println!("✓ Deleted session '{name}'");
    Ok(())
}

pub fn tag(name: &str, description: &str) -> Result<()> {
    validate_session_name(name)?;
    let profile = current_profile()?;
    SessionManager::new(platform::profile_dir(&profile)).tag(name, description)?;
    println!("✓ Tagged '{name}': {description}");
    Ok(())
}

pub fn archive(name: &str) -> Result<()> {
    validate_session_name(name)?;
    let profile = current_profile()?;
    SessionManager::new(platform::profile_dir(&profile)).archive(name)?;
    println!("✓ Archived '{name}'");
    Ok(())
}

/// Activate session `name` under a different profile.
///
/// If the session doesn't exist in `to_profile`, it is created there first.
/// Then writes a pending-switch file so the current shell picks up the change.
pub fn switch(name: &str, to_profile: &str) -> Result<()> {
    validate_session_name(name)?;
    validate_profile_name(to_profile)?;
    let from_profile = current_profile()?;

    // Ensure the target profile exists
    let target_profile_dir = platform::profile_dir(to_profile);
    if !target_profile_dir.exists() {
        anyhow::bail!(
            "Profile '{to_profile}' does not exist. Create it first with: cst new {to_profile}"
        );
    }

    // Ensure the session exists in the target profile — create it if missing
    let target_mgr = SessionManager::new(platform::profile_dir(to_profile));
    if target_mgr.load(name).is_err() {
        target_mgr.create(name, &platform::global_claude_dir())?;
        println!("  Created session '{name}' in profile '{to_profile}'");
    }

    // Write the pending-switch so the shell picks it up at next prompt
    cst_core::auto_switch::daemon::write_pending_switch(to_profile, name)?;

    // Update global config immediately
    let mut cfg = GlobalConfig::load().unwrap_or_default();
    cfg.current_profile = to_profile.to_string();
    cfg.current_session = name.to_string();
    cfg.save()?;

    println!("✓ Session '{name}' switched: {from_profile} → {to_profile}");
    println!("  Run `cst use {to_profile}:{name}` to apply in the current shell, or");
    println!("  it will be picked up automatically at the next prompt.");
    Ok(())
}
