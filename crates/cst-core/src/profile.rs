//! Profile management — create, list, load, delete, clone, import.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::hooks::ProfileHooks;
use crate::platform;

/// Authentication type for a profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthType {
    /// Claude Pro/Max OAuth subscription.
    OAuth,
    /// Direct API key (`ANTHROPIC_API_KEY`).
    Api,
    /// AWS Bedrock.
    Bedrock,
    /// Google Vertex AI.
    Vertex,
}

impl std::fmt::Display for AuthType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OAuth => write!(f, "oauth"),
            Self::Api => write!(f, "api"),
            Self::Bedrock => write!(f, "bedrock"),
            Self::Vertex => write!(f, "vertex"),
        }
    }
}

impl std::str::FromStr for AuthType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "oauth" => Ok(Self::OAuth),
            "api" => Ok(Self::Api),
            "bedrock" => Ok(Self::Bedrock),
            "vertex" => Ok(Self::Vertex),
            other => bail!("unknown auth type: {other}. Valid: oauth, api, bedrock, vertex"),
        }
    }
}

/// Metadata stored in `~/.claude-sentinel/profiles/{name}/profile.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Slug name (directory name, lowercase, no spaces).
    pub name: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: String,
    /// Authentication type.
    pub auth_type: AuthType,
    /// Optional display color label (used in TUI / Mac app).
    #[serde(default)]
    pub color: String,
    /// When this profile was created.
    pub created_at: DateTime<Utc>,
    /// Template this profile was created from, if any.
    #[serde(default)]
    pub template: Option<String>,
    /// Pre/post switch lifecycle hooks.
    #[serde(default)]
    pub hooks: ProfileHooks,
}

impl Profile {
    /// Load profile metadata from its directory.
    pub fn load(profile_dir: &Path) -> Result<Self> {
        let path = profile_dir.join("profile.toml");
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("reading profile at {}", path.display()))?;
        toml::from_str(&contents).context("parsing profile.toml")
    }

    /// Save profile metadata to its directory.
    pub fn save(&self, profile_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(profile_dir)?;
        let path = profile_dir.join("profile.toml");
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }
}

/// Manages all profiles in the data directory.
pub struct ProfileManager {
    profiles_dir: PathBuf,
}

impl ProfileManager {
    /// Create a manager rooted at the given profiles directory.
    pub fn new(profiles_dir: impl Into<PathBuf>) -> Self {
        Self {
            profiles_dir: profiles_dir.into(),
        }
    }
}

impl Default for ProfileManager {
    /// Create a manager using the default platform data dir.
    fn default() -> Self {
        Self::new(platform::profiles_dir())
    }
}

impl ProfileManager {

    fn profile_dir(&self, name: &str) -> PathBuf {
        self.profiles_dir.join(name)
    }

    /// Create a new profile. Fails if it already exists.
    pub fn create(&self, name: &str, auth_type: AuthType) -> Result<Profile> {
        validate_profile_name(name)?;
        let dir = self.profile_dir(name);
        if dir.exists() {
            bail!("profile '{name}' already exists");
        }
        let profile = Profile {
            name: name.to_string(),
            description: String::new(),
            auth_type,
            color: String::new(),
            created_at: Utc::now(),
            template: None,
            hooks: ProfileHooks::default(),
        };
        profile.save(&dir)?;
        // Create auth subdirectory
        std::fs::create_dir_all(dir.join("auth"))?;
        Ok(profile)
    }

    /// Load an existing profile by name.
    pub fn load(&self, name: &str) -> Result<Profile> {
        let dir = self.profile_dir(name);
        if !dir.exists() {
            bail!("profile '{name}' not found");
        }
        Profile::load(&dir)
    }

    /// List all profiles sorted alphabetically.
    pub fn list(&self) -> Result<Vec<Profile>> {
        std::fs::create_dir_all(&self.profiles_dir)?;
        let mut profiles = Vec::new();
        for entry in std::fs::read_dir(&self.profiles_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let profile_toml = entry.path().join("profile.toml");
                if profile_toml.exists() {
                    match Profile::load(&entry.path()) {
                        Ok(p) => profiles.push(p),
                        Err(e) => {
                            tracing::warn!("skipping malformed profile {:?}: {e}", entry.path())
                        }
                    }
                }
            }
        }
        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(profiles)
    }

    /// Delete a profile and all its sessions.
    pub fn delete(&self, name: &str) -> Result<()> {
        let dir = self.profile_dir(name);
        if !dir.exists() {
            bail!("profile '{name}' not found");
        }
        std::fs::remove_dir_all(dir)?;
        Ok(())
    }

    /// Rename a profile.
    pub fn rename(&self, old: &str, new: &str) -> Result<()> {
        validate_profile_name(new)?;
        let old_dir = self.profile_dir(old);
        let new_dir = self.profile_dir(new);
        if !old_dir.exists() {
            bail!("profile '{old}' not found");
        }
        if new_dir.exists() {
            bail!("profile '{new}' already exists");
        }
        std::fs::rename(&old_dir, &new_dir)?;
        // Update the name field inside profile.toml
        let mut profile = Profile::load(&new_dir)?;
        profile.name = new.to_string();
        profile.save(&new_dir)?;
        Ok(())
    }

    /// Clone a profile (copies directory, assigns new name).
    pub fn clone_profile(&self, src: &str, dst: &str) -> Result<Profile> {
        validate_profile_name(dst)?;
        let src_dir = self.profile_dir(src);
        let dst_dir = self.profile_dir(dst);
        if !src_dir.exists() {
            bail!("source profile '{src}' not found");
        }
        if dst_dir.exists() {
            bail!("destination profile '{dst}' already exists");
        }
        copy_dir_all(&src_dir, &dst_dir)?;
        // Update name in the copy
        let mut profile = Profile::load(&dst_dir)?;
        profile.name = dst.to_string();
        profile.created_at = Utc::now();
        profile.save(&dst_dir)?;
        Ok(profile)
    }

    /// Check whether a profile exists.
    pub fn exists(&self, name: &str) -> bool {
        self.profile_dir(name).join("profile.toml").exists()
    }
}

pub(crate) const MAX_PROFILE_NAME_LEN: usize = 64;

/// Validate that a profile name is a safe slug.
///
/// Called at both create/rename time and at CLI dispatch boundaries
/// where user-supplied profile names are used to construct file paths.
pub fn validate_profile_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("profile name cannot be empty");
    }
    if name.len() > MAX_PROFILE_NAME_LEN {
        bail!(
            "profile name must be at most {MAX_PROFILE_NAME_LEN} characters (got {})",
            name.len()
        );
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("profile name must contain only letters, digits, hyphens, and underscores: got '{name}'");
    }
    Ok(())
}

/// Recursively copy a directory.
/// Symlinks are recreated as symlinks (not followed), so machine-specific
/// symlinks (e.g. `.claude/agents → ~/.claude/agents`) remain correct in
/// the cloned profile.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_symlink() {
            let target = std::fs::read_link(entry.path())?;
            crate::platform::create_link(&target, &dst_path)?;
        } else if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn manager() -> (ProfileManager, TempDir) {
        let dir = TempDir::new().unwrap();
        (ProfileManager::new(dir.path()), dir)
    }

    #[test]
    fn test_create_profile_sets_name_and_auth() {
        let (mgr, _dir) = manager();
        let p = mgr.create("work", AuthType::OAuth).unwrap();
        assert_eq!(p.name, "work");
        assert_eq!(p.auth_type, AuthType::OAuth);
    }

    #[test]
    fn test_create_duplicate_fails() {
        let (mgr, _dir) = manager();
        mgr.create("work", AuthType::OAuth).unwrap();
        assert!(mgr.create("work", AuthType::Api).is_err());
    }

    #[test]
    fn test_create_invalid_name_fails() {
        let (mgr, _dir) = manager();
        assert!(mgr.create("has space", AuthType::OAuth).is_err());
        assert!(mgr.create("", AuthType::OAuth).is_err());
        assert!(mgr.create("has/slash", AuthType::OAuth).is_err());
        // Name exceeding max length must be rejected
        let long_name = "a".repeat(MAX_PROFILE_NAME_LEN + 1);
        let err = mgr.create(&long_name, AuthType::OAuth).unwrap_err();
        assert!(err.to_string().contains("at most"), "expected max-length error: {err}");
    }

    #[test]
    fn test_create_name_at_max_length_succeeds() {
        let (mgr, _dir) = manager();
        let name = "a".repeat(MAX_PROFILE_NAME_LEN);
        assert!(mgr.create(&name, AuthType::OAuth).is_ok());
    }

    #[test]
    fn test_list_returns_all_profiles_sorted() {
        let (mgr, _dir) = manager();
        mgr.create("zebra", AuthType::OAuth).unwrap();
        mgr.create("alpha", AuthType::Api).unwrap();
        let list = mgr.list().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "alpha");
        assert_eq!(list[1].name, "zebra");
    }

    #[test]
    fn test_delete_profile() {
        let (mgr, _dir) = manager();
        mgr.create("work", AuthType::OAuth).unwrap();
        assert!(mgr.exists("work"));
        mgr.delete("work").unwrap();
        assert!(!mgr.exists("work"));
    }

    #[test]
    fn test_delete_nonexistent_fails() {
        let (mgr, _dir) = manager();
        assert!(mgr.delete("nope").is_err());
    }

    #[test]
    fn test_rename_profile() {
        let (mgr, _dir) = manager();
        mgr.create("old", AuthType::OAuth).unwrap();
        mgr.rename("old", "new").unwrap();
        assert!(!mgr.exists("old"));
        assert!(mgr.exists("new"));
        let p = mgr.load("new").unwrap();
        assert_eq!(p.name, "new");
    }

    #[test]
    fn test_clone_profile() {
        let (mgr, _dir) = manager();
        mgr.create("src", AuthType::OAuth).unwrap();
        let cloned = mgr.clone_profile("src", "dst").unwrap();
        assert_eq!(cloned.name, "dst");
        assert!(mgr.exists("src"));
        assert!(mgr.exists("dst"));
    }

    #[test]
    fn test_auth_type_roundtrip_display_parse() {
        for at in [
            AuthType::OAuth,
            AuthType::Api,
            AuthType::Bedrock,
            AuthType::Vertex,
        ] {
            let s = at.to_string();
            let parsed: AuthType = s.parse().unwrap();
            assert_eq!(at, parsed);
        }
    }
}
