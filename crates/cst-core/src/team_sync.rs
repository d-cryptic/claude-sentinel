//! Team profile sharing — git-based config sync.
//!
//! Pushes/pulls profile configs (settings overrides, MCP overrides, env overlays,
//! auto-switch config) to/from a shared git remote.
//!
//! **What is synced** (safe to share):
//! - `profile.toml` — metadata only (auth_type, description, template)
//! - `settings-override.json` — profile-level settings overrides
//! - `mcp-override.json` — MCP add/disable list
//! - `env.toml` — extra env vars per session
//! - `auto-switch.toml` — fallback chain, schedule, round-robin config
//!
//! **What is NEVER synced** (stays local):
//! - `auth/` directory (credentials, OAuth tokens, API keys)
//! - `sessions/*/stats.json` (usage stats)
//! - `.claude/history.jsonl` (session history)
//! - Any `.enc` or keychain reference files
//!
//! ## Setup
//!
//! ```bash
//! cst team init git@github.com:myorg/claude-profiles.git
//! cst team push
//! # On another machine:
//! cst team pull
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::platform;

/// Team sync configuration stored at `~/.claude-sentinel/team-sync.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamSyncConfig {
    /// Git remote URL (SSH or HTTPS).
    pub remote_url: String,

    /// Branch to push/pull (default: "main").
    #[serde(default = "default_branch")]
    pub branch: String,

    /// Profiles to include in sync (empty = all profiles).
    #[serde(default)]
    pub include_profiles: Vec<String>,

    /// Profiles to never sync (always excluded).
    #[serde(default)]
    pub exclude_profiles: Vec<String>,

    /// Committer name for sync commits.
    #[serde(default)]
    pub committer_name: String,

    /// Committer email for sync commits.
    #[serde(default)]
    pub committer_email: String,
}

fn default_branch() -> String {
    "main".to_string()
}

impl TeamSyncConfig {
    /// Load from `~/.claude-sentinel/team-sync.toml`.
    pub fn load() -> Result<Self> {
        let path = config_path();
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("team sync not configured — run: cst team init <remote-url>"))?;
        Ok(toml::from_str(&content)?)
    }

    /// Save to `~/.claude-sentinel/team-sync.toml`.
    pub fn save(&self) -> Result<()> {
        let path = config_path();
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Whether a profile name should be included in sync.
    pub fn should_sync(&self, profile: &str) -> bool {
        if self.exclude_profiles.iter().any(|e| e == profile) {
            return false;
        }
        if self.include_profiles.is_empty() {
            return true;
        }
        self.include_profiles.iter().any(|i| i == profile)
    }
}

fn config_path() -> PathBuf {
    platform::data_dir().join("team-sync.toml")
}

// ─── Sync repo path ──────────────────────────────────────────────────────────

fn sync_repo_path() -> PathBuf {
    platform::data_dir().join("team-sync-repo")
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Initialise team sync: clone the remote into the sync-repo cache.
///
/// If the repo already exists, updates the remote URL.
pub fn init(remote_url: &str, branch: &str) -> Result<TeamSyncConfig> {
    let repo = sync_repo_path();

    if repo.exists() {
        // Update remote URL
        git(&repo, &["remote", "set-url", "origin", remote_url])?;
        println!("✓ Updated remote to {}", remote_url);
    } else {
        println!("Cloning {} …", remote_url);
        // Shallow clone; it's OK if the repo is empty
        let status = Command::new("git")
            .args(["clone", "--depth=1", "--branch", branch, remote_url, &repo.to_string_lossy()])
            .status();

        match status {
            Ok(s) if s.success() => {}
            _ => {
                // Remote may be empty — init a bare local repo and add the remote
                std::fs::create_dir_all(&repo)?;
                git(&repo, &["init", "-b", branch])?;
                git(&repo, &["remote", "add", "origin", remote_url])?;
                println!("note: empty remote — will push on first `cst team push`");
            }
        }
    }

    let cfg = TeamSyncConfig {
        remote_url: remote_url.to_string(),
        branch: branch.to_string(),
        include_profiles: Vec::new(),
        exclude_profiles: Vec::new(),
        committer_name: String::new(),
        committer_email: String::new(),
    };
    cfg.save()?;
    println!("✓ Team sync configured → {}", remote_url);
    Ok(cfg)
}

/// Push local profile configs to the remote.
pub fn push() -> Result<()> {
    let cfg = TeamSyncConfig::load()?;
    let repo = sync_repo_path();
    ensure_repo_exists(&repo, &cfg)?;

    // Pull first to avoid non-fast-forward rejections
    let _ = git(&repo, &["pull", "--rebase", "origin", &cfg.branch]);

    // Copy profile configs into repo
    let profiles_dir = platform::profiles_dir();
    let pm = crate::profile::ProfileManager::new(profiles_dir.clone());
    let profiles = pm.list()?;

    let synced: Vec<String> = profiles
        .iter()
        .filter(|p| cfg.should_sync(&p.name))
        .map(|p| {
            copy_profile_to_repo(&p.name, &profiles_dir, &repo)
                .map(|_| p.name.clone())
        })
        .collect::<Result<_>>()?;

    if synced.is_empty() {
        println!("Nothing to sync.");
        return Ok(());
    }

    // Commit and push
    git(&repo, &["add", "-A"])?;

    // Check if there are changes to commit
    let diff = Command::new("git")
        .args(["-C", &repo.to_string_lossy(), "diff", "--cached", "--quiet"])
        .status()?;

    if diff.success() {
        println!("Already up-to-date. Nothing to push.");
        return Ok(());
    }

    let msg = format!("sync: {} profile(s) — {}", synced.len(), synced.join(", "));
    git_commit(&repo, &cfg, &msg)?;
    git(&repo, &["push", "origin", &cfg.branch])?;

    println!("✓ Pushed {} profile(s): {}", synced.len(), synced.join(", "));
    Ok(())
}

/// Pull profile configs from the remote and merge into local profiles.
pub fn pull() -> Result<()> {
    let cfg = TeamSyncConfig::load()?;
    let repo = sync_repo_path();
    ensure_repo_exists(&repo, &cfg)?;

    git(&repo, &["fetch", "origin", &cfg.branch])?;
    git(&repo, &["reset", "--hard", &format!("origin/{}", cfg.branch)])?;

    // Copy from repo into local profiles
    let profiles_dir = repo.join("profiles");
    if !profiles_dir.exists() {
        println!("Remote has no profiles yet.");
        return Ok(());
    }

    let mut pulled = Vec::new();
    for entry in std::fs::read_dir(&profiles_dir)? {
        let entry = entry?;
        let profile_name = entry.file_name().to_string_lossy().to_string();
        if !cfg.should_sync(&profile_name) {
            continue;
        }
        let src = entry.path();
        let dst = platform::profile_dir(&profile_name);
        copy_profile_from_repo(&src, &dst)?;
        pulled.push(profile_name);
    }

    if pulled.is_empty() {
        println!("No profiles to pull.");
    } else {
        println!("✓ Pulled {} profile(s): {}", pulled.len(), pulled.join(", "));
    }
    Ok(())
}

/// Show status: last sync time, pending changes.
pub fn status() -> Result<()> {
    let cfg = TeamSyncConfig::load()?;
    let repo = sync_repo_path();

    println!("Remote : {}", cfg.remote_url);
    println!("Branch : {}", cfg.branch);

    if !repo.exists() {
        println!("Status : not initialised — run: cst team push");
        return Ok(());
    }

    // Last commit
    let last = Command::new("git")
        .args(["-C", &repo.to_string_lossy(), "log", "-1", "--format=%ci %s"])
        .output()?;
    if last.status.success() {
        println!("Last   : {}", String::from_utf8_lossy(&last.stdout).trim());
    }

    // Included/excluded
    if cfg.include_profiles.is_empty() {
        println!("Sync   : all profiles");
    } else {
        println!("Sync   : {}", cfg.include_profiles.join(", "));
    }
    if !cfg.exclude_profiles.is_empty() {
        println!("Excl.  : {}", cfg.exclude_profiles.join(", "));
    }

    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Files within a profile directory that are safe to sync.
const SYNC_FILES: &[&str] = &[
    "profile.toml",
    "settings-override.json",
    "mcp-override.json",
    "auto-switch.toml",
];

/// Per-session files that are safe to sync.
const SESSION_SYNC_FILES: &[&str] = &[
    "settings-override.json",
    "env.toml",
];

fn copy_profile_to_repo(name: &str, profiles_dir: &Path, repo: &Path) -> Result<()> {
    let src = profiles_dir.join(name);
    let dst = repo.join("profiles").join(name);
    std::fs::create_dir_all(&dst)?;

    for file in SYNC_FILES {
        let s = src.join(file);
        if s.exists() {
            std::fs::copy(&s, dst.join(file))?;
        }
    }

    // Sessions (no credentials — just overrides and env)
    let sessions_src = src.join("sessions");
    if sessions_src.exists() {
        for entry in std::fs::read_dir(&sessions_src)? {
            let entry = entry?;
            let sname = entry.file_name().to_string_lossy().to_string();
            let sdst = dst.join("sessions").join(&sname);
            std::fs::create_dir_all(&sdst)?;
            for file in SESSION_SYNC_FILES {
                let sf = entry.path().join(file);
                if sf.exists() {
                    std::fs::copy(&sf, sdst.join(file))?;
                }
            }
        }
    }

    Ok(())
}

fn copy_profile_from_repo(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;

    for file in SYNC_FILES {
        let s = src.join(file);
        if s.exists() {
            std::fs::copy(&s, dst.join(file))?;
        }
    }

    let sessions_src = src.join("sessions");
    if sessions_src.exists() {
        for entry in std::fs::read_dir(&sessions_src)? {
            let entry = entry?;
            let sdst = dst.join("sessions").join(entry.file_name());
            std::fs::create_dir_all(&sdst)?;
            for file in SESSION_SYNC_FILES {
                let sf = entry.path().join(file);
                if sf.exists() {
                    std::fs::copy(&sf, sdst.join(file))?;
                }
            }
        }
    }

    Ok(())
}

fn ensure_repo_exists(repo: &Path, cfg: &TeamSyncConfig) -> Result<()> {
    if !repo.exists() {
        init(&cfg.remote_url, &cfg.branch)?;
    }
    Ok(())
}

fn git(repo: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .args([&["-C", &repo.to_string_lossy().to_string()], args].concat())
        .status()
        .context("running git")?;
    if !status.success() {
        anyhow::bail!("git {:?} failed (exit {})", args, status);
    }
    Ok(())
}

fn git_commit(repo: &Path, cfg: &TeamSyncConfig, message: &str) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.args(["-C", &repo.to_string_lossy()]);
    if !cfg.committer_name.is_empty() {
        cmd.env("GIT_AUTHOR_NAME", &cfg.committer_name);
        cmd.env("GIT_COMMITTER_NAME", &cfg.committer_name);
    }
    if !cfg.committer_email.is_empty() {
        cmd.env("GIT_AUTHOR_EMAIL", &cfg.committer_email);
        cmd.env("GIT_COMMITTER_EMAIL", &cfg.committer_email);
    }
    cmd.args(["commit", "-m", message]);
    let s = cmd.status().context("running git commit")?;
    if !s.success() {
        anyhow::bail!("git commit failed");
    }
    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn should_sync_all_by_default() {
        let cfg = TeamSyncConfig {
            remote_url: "https://x".to_string(),
            branch: "main".to_string(),
            include_profiles: vec![],
            exclude_profiles: vec![],
            committer_name: String::new(),
            committer_email: String::new(),
        };
        assert!(cfg.should_sync("work"));
        assert!(cfg.should_sync("personal"));
    }

    #[test]
    fn exclude_list_blocks_profile() {
        let cfg = TeamSyncConfig {
            remote_url: "https://x".to_string(),
            branch: "main".to_string(),
            include_profiles: vec![],
            exclude_profiles: vec!["personal".to_string()],
            committer_name: String::new(),
            committer_email: String::new(),
        };
        assert!(cfg.should_sync("work"));
        assert!(!cfg.should_sync("personal"));
    }

    #[test]
    fn include_list_restricts_to_listed() {
        let cfg = TeamSyncConfig {
            remote_url: "https://x".to_string(),
            branch: "main".to_string(),
            include_profiles: vec!["work".to_string()],
            exclude_profiles: vec![],
            committer_name: String::new(),
            committer_email: String::new(),
        };
        assert!(cfg.should_sync("work"));
        assert!(!cfg.should_sync("personal"));
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        // Directly test the TOML round-trip without touching the real data dir
        let cfg = TeamSyncConfig {
            remote_url: "git@github.com:myorg/profiles.git".to_string(),
            branch: "main".to_string(),
            include_profiles: vec!["work".to_string()],
            exclude_profiles: vec!["personal".to_string()],
            committer_name: "Bot".to_string(),
            committer_email: "bot@example.com".to_string(),
        };
        let path = dir.path().join("team-sync.toml");
        std::fs::write(&path, toml::to_string_pretty(&cfg).unwrap()).unwrap();
        let loaded: TeamSyncConfig = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.remote_url, cfg.remote_url);
        assert_eq!(loaded.branch, cfg.branch);
        assert_eq!(loaded.include_profiles, cfg.include_profiles);
    }

    #[test]
    fn sync_files_list_excludes_auth() {
        // Verify auth/ is not in SYNC_FILES
        assert!(!SYNC_FILES.iter().any(|f| f.contains("auth")));
        assert!(!SYNC_FILES.iter().any(|f| f.contains("oauth")));
        assert!(!SYNC_FILES.iter().any(|f| f.contains("api_key")));
    }

    #[test]
    fn copy_profile_to_repo_only_copies_safe_files() {
        let src_root = TempDir::new().unwrap();
        let repo_root = TempDir::new().unwrap();
        let profile_src = src_root.path().join("work");
        std::fs::create_dir_all(profile_src.join("auth")).unwrap();
        std::fs::write(profile_src.join("profile.toml"), "name = \"work\"").unwrap();
        std::fs::write(profile_src.join("auth").join("oauth.json"), "{}").unwrap();
        std::fs::write(profile_src.join("settings-override.json"), "{}").unwrap();

        copy_profile_to_repo("work", src_root.path(), repo_root.path()).unwrap();

        let repo_profile = repo_root.path().join("profiles").join("work");
        assert!(repo_profile.join("profile.toml").exists());
        assert!(repo_profile.join("settings-override.json").exists());
        // auth/ must NOT be copied
        assert!(!repo_profile.join("auth").exists());
        assert!(!repo_profile.join("auth").join("oauth.json").exists());
    }
}
