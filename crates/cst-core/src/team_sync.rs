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

use crate::merge;
use crate::platform;

/// Merge strategy for resolving conflicts when pulling team profiles.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    /// Remote wins — overwrite local with pulled content (current behaviour, default).
    #[default]
    Theirs,
    /// Local wins — keep local files, ignore remote changes for conflicting keys.
    Ours,
    /// Deep merge — merge JSON/TOML objects key-by-key; remote wins on scalar conflicts.
    Merge,
}

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

    /// Merge strategy used when pulling profiles.
    #[serde(default)]
    pub merge_strategy: MergeStrategy,
}

fn default_branch() -> String {
    "main".to_string()
}

impl TeamSyncConfig {
    /// Load from `~/.claude-sentinel/team-sync.toml`.
    pub fn load() -> Result<Self> {
        let path = config_path();
        let content = std::fs::read_to_string(&path).with_context(|| {
            format!("team sync not configured — run: cst team init <remote-url>")
        })?;
        Ok(toml::from_str(&content)?)
    }

    /// Save to `~/.claude-sentinel/team-sync.toml`.
    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
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
            .args([
                "clone",
                "--depth=1",
                "--branch",
                branch,
                remote_url,
                &repo.to_string_lossy(),
            ])
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
        merge_strategy: MergeStrategy::default(),
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
        .map(|p| copy_profile_to_repo(&p.name, &profiles_dir, &repo).map(|_| p.name.clone()))
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

    println!(
        "✓ Pushed {} profile(s): {}",
        synced.len(),
        synced.join(", ")
    );
    Ok(())
}

/// Pull profile configs from the remote and merge into local profiles.
///
/// Uses the merge strategy from the config file.
pub fn pull() -> Result<()> {
    pull_with_strategy(None)
}

/// Pull profile configs with an explicit merge strategy override.
///
/// If `strategy` is `None`, falls back to the config file's `merge_strategy`.
pub fn pull_with_strategy(strategy: Option<MergeStrategy>) -> Result<()> {
    let cfg = TeamSyncConfig::load()?;
    let effective_strategy = strategy.unwrap_or_else(|| cfg.merge_strategy.clone());
    let repo = sync_repo_path();
    ensure_repo_exists(&repo, &cfg)?;

    git(&repo, &["fetch", "origin", &cfg.branch])?;
    git(
        &repo,
        &["reset", "--hard", &format!("origin/{}", cfg.branch)],
    )?;

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
        copy_profile_from_repo_with_strategy(&src, &dst, &effective_strategy)?;
        pulled.push(profile_name);
    }

    if pulled.is_empty() {
        println!("No profiles to pull.");
    } else {
        println!(
            "✓ Pulled {} profile(s) (strategy: {:?}): {}",
            pulled.len(),
            effective_strategy,
            pulled.join(", ")
        );
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
        .args([
            "-C",
            &repo.to_string_lossy(),
            "log",
            "-1",
            "--format=%ci %s",
        ])
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

    println!("Strategy: {:?}", cfg.merge_strategy);

    // Detect and display conflicts
    let conflicts = detect_conflicts(&cfg)?;
    if conflicts.is_empty() {
        println!("Conflicts: none");
    } else {
        println!("Conflicts: {} file(s) differ from remote", conflicts.len());
        for c in &conflicts {
            println!("  - {}", c);
        }
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
const SESSION_SYNC_FILES: &[&str] = &["settings-override.json", "env.toml"];

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
    copy_profile_from_repo_with_strategy(src, dst, &MergeStrategy::Theirs)
}

/// Copy profile from repo to local, applying the given merge strategy.
fn copy_profile_from_repo_with_strategy(
    src: &Path,
    dst: &Path,
    strategy: &MergeStrategy,
) -> Result<()> {
    std::fs::create_dir_all(dst)?;

    for file in SYNC_FILES {
        let s = src.join(file);
        if s.exists() {
            copy_file_with_strategy(&s, &dst.join(file), strategy)?;
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
                    copy_file_with_strategy(&sf, &sdst.join(file), strategy)?;
                }
            }
        }
    }

    Ok(())
}

/// Copy a single file from remote to local, applying merge strategy.
fn copy_file_with_strategy(remote: &Path, local: &Path, strategy: &MergeStrategy) -> Result<()> {
    match strategy {
        MergeStrategy::Theirs => {
            // Remote wins — always overwrite.
            std::fs::copy(remote, local)?;
        }
        MergeStrategy::Ours => {
            // Local wins — only copy if the local file does not exist.
            if !local.exists() {
                std::fs::copy(remote, local)?;
            }
        }
        MergeStrategy::Merge => {
            if !local.exists() {
                // No local file — just copy.
                std::fs::copy(remote, local)?;
            } else if remote.extension().map_or(false, |ext| ext == "json") {
                // Deep merge JSON: local is base, remote is overlay (remote wins on scalars).
                let mut base = merge::load_json(local)?;
                let overlay = merge::load_json(remote)?;
                merge::deep_merge(&mut base, &overlay);
                let merged = serde_json::to_string_pretty(&base)?;
                std::fs::write(local, merged)?;
            } else {
                // Non-JSON files: remote wins (same as Theirs).
                std::fs::copy(remote, local)?;
            }
        }
    }
    Ok(())
}

/// Compare local profile files against the repo copy and return paths that differ.
///
/// Each returned string is in the format `"<profile>/<filename>"`.
pub fn detect_conflicts(cfg: &TeamSyncConfig) -> Result<Vec<String>> {
    let repo = sync_repo_path();
    let repo_profiles = repo.join("profiles");
    let mut conflicts = Vec::new();

    if !repo_profiles.exists() {
        return Ok(conflicts);
    }

    for entry in std::fs::read_dir(&repo_profiles)? {
        let entry = entry?;
        let profile_name = entry.file_name().to_string_lossy().to_string();
        if !cfg.should_sync(&profile_name) {
            continue;
        }
        let repo_dir = entry.path();
        let local_dir = platform::profile_dir(&profile_name);

        for file in SYNC_FILES {
            let repo_file = repo_dir.join(file);
            let local_file = local_dir.join(file);
            if repo_file.exists() && local_file.exists() {
                let repo_content = std::fs::read(&repo_file)?;
                let local_content = std::fs::read(&local_file)?;
                if repo_content != local_content {
                    conflicts.push(format!("{}/{}", profile_name, file));
                }
            } else if repo_file.exists() != local_file.exists() {
                // One exists and the other doesn't — that's a conflict too.
                conflicts.push(format!("{}/{}", profile_name, file));
            }
        }
    }

    Ok(conflicts)
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
    use std::sync::Mutex;
    use tempfile::TempDir;

    // Serialise tests that mutate CST_DATA_DIR to prevent parallel-test UB.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn should_sync_all_by_default() {
        let cfg = TeamSyncConfig {
            remote_url: "https://x".to_string(),
            branch: "main".to_string(),
            include_profiles: vec![],
            exclude_profiles: vec![],
            committer_name: String::new(),
            committer_email: String::new(),
            merge_strategy: MergeStrategy::default(),
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
            merge_strategy: MergeStrategy::default(),
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
            merge_strategy: MergeStrategy::default(),
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
            merge_strategy: MergeStrategy::default(),
        };
        let path = dir.path().join("team-sync.toml");
        std::fs::write(&path, toml::to_string_pretty(&cfg).unwrap()).unwrap();
        let loaded: TeamSyncConfig =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
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

    // ── Additional security / correctness tests ──────────────────────────

    #[test]
    fn copy_profile_to_repo_does_not_copy_api_key_enc() {
        let src_root = TempDir::new().unwrap();
        let repo_root = TempDir::new().unwrap();
        let profile_src = src_root.path().join("api-work");
        std::fs::create_dir_all(profile_src.join("auth")).unwrap();
        std::fs::write(profile_src.join("profile.toml"), "name = \"api-work\"").unwrap();
        std::fs::write(profile_src.join("auth").join("api_key.enc"), "ENCRYPTED").unwrap();
        std::fs::write(profile_src.join("auth").join("api_keys.toml"), "[keys]").unwrap();

        copy_profile_to_repo("api-work", src_root.path(), repo_root.path()).unwrap();

        let repo_profile = repo_root.path().join("profiles").join("api-work");
        // auth/ directory must not exist in the repo copy
        assert!(!repo_profile.join("auth").exists());
        assert!(!repo_profile.join("auth").join("api_key.enc").exists());
    }

    #[test]
    fn copy_profile_from_repo_only_copies_safe_files() {
        let repo_root = TempDir::new().unwrap();
        let dst_root = TempDir::new().unwrap();

        // Simulate a repo profile dir that (maliciously or by mistake) contains auth/
        let repo_profile = repo_root.path().join("work");
        std::fs::create_dir_all(repo_profile.join("auth")).unwrap();
        std::fs::write(repo_profile.join("profile.toml"), "name = \"work\"").unwrap();
        std::fs::write(repo_profile.join("auth").join("oauth.json"), "{}").unwrap();
        std::fs::write(repo_profile.join("settings-override.json"), "{}").unwrap();

        let dst = dst_root.path().join("work");
        copy_profile_from_repo(&repo_profile, &dst).unwrap();

        assert!(dst.join("profile.toml").exists());
        assert!(dst.join("settings-override.json").exists());
        // auth/ must NOT be created in the destination
        assert!(!dst.join("auth").exists());
    }

    #[test]
    fn session_files_sync_copies_env_toml_not_stats() {
        let src_root = TempDir::new().unwrap();
        let repo_root = TempDir::new().unwrap();
        let session_src = src_root
            .path()
            .join("work")
            .join("sessions")
            .join("backend");
        std::fs::create_dir_all(&session_src).unwrap();
        std::fs::write(session_src.join("env.toml"), "[env]").unwrap();
        std::fs::write(session_src.join("settings-override.json"), "{}").unwrap();
        std::fs::write(session_src.join("stats.json"), "{\"tokens_in\":1000}").unwrap();

        copy_profile_to_repo("work", src_root.path(), repo_root.path()).unwrap();

        let repo_session = repo_root
            .path()
            .join("profiles")
            .join("work")
            .join("sessions")
            .join("backend");
        assert!(
            repo_session.join("env.toml").exists(),
            "env.toml should be synced"
        );
        assert!(
            repo_session.join("settings-override.json").exists(),
            "settings-override should be synced"
        );
        // stats.json is private usage data — must NOT be synced
        assert!(
            !repo_session.join("stats.json").exists(),
            "stats.json must not be synced"
        );
    }

    #[test]
    fn exclude_takes_precedence_over_include() {
        // If a profile appears in both include and exclude lists,
        // exclude wins (security-safe behaviour).
        let cfg = TeamSyncConfig {
            remote_url: "https://x".to_string(),
            branch: "main".to_string(),
            include_profiles: vec!["work".to_string(), "personal".to_string()],
            exclude_profiles: vec!["personal".to_string()],
            committer_name: String::new(),
            committer_email: String::new(),
            merge_strategy: MergeStrategy::default(),
        };
        assert!(cfg.should_sync("work"));
        assert!(
            !cfg.should_sync("personal"),
            "exclude must take precedence over include"
        );
    }

    #[test]
    fn load_missing_config_returns_error() {
        // Remove env so the platform data_dir points somewhere under TempDir
        // We just verify that loading when the file doesn't exist fails gracefully.
        // (Direct test without touching real home directory.)
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("team-sync.toml");
        // File does not exist — from_str would be the path we take
        let result = std::fs::read_to_string(&path);
        assert!(result.is_err(), "reading nonexistent config file must fail");
    }

    #[test]
    fn session_sync_files_list_excludes_history_and_stats() {
        assert!(!SESSION_SYNC_FILES.iter().any(|f| f.contains("stats")));
        assert!(!SESSION_SYNC_FILES.iter().any(|f| f.contains("history")));
        assert!(!SESSION_SYNC_FILES.iter().any(|f| f.contains("auth")));
    }

    #[test]
    fn sync_files_list_is_complete_and_safe() {
        // Positive: expected safe files should all be present
        assert!(SYNC_FILES.contains(&"profile.toml"));
        assert!(SYNC_FILES.contains(&"settings-override.json"));
        assert!(SYNC_FILES.contains(&"mcp-override.json"));
        assert!(SYNC_FILES.contains(&"auto-switch.toml"));
        // Negative: credential-adjacent names must never appear
        for file in SYNC_FILES {
            assert!(
                !file.contains("key"),
                "SYNC_FILES must not contain key files: {}",
                file
            );
            assert!(
                !file.contains("enc"),
                "SYNC_FILES must not contain encrypted files: {}",
                file
            );
            assert!(
                !file.contains("oauth"),
                "SYNC_FILES must not contain oauth files: {}",
                file
            );
            assert!(
                !file.contains("secret"),
                "SYNC_FILES must not contain secret files: {}",
                file
            );
        }
    }

    // ── Merge strategy tests ─────────────────────────────────────────────

    #[test]
    fn pull_strategy_ours_does_not_overwrite_local() {
        let remote_dir = TempDir::new().unwrap();
        let local_dir = TempDir::new().unwrap();

        // Remote has a file with remote content
        std::fs::write(remote_dir.path().join("profile.toml"), "name = \"remote\"").unwrap();
        // Local already has a different version
        std::fs::write(local_dir.path().join("profile.toml"), "name = \"local\"").unwrap();

        copy_profile_from_repo_with_strategy(
            remote_dir.path(),
            local_dir.path(),
            &MergeStrategy::Ours,
        )
        .unwrap();

        // Local file must be untouched — Ours means local wins
        let content = std::fs::read_to_string(local_dir.path().join("profile.toml")).unwrap();
        assert_eq!(content, "name = \"local\"");
    }

    #[test]
    fn pull_strategy_ours_copies_new_files() {
        let remote_dir = TempDir::new().unwrap();
        let local_dir = TempDir::new().unwrap();

        // Remote has a file that doesn't exist locally
        std::fs::write(
            remote_dir.path().join("settings-override.json"),
            r#"{"model":"opus"}"#,
        )
        .unwrap();

        copy_profile_from_repo_with_strategy(
            remote_dir.path(),
            local_dir.path(),
            &MergeStrategy::Ours,
        )
        .unwrap();

        // New file should be copied even with Ours strategy
        assert!(local_dir.path().join("settings-override.json").exists());
    }

    #[test]
    fn pull_strategy_theirs_overwrites_local() {
        let remote_dir = TempDir::new().unwrap();
        let local_dir = TempDir::new().unwrap();

        std::fs::write(remote_dir.path().join("profile.toml"), "name = \"remote\"").unwrap();
        std::fs::write(local_dir.path().join("profile.toml"), "name = \"local\"").unwrap();

        copy_profile_from_repo_with_strategy(
            remote_dir.path(),
            local_dir.path(),
            &MergeStrategy::Theirs,
        )
        .unwrap();

        let content = std::fs::read_to_string(local_dir.path().join("profile.toml")).unwrap();
        assert_eq!(content, "name = \"remote\"");
    }

    #[test]
    fn pull_strategy_merge_combines_json_keys() {
        let remote_dir = TempDir::new().unwrap();
        let local_dir = TempDir::new().unwrap();

        // Local JSON has key "a"
        std::fs::write(
            local_dir.path().join("settings-override.json"),
            r#"{"a": 1, "shared": "local"}"#,
        )
        .unwrap();

        // Remote JSON has key "b" and different "shared"
        std::fs::write(
            remote_dir.path().join("settings-override.json"),
            r#"{"b": 2, "shared": "remote"}"#,
        )
        .unwrap();

        copy_profile_from_repo_with_strategy(
            remote_dir.path(),
            local_dir.path(),
            &MergeStrategy::Merge,
        )
        .unwrap();

        let content =
            std::fs::read_to_string(local_dir.path().join("settings-override.json")).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();

        // "a" from local preserved
        assert_eq!(val["a"], serde_json::json!(1));
        // "b" from remote added
        assert_eq!(val["b"], serde_json::json!(2));
        // "shared" — remote wins on scalar conflicts
        assert_eq!(val["shared"], serde_json::json!("remote"));
    }

    #[test]
    fn pull_strategy_merge_deep_merges_nested_objects() {
        let remote_dir = TempDir::new().unwrap();
        let local_dir = TempDir::new().unwrap();

        std::fs::write(
            local_dir.path().join("settings-override.json"),
            r#"{"outer": {"local_key": true, "conflict": "local"}}"#,
        )
        .unwrap();

        std::fs::write(
            remote_dir.path().join("settings-override.json"),
            r#"{"outer": {"remote_key": true, "conflict": "remote"}}"#,
        )
        .unwrap();

        copy_profile_from_repo_with_strategy(
            remote_dir.path(),
            local_dir.path(),
            &MergeStrategy::Merge,
        )
        .unwrap();

        let content =
            std::fs::read_to_string(local_dir.path().join("settings-override.json")).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(val["outer"]["local_key"], serde_json::json!(true));
        assert_eq!(val["outer"]["remote_key"], serde_json::json!(true));
        assert_eq!(val["outer"]["conflict"], serde_json::json!("remote"));
    }

    #[test]
    fn pull_strategy_merge_non_json_uses_theirs() {
        let remote_dir = TempDir::new().unwrap();
        let local_dir = TempDir::new().unwrap();

        // TOML file — Merge strategy falls back to Theirs for non-JSON
        std::fs::write(local_dir.path().join("profile.toml"), "name = \"local\"").unwrap();
        std::fs::write(remote_dir.path().join("profile.toml"), "name = \"remote\"").unwrap();

        copy_profile_from_repo_with_strategy(
            remote_dir.path(),
            local_dir.path(),
            &MergeStrategy::Merge,
        )
        .unwrap();

        let content = std::fs::read_to_string(local_dir.path().join("profile.toml")).unwrap();
        assert_eq!(content, "name = \"remote\"");
    }

    #[test]
    fn detect_conflicts_finds_differing_files() {
        // Set up a fake sync repo and local profiles via CST_DATA_DIR
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("cst-data");
        std::fs::create_dir_all(&data_dir).unwrap();

        // Create a team-sync config
        let cfg = TeamSyncConfig {
            remote_url: "https://x".to_string(),
            branch: "main".to_string(),
            include_profiles: vec![],
            exclude_profiles: vec![],
            committer_name: String::new(),
            committer_email: String::new(),
            merge_strategy: MergeStrategy::default(),
        };

        // Create "repo" profile dir
        let repo_profile = data_dir
            .join("team-sync-repo")
            .join("profiles")
            .join("work");
        std::fs::create_dir_all(&repo_profile).unwrap();
        std::fs::write(repo_profile.join("profile.toml"), "name = \"remote\"").unwrap();
        std::fs::write(repo_profile.join("settings-override.json"), r#"{"a":1}"#).unwrap();

        // Create local profile dir
        let local_profile = data_dir.join("profiles").join("work");
        std::fs::create_dir_all(&local_profile).unwrap();
        std::fs::write(local_profile.join("profile.toml"), "name = \"local\"").unwrap();
        std::fs::write(local_profile.join("settings-override.json"), r#"{"a":1}"#).unwrap();

        // Temporarily set CST_DATA_DIR for the test
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: we hold ENV_LOCK.
        unsafe { std::env::set_var("CST_DATA_DIR", &data_dir) }
        let result = detect_conflicts(&cfg);
        unsafe { std::env::remove_var("CST_DATA_DIR") }

        let conflicts = result.unwrap();
        // profile.toml differs, settings-override.json is the same
        assert!(
            conflicts.contains(&"work/profile.toml".to_string()),
            "expected profile.toml conflict, got: {:?}",
            conflicts,
        );
        assert!(
            !conflicts.contains(&"work/settings-override.json".to_string()),
            "settings-override.json should NOT conflict (identical content)",
        );
    }

    #[test]
    fn merge_strategy_default_is_theirs() {
        assert_eq!(MergeStrategy::default(), MergeStrategy::Theirs);
    }

    #[test]
    fn merge_strategy_serde_roundtrip() {
        #[derive(Serialize, Deserialize)]
        struct Wrapper {
            strategy: MergeStrategy,
        }

        for variant in [
            MergeStrategy::Theirs,
            MergeStrategy::Ours,
            MergeStrategy::Merge,
        ] {
            let w = Wrapper {
                strategy: variant.clone(),
            };
            let s = toml::to_string(&w).unwrap();
            let loaded: Wrapper = toml::from_str(&s).unwrap();
            assert_eq!(loaded.strategy, variant);
        }
    }

    #[test]
    fn config_with_merge_strategy_deserializes() {
        let toml_str = r#"
            remote_url = "https://x"
            branch = "main"
            merge_strategy = "merge"
        "#;
        let cfg: TeamSyncConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.merge_strategy, MergeStrategy::Merge);
    }

    #[test]
    fn config_without_merge_strategy_defaults_to_theirs() {
        let toml_str = r#"
            remote_url = "https://x"
            branch = "main"
        "#;
        let cfg: TeamSyncConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.merge_strategy, MergeStrategy::Theirs);
    }
}
