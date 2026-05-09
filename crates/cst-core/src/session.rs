//! Session management — create, list, delete, and symlink setup.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::platform;

/// Session metadata stored in `sessions/{name}/session.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub created_at: DateTime<Utc>,
    /// Last time this session was activated.
    pub last_used: Option<DateTime<Utc>>,
    #[serde(default)]
    pub archived: bool,
}

impl Session {
    pub fn load(session_dir: &Path) -> Result<Self> {
        let path = session_dir.join("session.toml");
        if !path.exists() {
            anyhow::bail!("no session.toml in {}", session_dir.display());
        }
        let contents = std::fs::read_to_string(&path)?;
        toml::from_str(&contents).context("parsing session.toml")
    }

    pub fn save(&self, session_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(session_dir)?;
        let path = session_dir.join("session.toml");
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }
}

/// Manages sessions within a single profile's directory.
pub struct SessionManager {
    sessions_dir: PathBuf,
}

impl SessionManager {
    pub fn new(profile_dir: impl Into<PathBuf>) -> Self {
        let profile_dir: PathBuf = profile_dir.into();
        Self {
            sessions_dir: profile_dir.join("sessions"),
        }
    }

    fn session_dir(&self, name: &str) -> PathBuf {
        self.sessions_dir.join(name)
    }

    /// Create a new session, including `.claude/` directory with shared symlinks.
    pub fn create(&self, name: &str, global_claude_dir: &Path) -> Result<Session> {
        validate_session_name(name)?;
        let dir = self.session_dir(name);
        if dir.exists() {
            bail!("session '{name}' already exists");
        }
        let session = Session {
            name: name.to_string(),
            description: String::new(),
            created_at: Utc::now(),
            last_used: None,
            archived: false,
        };
        session.save(&dir)?;
        // Create the .claude config directory and populate with symlinks
        setup_claude_dir(&dir.join(".claude"), global_claude_dir)?;
        Ok(session)
    }

    /// List all (non-archived) sessions, sorted by last_used desc, then name.
    pub fn list(&self) -> Result<Vec<Session>> {
        std::fs::create_dir_all(&self.sessions_dir)?;
        let mut sessions = Vec::new();
        for entry in std::fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                match Session::load(&entry.path()) {
                    Ok(s) if !s.archived => sessions.push(s),
                    Ok(_) => {} // archived, skip
                    Err(e) => tracing::debug!(
                        "skipping directory without session.toml {:?}: {e}",
                        entry.path()
                    ),
                }
            }
        }
        sessions.sort_by(|a, b| {
            b.last_used
                .cmp(&a.last_used)
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(sessions)
    }

    pub fn load(&self, name: &str) -> Result<Session> {
        let dir = self.session_dir(name);
        if !dir.exists() {
            bail!("session '{name}' not found");
        }
        Session::load(&dir)
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        if name == "default" {
            bail!("cannot delete the 'default' session");
        }
        let dir = self.session_dir(name);
        if !dir.exists() {
            bail!("session '{name}' not found");
        }
        std::fs::remove_dir_all(dir)?;
        Ok(())
    }

    pub fn archive(&self, name: &str) -> Result<()> {
        let dir = self.session_dir(name);
        let mut session = Session::load(&dir)?;
        session.archived = true;
        session.save(&dir)?;
        Ok(())
    }

    pub fn tag(&self, name: &str, description: &str) -> Result<()> {
        let dir = self.session_dir(name);
        let mut session = Session::load(&dir)?;
        session.description = description.to_string();
        session.save(&dir)?;
        Ok(())
    }

    pub fn mark_used(&self, name: &str) -> Result<()> {
        let dir = self.session_dir(name);
        let mut session = Session::load(&dir)?;
        session.last_used = Some(Utc::now());
        session.save(&dir)?;
        Ok(())
    }

    pub fn exists(&self, name: &str) -> bool {
        self.session_dir(name).exists()
    }

    /// Rebuild symlinks for an existing session (run after `cst sync`).
    pub fn sync_symlinks(&self, name: &str, global_claude_dir: &Path) -> Result<()> {
        let dir = self.session_dir(name);
        setup_claude_dir(&dir.join(".claude"), global_claude_dir)?;
        Ok(())
    }
}

/// Populate `session/.claude/` with the correct layout:
/// - settings.json: created as empty placeholder (filled on activate)
/// - agents/, rules/, skills/, commands/, hooks.json, statusline.sh, CLAUDE.md: symlinks to global
/// - projects/, history.jsonl: local (not symlinked)
pub(crate) fn setup_claude_dir(claude_dir: &Path, global_claude_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(claude_dir)?;
    // Create local-only dirs
    std::fs::create_dir_all(claude_dir.join("projects"))?;

    // Shared symlinks — directories
    for dir_name in &["agents", "rules", "skills", "commands"] {
        let src = global_claude_dir.join(dir_name);
        let dst = claude_dir.join(dir_name);
        if src.exists() {
            platform::create_link(&src, &dst).with_context(|| format!("symlinking {dir_name}"))?;
        }
    }
    // Shared symlinks — files
    for file_name in &["hooks.json", "statusline.sh", "CLAUDE.md"] {
        let src = global_claude_dir.join(file_name);
        let dst = claude_dir.join(file_name);
        if src.exists() {
            platform::create_link(&src, &dst).with_context(|| format!("symlinking {file_name}"))?;
        }
    }
    Ok(())
}

pub const MAX_SESSION_NAME_LEN: usize = 64;

/// Validate a session name is a safe slug with bounded length.
///
/// Called at both create time and at CLI dispatch boundaries
/// where user-supplied session names are used to construct file paths.
pub fn validate_session_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("session name cannot be empty");
    }
    if name.len() > MAX_SESSION_NAME_LEN {
        bail!(
            "session name must be at most {MAX_SESSION_NAME_LEN} characters (got {})",
            name.len()
        );
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("session name must contain only letters, digits, hyphens, and underscores");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (SessionManager, TempDir, TempDir) {
        let profile_dir = TempDir::new().unwrap();
        let global_dir = TempDir::new().unwrap();
        let mgr = SessionManager::new(profile_dir.path());
        (mgr, profile_dir, global_dir)
    }

    #[test]
    fn test_create_session_sets_name() {
        let (mgr, _pd, gd) = setup();
        let s = mgr.create("backend", gd.path()).unwrap();
        assert_eq!(s.name, "backend");
        assert!(!s.archived);
    }

    #[test]
    fn test_create_duplicate_fails() {
        let (mgr, _pd, gd) = setup();
        mgr.create("backend", gd.path()).unwrap();
        assert!(mgr.create("backend", gd.path()).is_err());
    }

    #[test]
    fn test_create_builds_claude_dir() {
        let (mgr, _pd, gd) = setup();
        // Create a fake agents dir in global
        std::fs::create_dir_all(gd.path().join("agents")).unwrap();
        mgr.create("test", gd.path()).unwrap();
        // projects/ should exist locally
        assert!(mgr
            .session_dir("test")
            .join(".claude")
            .join("projects")
            .exists());
    }

    #[test]
    fn test_delete_session() {
        let (mgr, _pd, gd) = setup();
        mgr.create("temp", gd.path()).unwrap();
        mgr.delete("temp").unwrap();
        assert!(!mgr.exists("temp"));
    }

    #[test]
    fn test_cannot_delete_default_session() {
        let (mgr, _pd, gd) = setup();
        mgr.create("default", gd.path()).unwrap();
        assert!(mgr.delete("default").is_err());
    }

    #[test]
    fn test_archive_hides_from_list() {
        let (mgr, _pd, gd) = setup();
        mgr.create("old", gd.path()).unwrap();
        mgr.archive("old").unwrap();
        let list = mgr.list().unwrap();
        assert!(list.iter().all(|s| s.name != "old"));
    }

    #[test]
    fn test_tag_session() {
        let (mgr, _pd, gd) = setup();
        mgr.create("api", gd.path()).unwrap();
        mgr.tag("api", "API backend work").unwrap();
        let s = mgr.load("api").unwrap();
        assert_eq!(s.description, "API backend work");
    }

    #[test]
    fn test_list_is_sorted_by_last_used_then_name() {
        let (mgr, _pd, gd) = setup();
        mgr.create("aaa", gd.path()).unwrap();
        mgr.create("bbb", gd.path()).unwrap();
        mgr.mark_used("bbb").unwrap();
        let list = mgr.list().unwrap();
        // bbb has last_used set, so should come first
        assert_eq!(list[0].name, "bbb");
    }
}
