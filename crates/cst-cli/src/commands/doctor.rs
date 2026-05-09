//! `cst doctor` — full health check for Claude Sentinel.
//!
//! Checks:
//!  1. Claude Code installation (claude binary + ~/.claude/)
//!  2. Data directory and structure
//!  3. Each profile: config validity, auth files, sessions, symlinks
//!  4. Daemon status and PID file health
//!  5. Pending broadcast or switch files (stale?)
//!  6. Shell integration (eval block in rc file)

use anyhow::Result;
use cst_core::{
    auto_switch::daemon as daemon_core, config::GlobalConfig, platform, profile::ProfileManager,
    session::SessionManager,
};

struct Check {
    label: String,
    passed: bool,
    detail: Option<String>,
}

impl Check {
    fn ok(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            passed: true,
            detail: None,
        }
    }
    fn ok_detail(label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            passed: true,
            detail: Some(detail.into()),
        }
    }
    fn fail(label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            passed: false,
            detail: Some(detail.into()),
        }
    }
    fn warn(label: impl Into<String>, detail: impl Into<String>) -> Self {
        // Warn is displayed as a note, not a failure
        Self {
            label: label.into(),
            passed: true,
            detail: Some(format!("note: {}", detail.into())),
        }
    }
}

pub fn run() -> Result<()> {
    let mut checks: Vec<Check> = Vec::new();
    let mut failures = 0;

    // ── 1. Claude Code ────────────────────────────────────────────────────
    section("Claude Code");

    match which::which("claude") {
        Ok(path) => checks.push(Check::ok_detail(
            "claude binary",
            path.to_string_lossy().to_string(),
        )),
        Err(_) => checks.push(Check::fail(
            "claude binary",
            "not found in PATH — is Claude Code installed?",
        )),
    }

    let global_claude = platform::global_claude_dir();
    if global_claude.exists() {
        checks.push(Check::ok_detail(
            "~/.claude/ directory",
            global_claude.display().to_string(),
        ));
    } else {
        checks.push(Check::fail(
            "~/.claude/ directory",
            "not found — Claude Code may not be configured",
        ));
    }

    let claude_json = platform::global_claude_json();
    if claude_json.exists() {
        checks.push(Check::ok("~/.claude.json (OAuth creds)"));
    } else {
        checks.push(Check::warn(
            "~/.claude.json",
            "not found — OAuth profiles will need login",
        ));
    }

    flush_checks(&mut checks, &mut failures);

    // ── 2. Data directory ─────────────────────────────────────────────────
    section("Data Directory");

    let data_dir = platform::data_dir();
    if data_dir.exists() {
        checks.push(Check::ok_detail(
            "~/.claude-sentinel/",
            data_dir.display().to_string(),
        ));
    } else {
        checks.push(Check::fail(
            "~/.claude-sentinel/",
            "not found — run: cst init",
        ));
    }

    let profiles_dir = platform::profiles_dir();
    if profiles_dir.exists() {
        checks.push(Check::ok("profiles/"));
    } else {
        checks.push(Check::warn(
            "profiles/",
            "no profiles yet — run: cst new <name>",
        ));
    }

    match GlobalConfig::load() {
        Ok(cfg) if !cfg.current_profile.is_empty() => {
            checks.push(Check::ok_detail(
                "config.toml",
                format!("active: {}:{}", cfg.current_profile, cfg.current_session),
            ));
        }
        Ok(_) => checks.push(Check::warn("config.toml", "no active profile")),
        Err(e) => checks.push(Check::fail("config.toml", e.to_string())),
    }

    flush_checks(&mut checks, &mut failures);

    // ── 3. Profiles & sessions ────────────────────────────────────────────
    section("Profiles & Sessions");

    let pm = ProfileManager::new(platform::profiles_dir());
    match pm.list() {
        Err(e) => {
            checks.push(Check::fail("profile list", e.to_string()));
            flush_checks(&mut checks, &mut failures);
        }
        Ok(profiles) if profiles.is_empty() => {
            checks.push(Check::warn("profiles", "none found — run: cst new <name>"));
            flush_checks(&mut checks, &mut failures);
        }
        Ok(profiles) => {
            for p in &profiles {
                // Profile config readable
                let profile_dir = platform::profile_dir(&p.name);
                checks.push(Check::ok_detail(
                    format!("profile/{}", p.name),
                    format!("auth={}", p.auth_type),
                ));

                // Auth directory
                let auth_dir = profile_dir.join("auth");
                if !auth_dir.exists() {
                    checks.push(Check::fail(
                        format!("  {}/auth/", p.name),
                        "missing — run: cst sync",
                    ));
                }

                // Sessions
                let sm = SessionManager::new(platform::profile_dir(&p.name));
                match sm.list() {
                    Err(e) => {
                        checks.push(Check::fail(format!("  {}/sessions", p.name), e.to_string()))
                    }
                    Ok(sessions) => {
                        for s in &sessions {
                            let claude_dir = platform::claude_config_dir(&p.name, &s.name);
                            if claude_dir.exists() {
                                // Check key symlinks
                                let missing: Vec<&str> = ["agents", "rules", "skills", "CLAUDE.md"]
                                    .iter()
                                    .filter(|&&item| !claude_dir.join(item).exists())
                                    .copied()
                                    .collect();
                                if missing.is_empty() {
                                    checks.push(Check::ok(format!(
                                        "  {}:{} .claude/ symlinks",
                                        p.name, s.name
                                    )));
                                } else {
                                    checks.push(Check::warn(
                                        format!("  {}:{} .claude/", p.name, s.name),
                                        format!(
                                            "missing symlinks: {} — run: cst sync",
                                            missing.join(", ")
                                        ),
                                    ));
                                }
                            } else {
                                checks.push(Check::fail(
                                    format!("  {}:{} .claude/", p.name, s.name),
                                    "config dir missing — run: cst sync",
                                ));
                            }
                        }
                    }
                }
            }
            flush_checks(&mut checks, &mut failures);
        }
    }

    // ── 4. Daemon ─────────────────────────────────────────────────────────
    section("Daemon");

    let pid_file = daemon_core::pid_file();
    if daemon_core::is_running() {
        let pid = std::fs::read_to_string(&pid_file)
            .unwrap_or_default()
            .trim()
            .to_string();
        checks.push(Check::ok_detail("daemon", format!("running (pid {})", pid)));
    } else if pid_file.exists() {
        checks.push(Check::warn(
            "daemon pid file",
            "stale PID file found but process is not running — run: cst daemon start",
        ));
    } else {
        checks.push(Check::warn(
            "daemon",
            "not running — auto-switch inactive. Start with: cst daemon start",
        ));
    }

    // Broadcast file
    let broadcast_path = platform::data_dir().join("broadcast-switch.json");
    if broadcast_path.exists() {
        if let Some(b) = cst_core::broadcast::BroadcastSwitch::load_active() {
            checks.push(Check::warn(
                "broadcast-switch.json",
                format!(
                    "active: {} → {} (expires {})",
                    b.from,
                    b.to,
                    b.expires_at.format("%H:%M:%S")
                ),
            ));
        } else {
            checks.push(Check::warn(
                "broadcast-switch.json",
                "stale file (expired, will be cleaned up)",
            ));
        }
    }

    flush_checks(&mut checks, &mut failures);

    // ── 5. Shell integration ──────────────────────────────────────────────
    section("Shell Integration");

    let rc_files = [
        dirs::home_dir().map(|h| h.join(".zshrc")),
        dirs::home_dir().map(|h| h.join(".bashrc")),
        dirs::home_dir().map(|h| h.join(".bash_profile")),
    ];

    let mut shell_ok = false;
    for rc in rc_files.iter().flatten() {
        if rc.exists() {
            if let Ok(content) = std::fs::read_to_string(rc) {
                if content.contains("cst shell-init") {
                    checks.push(Check::ok_detail(
                        "shell-init",
                        format!("found in {}", rc.display()),
                    ));
                    shell_ok = true;
                    break;
                }
            }
        }
    }
    if !shell_ok {
        checks.push(Check::warn(
            "shell-init",
            "not found in rc file — add: eval \"$(cst shell-init)\"",
        ));
    }

    flush_checks(&mut checks, &mut failures);

    // ── Summary ───────────────────────────────────────────────────────────
    println!();
    if failures == 0 {
        println!("✓  All checks passed.");
    } else {
        eprintln!("✗  {} check(s) failed. See above for details.", failures);
        std::process::exit(1);
    }
    Ok(())
}

/// Per-profile validation with full credential checks.
pub fn validate(profile: &str) -> Result<()> {
    cst_core::profile::validate_profile_name(profile)?;
    let pm = ProfileManager::new(platform::profiles_dir());
    let p = pm.load(profile)?;

    println!("Profile  : {}", p.name);
    println!("Auth     : {}", p.auth_type);
    println!("Created  : {}", p.created_at.format("%Y-%m-%d %H:%M UTC"));
    if !p.description.is_empty() {
        println!("Desc     : {}", p.description);
    }
    if let Some(tmpl) = &p.template {
        println!("Template : {}", tmpl);
    }
    println!();

    // Validate auth files exist
    let profile_dir = platform::profile_dir(profile);
    let auth_dir = profile_dir.join("auth");
    let mut ok = true;

    match p.auth_type {
        cst_core::profile::AuthType::OAuth => {
            let oauth_path = auth_dir.join("oauth.json");
            if oauth_path.exists() {
                println!("✓ OAuth credentials file exists");
            } else {
                eprintln!("✗ OAuth credentials missing — run: cst login {}", profile);
                ok = false;
            }
        }
        cst_core::profile::AuthType::Api => {
            let keys_path = auth_dir.join("api_keys.toml");
            if keys_path.exists() {
                if let Ok(contents) = std::fs::read_to_string(&keys_path) {
                    if let Ok(pool) =
                        toml::from_str::<cst_core::auth::apikey::ApiKeyPool>(&contents)
                    {
                        let slots = pool.sorted_slots();
                        println!("✓ API key pool: {} slot(s)", slots.len());
                    }
                }
            } else {
                eprintln!("✗ No API keys — run: cst add-key {}", profile);
                ok = false;
            }
        }
        cst_core::profile::AuthType::Bedrock => {
            let aws_path = auth_dir.join("aws.toml");
            if aws_path.exists() {
                println!("✓ AWS credentials file exists");
            } else {
                eprintln!("✗ AWS credentials missing at {}", aws_path.display());
                ok = false;
            }
        }
        cst_core::profile::AuthType::Vertex => {
            let vertex_path = auth_dir.join("vertex.toml");
            if vertex_path.exists() {
                println!("✓ Vertex AI config file exists");
            } else {
                eprintln!("✗ Vertex AI config missing at {}", vertex_path.display());
                ok = false;
            }
        }
    }

    // Sessions
    let sm = SessionManager::new(platform::profile_dir(profile));
    let sessions = sm.list().unwrap_or_default();
    println!("Sessions : {}", sessions.len());
    for s in &sessions {
        let claude_dir = platform::claude_config_dir(profile, &s.name);
        let marker = if claude_dir.exists() { "✓" } else { "✗" };
        println!("  [{marker}] {}", s.name);
    }

    if ok {
        println!("\n✓ Profile '{}' is valid", profile);
    } else {
        eprintln!("\n✗ Profile '{}' has issues", profile);
    }
    Ok(())
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn section(name: &str) {
    println!("\n── {} {}", name, "─".repeat(50 - name.len().min(48)));
}

fn flush_checks(checks: &mut Vec<Check>, failures: &mut usize) {
    for c in checks.drain(..) {
        let marker = if c.passed { "✓" } else { "✗" };
        if !c.passed {
            *failures += 1;
        }
        if let Some(detail) = c.detail {
            println!("  {marker} {}  ({})", c.label, detail);
        } else {
            println!("  {marker} {}", c.label);
        }
    }
}
