//! `.cstrc` auto-detect — direnv-style per-project profile selection.
//!
//! Walks from `start_dir` upward looking for a `.cstrc` file, then checks
//! git remote URL patterns before falling back to the explicit `profile` field.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Result of auto-detect: which profile (and optional session) to activate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectResult {
    pub profile: String,
    pub session: Option<String>,
    pub source: DetectSource,
}

/// Where the detection came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectSource {
    /// Explicit `profile` field in a `.cstrc` at this path.
    CstRc(PathBuf),
    /// Matched a git remote URL pattern.
    GitRemote { pattern: String },
}

/// Contents of a `.cstrc` file (TOML).
///
/// ```toml
/// profile = "work"
/// session = "backend"
///
/// [[auto_detect]]
/// git_remote_pattern = "github.com/mycompany/*"
/// profile = "work"
/// session = "backend"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CstRc {
    /// Profile to activate when this `.cstrc` is found.
    pub profile: Option<String>,
    /// Session override (defaults to "default" if absent).
    pub session: Option<String>,
    /// Git remote URL patterns for more specific matching.
    #[serde(default)]
    pub auto_detect: Vec<GitPattern>,
}

/// A git remote URL → profile mapping entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitPattern {
    /// Glob matching the git remote URL (e.g. `"github.com/myco/*"`).
    /// Supports `*` as a single-segment wildcard.
    pub git_remote_pattern: String,
    /// Profile to activate when this pattern matches.
    pub profile: String,
    /// Optional session override.
    pub session: Option<String>,
}

/// Detect the appropriate profile for `start_dir`.
///
/// Returns `None` if no `.cstrc` is found anywhere in the directory tree.
pub fn detect(start_dir: &Path) -> Option<DetectResult> {
    let rc_path = find_cstrc(start_dir)?;
    let rc = load_cstrc(&rc_path).ok()?;

    // Git remote patterns take priority over the plain `profile` field.
    if !rc.auto_detect.is_empty() {
        if let Some(remote_url) = git_remote_url(start_dir) {
            for p in &rc.auto_detect {
                if glob_match(&p.git_remote_pattern, &remote_url) {
                    return Some(DetectResult {
                        profile: p.profile.clone(),
                        session: p.session.clone(),
                        source: DetectSource::GitRemote {
                            pattern: p.git_remote_pattern.clone(),
                        },
                    });
                }
            }
        }
    }

    // Fall back to explicit profile in `.cstrc`.
    rc.profile.map(|profile| DetectResult {
        session: rc.session,
        source: DetectSource::CstRc(rc_path),
        profile,
    })
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Walk up the directory tree from `start` to find the nearest `.cstrc`.
fn find_cstrc(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = dir.join(".cstrc");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn load_cstrc(path: &Path) -> Result<CstRc> {
    let content = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&content)?)
}

/// Get the `origin` remote URL for the repo containing `dir`.
fn git_remote_url(dir: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["-C", &dir.to_string_lossy(), "remote", "get-url", "origin"])
        .output()
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    } else {
        None
    }
}

/// Glob match with `*` as a multi-character wildcard.
///
/// Both `pattern` and `value` are first normalised (SSH/HTTPS → bare host/path).
pub(crate) fn glob_match(pattern: &str, value: &str) -> bool {
    let v = normalise_git_url(value);
    let p = normalise_git_url(pattern);
    let pv: Vec<char> = p.chars().collect();
    let vv: Vec<char> = v.chars().collect();
    glob_chars(&pv, &vv)
}

fn normalise_git_url(url: &str) -> String {
    let url = url.trim_end_matches(".git");
    // git@github.com:org/repo → github.com/org/repo
    if let Some(rest) = url.strip_prefix("git@") {
        return rest.replacen(':', "/", 1);
    }
    // https:// / http:// / ssh://git@ / ssh://
    for prefix in &["https://", "http://", "ssh://git@", "ssh://"] {
        if let Some(rest) = url.strip_prefix(prefix) {
            return rest.to_string();
        }
    }
    url.to_string()
}

/// Linear-time glob matching using a two-pointer DP approach.
///
/// The recursive version had exponential worst-case complexity: a pattern like
/// "*a*a*a*" against "bbbbb" caused O(2^n) recursive calls. This iterative
/// version tracks the last `*` position and is O(|p| * |v|) in the worst case,
/// but O(|p| + |v|) for most inputs — safe against crafted `.cstrc` patterns.
fn glob_chars(p: &[char], v: &[char]) -> bool {
    let (mut pi, mut vi) = (0usize, 0usize);
    let (mut star_pi, mut star_vi) = (usize::MAX, 0usize);

    while vi < v.len() {
        if pi < p.len() && p[pi] == '*' {
            star_pi = pi;
            star_vi = vi;
            pi += 1;
        } else if pi < p.len() && p[pi] == v[vi] {
            pi += 1;
            vi += 1;
        } else if star_pi != usize::MAX {
            // Backtrack: the star matches one more character
            star_vi += 1;
            vi = star_vi;
            pi = star_pi + 1;
        } else {
            return false;
        }
    }

    // Consume trailing stars
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── glob_match ────────────────────────────────────────────────────────

    #[test]
    fn glob_exact_match() {
        assert!(glob_match("github.com/myco/repo", "github.com/myco/repo"));
    }

    #[test]
    fn glob_star_suffix() {
        assert!(glob_match("github.com/myco/*", "github.com/myco/backend"));
        assert!(glob_match("github.com/myco/*", "github.com/myco/frontend"));
        assert!(!glob_match("github.com/myco/*", "github.com/other/repo"));
    }

    #[test]
    fn glob_star_prefix() {
        assert!(glob_match("*.myco.internal/*", "git.myco.internal/api"));
    }

    #[test]
    fn glob_no_match() {
        assert!(!glob_match("github.com/myco/*", "gitlab.com/myco/repo"));
    }

    // ── normalise_git_url ─────────────────────────────────────────────────

    #[test]
    fn normalise_ssh_url() {
        assert_eq!(
            normalise_git_url("git@github.com:myco/repo.git"),
            "github.com/myco/repo"
        );
    }

    #[test]
    fn normalise_https_url() {
        assert_eq!(
            normalise_git_url("https://github.com/myco/repo.git"),
            "github.com/myco/repo"
        );
    }

    #[test]
    fn normalise_plain() {
        assert_eq!(
            normalise_git_url("github.com/myco/repo"),
            "github.com/myco/repo"
        );
    }

    // ── find_cstrc ────────────────────────────────────────────────────────

    #[test]
    fn find_cstrc_walks_up() {
        let root = TempDir::new().unwrap();
        let nested = root.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&nested).unwrap();
        let rc = root.path().join(".cstrc");
        std::fs::write(&rc, r#"profile = "work""#).unwrap();
        assert_eq!(find_cstrc(&nested).unwrap(), rc);
    }

    #[test]
    fn find_cstrc_prefers_nearest() {
        let root = TempDir::new().unwrap();
        let child = root.path().join("sub");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::write(root.path().join(".cstrc"), r#"profile = "outer""#).unwrap();
        let inner = child.join(".cstrc");
        std::fs::write(&inner, r#"profile = "inner""#).unwrap();
        assert_eq!(find_cstrc(&child).unwrap(), inner);
    }

    #[test]
    fn find_cstrc_absent_returns_none() {
        let dir = TempDir::new().unwrap();
        assert!(find_cstrc(dir.path()).is_none());
    }

    // ── detect ────────────────────────────────────────────────────────────

    #[test]
    fn detect_reads_profile_and_session() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join(".cstrc"),
            "profile = \"work\"\nsession = \"backend\"",
        )
        .unwrap();
        let r = detect(dir.path()).unwrap();
        assert_eq!(r.profile, "work");
        assert_eq!(r.session.as_deref(), Some("backend"));
        assert_eq!(r.source, DetectSource::CstRc(dir.path().join(".cstrc")));
    }

    #[test]
    fn detect_profile_only() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".cstrc"), "profile = \"personal\"").unwrap();
        let r = detect(dir.path()).unwrap();
        assert_eq!(r.profile, "personal");
        assert!(r.session.is_none());
    }

    #[test]
    fn detect_no_cstrc_returns_none() {
        let dir = TempDir::new().unwrap();
        assert!(detect(dir.path()).is_none());
    }

    #[test]
    fn detect_empty_cstrc_returns_none() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".cstrc"), "").unwrap();
        assert!(detect(dir.path()).is_none());
    }

    // ── Error / malformed input ───────────────────────────────────────────

    #[test]
    fn detect_malformed_toml_returns_none() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".cstrc"), "profile = [[[invalid toml").unwrap();
        // Must not panic; silently returns None on parse failure
        assert!(detect(dir.path()).is_none());
    }

    #[test]
    fn detect_only_auto_detect_entries_no_explicit_profile_returns_none_when_no_git_match() {
        // A .cstrc with only [[auto_detect]] entries and no bare `profile` field:
        // if no git remote matches, detect() should return None.
        let dir = TempDir::new().unwrap();
        let rc = r#"
[[auto_detect]]
git_remote_pattern = "github.com/myco/*"
profile = "work"
"#;
        std::fs::write(dir.path().join(".cstrc"), rc).unwrap();
        // dir is not a git repo, so git_remote_url returns None → no pattern match → None
        assert!(detect(dir.path()).is_none());
    }

    // ── glob_match edge cases ─────────────────────────────────────────────

    #[test]
    fn glob_empty_pattern_matches_empty_value() {
        assert!(glob_match("", ""));
    }

    #[test]
    fn glob_empty_pattern_does_not_match_nonempty_value() {
        assert!(!glob_match("", "anything"));
    }

    #[test]
    fn glob_star_only_matches_any_string() {
        assert!(glob_match("*", ""));
        assert!(glob_match("*", "a"));
        assert!(glob_match("*", "github.com/org/repo"));
    }

    #[test]
    fn glob_star_matches_empty_segment() {
        // "github.com/myco/*" should also match "github.com/myco/" (empty segment after /)
        assert!(glob_match("github.com/myco/*", "github.com/myco/"));
    }

    #[test]
    fn glob_double_star_matches_any() {
        // Consecutive stars should behave like a single star
        assert!(glob_match("**", "anything"));
        assert!(glob_match("**", ""));
        assert!(glob_match("a**b", "axyzb"));
        assert!(glob_match("a**b", "ab")); // zero chars between a and b
        assert!(!glob_match("a**b", "axyzc")); // must end with b
    }

    #[test]
    fn glob_middle_star() {
        assert!(glob_match("github.com/*/repo", "github.com/myco/repo"));
        assert!(!glob_match("github.com/*/repo", "github.com/myco/other"));
    }

    // ── normalise_git_url ─────────────────────────────────────────────────

    #[test]
    fn normalise_http_url() {
        assert_eq!(
            normalise_git_url("http://github.com/myco/repo.git"),
            "github.com/myco/repo"
        );
    }

    #[test]
    fn normalise_ssh_scheme_url() {
        assert_eq!(
            normalise_git_url("ssh://git@github.com/myco/repo.git"),
            "github.com/myco/repo"
        );
    }

    #[test]
    fn normalise_idempotent() {
        let bare = "github.com/myco/repo";
        assert_eq!(normalise_git_url(bare), bare);
        // Applying twice produces same result
        assert_eq!(normalise_git_url(&normalise_git_url(bare)), bare);
    }

    #[test]
    fn normalise_strips_dot_git_only_once() {
        // Should not strip double .git.git
        let url = "github.com/myco/repo.git";
        assert_eq!(normalise_git_url(url), "github.com/myco/repo");
    }

    // ── find_cstrc edge cases ─────────────────────────────────────────────

    #[test]
    fn find_cstrc_in_current_dir() {
        let dir = TempDir::new().unwrap();
        let rc = dir.path().join(".cstrc");
        std::fs::write(&rc, r#"profile = "work""#).unwrap();
        assert_eq!(find_cstrc(dir.path()).unwrap(), rc);
    }
}
