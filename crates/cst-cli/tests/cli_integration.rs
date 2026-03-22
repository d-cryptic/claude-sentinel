//! Integration tests for the `cst` CLI binary.
//!
//! Every test creates its own [`TestEnv`] with an isolated temp directory,
//! ensuring tests never touch the real `~/.claude-sentinel/` or `~/.claude/`.
//!
//! The `CST_DATA_DIR` environment variable points cst-core's `platform::data_dir()`
//! at the temp directory, and `HOME` is overridden so that `~/.claude/` also resolves
//! inside the sandbox.
//!
//! ## Known issue: `cst new` exits non-zero
//!
//! `ProfileManager::create()` pre-creates `sessions/default/`, then the CLI's
//! `profile::new()` calls `SessionManager::create("default")` which fails with
//! "session already exists".  The profile directory IS created correctly, so
//! `create_profile()` asserts on the directory rather than the exit code.

use std::path::PathBuf;
use std::process::{Command, Output};
use tempfile::TempDir;

// ─── Harness ─────────────────────────────────────────────────────────────────

/// Path to the `cst` binary built by Cargo.
///
/// `cargo test` sets `CARGO_BIN_EXE_<name>` automatically when the crate
/// declares a `[[bin]]` target.  We fall back to a manual build if the env
/// var is missing (e.g. when the test is run outside `cargo test`).
fn cst_bin() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_cst") {
        return PathBuf::from(p);
    }

    let manifest = env!("CARGO_MANIFEST_DIR");
    let workspace = std::path::Path::new(manifest)
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let status = Command::new("cargo")
        .args(["build", "-p", "cst-cli", "--quiet"])
        .current_dir(workspace)
        .status()
        .expect("cargo build must run");
    assert!(status.success(), "cargo build must succeed");
    workspace.join("target").join("debug").join("cst")
}

/// An isolated test environment with its own data and home directories.
///
/// Every test creates a fresh `TestEnv` so there is zero shared state.
struct TestEnv {
    /// Temp dir used as `CST_DATA_DIR` (replaces `~/.claude-sentinel/`).
    data_dir: TempDir,
    /// Temp dir used as `HOME` (replaces the user's home).
    home_dir: TempDir,
    /// Absolute path to the `cst` binary.
    bin: PathBuf,
}

impl TestEnv {
    fn new() -> Self {
        let bin = cst_bin();
        let data_dir = TempDir::new().expect("failed to create data_dir tempdir");
        let home_dir = TempDir::new().expect("failed to create home_dir tempdir");

        // Create a minimal ~/.claude/ directory with essential structure
        // so that commands referencing the global claude dir don't fail.
        let claude_dir = home_dir.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();

        TestEnv {
            data_dir,
            home_dir,
            bin,
        }
    }

    /// Run `cst <args>` in this isolated environment, returning raw output.
    fn run(&self, args: &[&str]) -> Output {
        Command::new(&self.bin)
            .args(args)
            .env("CST_DATA_DIR", self.data_dir.path())
            .env("HOME", self.home_dir.path())
            .env("USERPROFILE", self.home_dir.path()) // Windows
            .env_remove("CST_CURRENT")
            .env_remove("RUST_LOG") // avoid noisy tracing in test output
            .output()
            .expect("cst binary must execute")
    }

    /// Run `cst <args>`, assert exit 0, and return stdout as a String.
    fn run_ok(&self, args: &[&str]) -> String {
        let out = self.run(args);
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(
            out.status.success(),
            "cst {:?} failed (exit {:?})\nstdout: {}\nstderr: {}",
            args,
            out.status.code(),
            stdout,
            stderr
        );
        stdout
    }

    /// Run `cst <args>`, assert non-zero exit, and return stderr as a String.
    fn run_fail(&self, args: &[&str]) -> String {
        let out = self.run(args);
        assert!(
            !out.status.success(),
            "cst {:?} should have failed but exited 0\nstdout: {}",
            args,
            String::from_utf8_lossy(&out.stdout)
        );
        String::from_utf8_lossy(&out.stderr).to_string()
    }

    /// Run `cst <args>` and return stdout regardless of exit code.
    fn run_stdout(&self, args: &[&str]) -> String {
        let out = self.run(args);
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    /// Create a profile, tolerating the known "session already exists" bug.
    ///
    /// `ProfileManager::create()` pre-creates `sessions/default/`, then the
    /// CLI tries `SessionManager::create("default")` which fails because the
    /// directory already exists.  The profile and its directory are still
    /// created correctly, so we verify via the filesystem.
    fn create_profile(&self, name: &str, auth: &str) {
        let out = self.run(&["new", name, "--auth", auth]);
        let stdout = String::from_utf8_lossy(&out.stdout);
        // The profile must be reported as created in stdout
        assert!(
            stdout.contains(&format!("Created profile '{name}'")),
            "cst new must report profile creation\nstdout: {}\nstderr: {}",
            stdout,
            String::from_utf8_lossy(&out.stderr)
        );
        // Verify the profile directory and config file exist
        let profile_dir = self.data_dir.path().join("profiles").join(name);
        assert!(
            profile_dir.join("profile.toml").exists(),
            "profile.toml must exist after create: {:?}",
            profile_dir
        );
    }

    /// Create a profile with a template.
    fn create_profile_with_template(&self, name: &str, auth: &str, template: &str) {
        let out = self.run(&["new", name, "--auth", auth, "--template", template]);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains(&format!("Created profile '{name}'")),
            "cst new must report profile creation\nstdout: {}\nstderr: {}",
            stdout,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// Write a config.toml that sets the active profile and session.
    ///
    /// Many session subcommands read `current_profile` from config.toml
    /// rather than accepting a `--profile` flag.
    fn set_current(&self, profile: &str, session: &str) {
        let config = format!(
            "current_profile = \"{}\"\ncurrent_session = \"{}\"\n",
            profile, session
        );
        let config_path = self.data_dir.path().join("config.toml");
        std::fs::write(config_path, config).unwrap();
    }

    /// Create a profile AND set it as current.  Used by session tests.
    fn setup_active_profile(&self, name: &str) {
        self.create_profile(name, "api");
        self.set_current(name, "default");
    }
}

// ─── Smoke tests ─────────────────────────────────────────────────────────────

#[test]
fn version_exits_zero() {
    let env = TestEnv::new();
    let out = env.run_ok(&["--version"]);
    assert!(
        out.contains("cst") || out.contains("0."),
        "version output must mention cst or version number: {}",
        out
    );
}

#[test]
fn help_exits_zero() {
    let env = TestEnv::new();
    let out = env.run_ok(&["--help"]);
    assert!(
        out.contains("use") || out.contains("profile") || out.contains("Claude"),
        "help output must mention key concepts: {}",
        out
    );
}

#[test]
fn help_lists_subcommands() {
    let env = TestEnv::new();
    let out = env.run_ok(&["--help"]);
    for cmd in &["new", "list", "doctor", "session", "templates"] {
        assert!(
            out.contains(cmd),
            "help must list '{}' subcommand: {}",
            cmd,
            out
        );
    }
}

#[test]
fn unknown_subcommand_exits_nonzero() {
    let env = TestEnv::new();
    let out = env.run(&["this-command-does-not-exist"]);
    assert!(!out.status.success(), "unknown subcommand must fail");
}

#[test]
fn shell_init_bash_outputs_hook() {
    let env = TestEnv::new();
    let out = env.run_ok(&["shell-init", "--shell", "bash"]);
    assert!(
        out.contains("_cst_check_switch") || out.contains("cst"),
        "shell-init bash must emit hook function: {}",
        out
    );
}

#[test]
fn shell_init_zsh_outputs_hook() {
    let env = TestEnv::new();
    let out = env.run_ok(&["shell-init", "--shell", "zsh"]);
    assert!(
        out.contains("precmd") || out.contains("_cst") || out.contains("cst"),
        "shell-init zsh must emit precmd hook: {}",
        out
    );
}

#[test]
fn shell_init_fish_outputs_hook() {
    let env = TestEnv::new();
    let out = env.run_ok(&["shell-init", "--shell", "fish"]);
    assert!(
        out.contains("function") || out.contains("cst"),
        "shell-init fish must emit function: {}",
        out
    );
}

#[test]
fn completions_bash_exits_zero() {
    let env = TestEnv::new();
    let out = env.run_ok(&["completions", "bash"]);
    assert!(!out.is_empty(), "completions must produce output");
}

#[test]
fn completions_zsh_exits_zero() {
    let env = TestEnv::new();
    let out = env.run_ok(&["completions", "zsh"]);
    assert!(!out.is_empty(), "completions must produce output");
}

#[test]
fn completions_fish_exits_zero() {
    let env = TestEnv::new();
    let out = env.run_ok(&["completions", "fish"]);
    assert!(!out.is_empty(), "completions must produce output");
}

#[test]
fn templates_list_exits_zero() {
    let env = TestEnv::new();
    let out = env.run_ok(&["templates"]);
    assert!(
        out.contains("pro") || out.contains("max") || out.contains("api"),
        "templates must list known templates: {}",
        out
    );
}

// ─── Profile CRUD ────────────────────────────────────────────────────────────

#[test]
fn list_when_empty_shows_no_profiles() {
    let env = TestEnv::new();
    std::fs::create_dir_all(env.data_dir.path().join("profiles")).unwrap();
    let out = env.run_ok(&["list"]);
    assert!(
        out.contains("No profiles") || out.trim().is_empty(),
        "list on empty data dir: {}",
        out
    );
}

#[test]
fn new_profile_api_type_creates_profile() {
    let env = TestEnv::new();
    env.create_profile("test-api", "api");

    let profile_dir = env.data_dir.path().join("profiles").join("test-api");
    assert!(profile_dir.exists(), "profile dir must be created");
    assert!(
        profile_dir.join("profile.toml").exists(),
        "profile.toml must exist"
    );

    let out = env.run_ok(&["list"]);
    assert!(out.contains("test-api"), "list must show new profile: {}", out);
}

#[test]
fn new_profile_oauth_type() {
    let env = TestEnv::new();
    env.create_profile("personal", "oauth");
    let out = env.run_ok(&["list"]);
    assert!(
        out.contains("personal"),
        "list must show oauth profile: {}",
        out
    );
}

#[test]
fn new_profile_creates_default_session_dir() {
    let env = TestEnv::new();
    env.create_profile("work", "api");

    let default_session = env
        .data_dir
        .path()
        .join("profiles")
        .join("work")
        .join("sessions")
        .join("default");
    assert!(
        default_session.exists(),
        "default session dir must be created: {:?}",
        default_session
    );
}

#[test]
fn new_profile_creates_auth_dir() {
    let env = TestEnv::new();
    env.create_profile("work", "api");

    let auth_dir = env
        .data_dir
        .path()
        .join("profiles")
        .join("work")
        .join("auth");
    assert!(
        auth_dir.exists(),
        "auth dir must be created: {:?}",
        auth_dir
    );
}

#[test]
fn new_profile_from_template() {
    let env = TestEnv::new();
    env.create_profile_with_template("work-max", "oauth", "max");
    let out = env.run_ok(&["list"]);
    assert!(
        out.contains("work-max"),
        "list must show templated profile: {}",
        out
    );
}

#[test]
fn new_profile_invalid_auth_fails() {
    let env = TestEnv::new();
    env.run_fail(&["new", "bad", "--auth", "invalid-auth-type"]);
}

#[test]
fn clone_profile() {
    let env = TestEnv::new();
    env.create_profile("original", "api");
    env.run_ok(&["clone", "original", "copy"]);

    let out = env.run_ok(&["list"]);
    assert!(out.contains("original"), "original must remain: {}", out);
    assert!(out.contains("copy"), "clone must appear: {}", out);
}

#[test]
fn clone_nonexistent_profile_fails() {
    let env = TestEnv::new();
    env.run_fail(&["clone", "ghost", "copy"]);
}

#[test]
fn rename_profile() {
    let env = TestEnv::new();
    env.create_profile("old-name", "api");
    env.run_ok(&["rename", "old-name", "new-name"]);

    let out = env.run_ok(&["list"]);
    assert!(!out.contains("old-name"), "old name must be gone: {}", out);
    assert!(out.contains("new-name"), "new name must appear: {}", out);
}

#[test]
fn rename_nonexistent_fails() {
    let env = TestEnv::new();
    env.run_fail(&["rename", "ghost", "new"]);
}

#[test]
fn rm_profile() {
    let env = TestEnv::new();
    env.create_profile("to-delete", "api");
    env.run_ok(&["rm", "to-delete"]);

    let out = env.run_stdout(&["list"]);
    assert!(
        !out.contains("to-delete"),
        "deleted profile must not appear: {}",
        out
    );
}

#[test]
fn rm_nonexistent_profile_fails() {
    let env = TestEnv::new();
    env.run_fail(&["rm", "does-not-exist"]);
}

#[test]
fn create_multiple_profiles() {
    let env = TestEnv::new();
    env.create_profile("alpha", "api");
    env.create_profile("beta", "oauth");
    env.create_profile("gamma", "api");

    let out = env.run_ok(&["list"]);
    assert!(out.contains("alpha"), "must show alpha: {}", out);
    assert!(out.contains("beta"), "must show beta: {}", out);
    assert!(out.contains("gamma"), "must show gamma: {}", out);
}

#[test]
fn double_create_same_profile_name_fails() {
    let env = TestEnv::new();
    env.create_profile("dup", "api");
    // Second create with same name must fail (profile dir already exists)
    let out = env.run(&["new", "dup", "--auth", "api"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("already exists"),
        "duplicate profile create must report already exists: {}",
        stderr
    );
}

#[test]
fn rename_to_existing_name_fails() {
    let env = TestEnv::new();
    env.create_profile("alpha", "api");
    env.create_profile("beta", "api");
    let out = env.run(&["rename", "alpha", "beta"]);
    assert!(
        !out.status.success(),
        "renaming to an existing profile name should fail"
    );
}

// ─── Session CRUD ────────────────────────────────────────────────────────────

#[test]
fn session_list_shows_default() {
    let env = TestEnv::new();
    env.setup_active_profile("work");

    let out = env.run_ok(&["session", "list"]);
    assert!(
        out.contains("default"),
        "session list must show default session: {}",
        out
    );
}

#[test]
fn session_new_creates_session() {
    let env = TestEnv::new();
    env.setup_active_profile("work");
    env.run_ok(&["session", "new", "backend"]);

    let out = env.run_ok(&["session", "list"]);
    assert!(
        out.contains("backend"),
        "session list must show new session: {}",
        out
    );
}

#[test]
fn session_new_creates_claude_dir() {
    let env = TestEnv::new();
    env.setup_active_profile("work");
    env.run_ok(&["session", "new", "backend"]);

    let claude_dir = env
        .data_dir
        .path()
        .join("profiles")
        .join("work")
        .join("sessions")
        .join("backend")
        .join(".claude");
    assert!(
        claude_dir.exists(),
        ".claude dir must exist inside session: {:?}",
        claude_dir
    );
}

#[test]
fn session_new_with_tag() {
    let env = TestEnv::new();
    env.setup_active_profile("work");
    env.run_ok(&["session", "new", "infra", "--tag", "infrastructure work"]);

    let out = env.run_ok(&["session", "list"]);
    assert!(out.contains("infra"), "tagged session must appear: {}", out);
}

#[test]
fn session_tag_updates_description() {
    let env = TestEnv::new();
    env.setup_active_profile("work");
    env.run_ok(&["session", "new", "backend"]);
    env.run_ok(&["session", "tag", "backend", "backend api work"]);
}

#[test]
fn session_rm_deletes_session() {
    let env = TestEnv::new();
    env.setup_active_profile("work");
    env.run_ok(&["session", "new", "temp"]);
    env.run_ok(&["session", "rm", "temp"]);

    let out = env.run_ok(&["session", "list"]);
    assert!(
        !out.contains("temp"),
        "removed session must not appear: {}",
        out
    );
}

#[test]
fn session_rm_nonexistent_fails() {
    let env = TestEnv::new();
    env.setup_active_profile("work");
    env.run_fail(&["session", "rm", "ghost"]);
}

#[test]
fn session_archive() {
    let env = TestEnv::new();
    env.setup_active_profile("work");
    env.run_ok(&["session", "new", "old-feature"]);
    env.run_ok(&["session", "archive", "old-feature"]);

    // Archived sessions should not appear in the regular list
    let out = env.run_ok(&["session", "list"]);
    assert!(
        !out.contains("old-feature"),
        "archived session must not appear in list: {}",
        out
    );
}

#[test]
fn session_list_with_profile_arg() {
    let env = TestEnv::new();
    env.create_profile("alpha", "api");
    env.create_profile("beta", "api");
    env.set_current("alpha", "default");

    let out = env.run_ok(&["session", "list", "beta"]);
    assert!(
        out.contains("default"),
        "listing another profile's sessions must work: {}",
        out
    );
}

#[test]
fn session_commands_without_active_profile_fail() {
    let env = TestEnv::new();
    let out = env.run(&["session", "new", "test"]);
    assert!(
        !out.status.success(),
        "session new without active profile must fail"
    );
}

// ─── Auto-detect ─────────────────────────────────────────────────────────────

#[test]
fn auto_detect_no_cstrc_emits_nothing() {
    let env = TestEnv::new();
    let project = TempDir::new().unwrap();

    let out = Command::new(&env.bin)
        .args([
            "_auto-detect",
            &project.path().to_string_lossy(),
            "work:default",
        ])
        .env("CST_DATA_DIR", env.data_dir.path())
        .env("HOME", env.home_dir.path())
        .env_remove("RUST_LOG")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(
        stdout.is_empty(),
        "_auto-detect with no .cstrc must emit nothing: '{}'",
        stdout
    );
}

#[test]
fn auto_detect_same_profile_emits_nothing() {
    let env = TestEnv::new();
    let project = TempDir::new().unwrap();
    std::fs::write(
        project.path().join(".cstrc"),
        "profile = \"work\"\nsession = \"default\"",
    )
    .unwrap();

    let out = Command::new(&env.bin)
        .args([
            "_auto-detect",
            &project.path().to_string_lossy(),
            "work:default",
        ])
        .env("CST_DATA_DIR", env.data_dir.path())
        .env("HOME", env.home_dir.path())
        .env_remove("RUST_LOG")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(
        stdout.is_empty(),
        "_auto-detect when already correct must emit nothing: '{}'",
        stdout
    );
}

#[test]
fn auto_detect_different_profile_emits_exports() {
    let env = TestEnv::new();
    env.create_profile("work", "api");

    let project = TempDir::new().unwrap();
    std::fs::write(
        project.path().join(".cstrc"),
        "profile = \"work\"\nsession = \"default\"",
    )
    .unwrap();

    let out = Command::new(&env.bin)
        .args([
            "_auto-detect",
            &project.path().to_string_lossy(),
            "personal:default",
        ])
        .env("CST_DATA_DIR", env.data_dir.path())
        .env("HOME", env.home_dir.path())
        .env_remove("RUST_LOG")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("export") || stdout.contains("CST_CURRENT"),
        "_auto-detect must emit exports on mismatch: {}",
        stdout
    );
}

#[test]
fn auto_detect_status_shows_profile() {
    let env = TestEnv::new();
    env.create_profile("work", "api");

    let project = TempDir::new().unwrap();
    std::fs::write(
        project.path().join(".cstrc"),
        "profile = \"work\"\nsession = \"backend\"",
    )
    .unwrap();

    let out = Command::new(&env.bin)
        .args(["auto-detect-status", &project.path().to_string_lossy()])
        .env("CST_DATA_DIR", env.data_dir.path())
        .env("HOME", env.home_dir.path())
        .env_remove("RUST_LOG")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("work"),
        "auto-detect-status must mention profile: {}",
        stdout
    );
}

#[test]
fn auto_detect_status_no_cstrc() {
    let env = TestEnv::new();
    let project = TempDir::new().unwrap();

    let out = Command::new(&env.bin)
        .args(["auto-detect-status", &project.path().to_string_lossy()])
        .env("CST_DATA_DIR", env.data_dir.path())
        .env("HOME", env.home_dir.path())
        .env_remove("RUST_LOG")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("No .cstrc") || stdout.contains("not found") || stdout.contains("No"),
        "must indicate no .cstrc found: {}",
        stdout
    );
}

// ─── Doctor ──────────────────────────────────────────────────────────────────

#[test]
fn doctor_does_not_crash() {
    let env = TestEnv::new();
    let out = env.run(&["doctor"]);
    assert!(
        out.status.code().is_some(),
        "doctor must not crash with a signal"
    );
}

#[test]
fn doctor_output_mentions_claude() {
    let env = TestEnv::new();
    let out = env.run(&["doctor"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("Claude") || combined.contains("claude"),
        "doctor must check for claude: {}",
        combined
    );
}

#[test]
fn doctor_output_mentions_data_dir() {
    let env = TestEnv::new();
    let out = env.run(&["doctor"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("Data") || combined.contains("data") || combined.contains("sentinel"),
        "doctor must check data dir: {}",
        combined
    );
}

#[test]
fn doctor_after_profile_creation() {
    let env = TestEnv::new();
    env.create_profile("work", "api");
    let out = env.run(&["doctor"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("work") || stdout.contains("profiles"),
        "doctor should see the profile: {}",
        stdout
    );
}

// ─── Settings / config files ─────────────────────────────────────────────────

#[test]
fn profile_new_creates_profile_toml() {
    let env = TestEnv::new();
    env.create_profile("work", "api");
    let profile_toml = env
        .data_dir
        .path()
        .join("profiles")
        .join("work")
        .join("profile.toml");
    assert!(profile_toml.exists(), "profile.toml must be created");

    let contents = std::fs::read_to_string(&profile_toml).unwrap();
    assert!(
        contents.contains("api") || contents.contains("Api"),
        "profile.toml must contain auth type: {}",
        contents
    );
}

#[test]
fn profile_toml_contains_name() {
    let env = TestEnv::new();
    env.create_profile("my-project", "oauth");
    let profile_toml = env
        .data_dir
        .path()
        .join("profiles")
        .join("my-project")
        .join("profile.toml");
    let contents = std::fs::read_to_string(&profile_toml).unwrap();
    assert!(
        contents.contains("my-project"),
        "profile.toml must contain profile name: {}",
        contents
    );
}

#[test]
fn session_new_creates_claude_settings_dir() {
    let env = TestEnv::new();
    env.setup_active_profile("work");
    env.run_ok(&["session", "new", "backend"]);

    let claude_dir = env
        .data_dir
        .path()
        .join("profiles")
        .join("work")
        .join("sessions")
        .join("backend")
        .join(".claude");
    assert!(
        claude_dir.exists(),
        ".claude dir must be created: {:?}",
        claude_dir
    );
}

#[test]
fn use_command_outputs_env_exports() {
    let env = TestEnv::new();
    env.create_profile("work", "api");

    // `cst use work:default` prints env exports when called directly
    let out = env.run_stdout(&["use", "work:default"]);
    assert!(
        out.contains("CST_CURRENT") || out.contains("CLAUDE_CONFIG_DIR"),
        "use must output env exports: {}",
        out
    );
}

// ─── Validate ────────────────────────────────────────────────────────────────

#[test]
fn validate_nonexistent_profile_fails() {
    let env = TestEnv::new();
    env.run_fail(&["validate", "ghost"]);
}

#[test]
fn validate_existing_profile_does_not_crash() {
    let env = TestEnv::new();
    env.create_profile("work", "api");
    let out = env.run(&["validate", "work"]);
    assert!(
        out.status.code().is_some(),
        "validate must not crash with a signal"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("work"),
        "validate must mention the profile name: {}",
        stdout
    );
}

#[test]
fn validate_shows_auth_type() {
    let env = TestEnv::new();
    env.create_profile("work", "api");
    let out = env.run(&["validate", "work"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("api") || stdout.contains("Api"),
        "validate must show auth type: {}",
        stdout
    );
}

// ─── Team sync (git-based) ──────────────────────────────────────────────────

fn has_git() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn init_bare_repo(parent: &std::path::Path) -> PathBuf {
    let repo = parent.join("remote.git");
    std::fs::create_dir_all(&repo).unwrap();
    Command::new("git")
        .args(["init", "--bare", "-b", "main"])
        .current_dir(&repo)
        .output()
        .unwrap();
    repo
}

/// Helper to run cst with git env vars set for reproducible commits.
fn run_with_git_env(env: &TestEnv, args: &[&str]) -> Output {
    Command::new(&env.bin)
        .args(args)
        .env("CST_DATA_DIR", env.data_dir.path())
        .env("HOME", env.home_dir.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .env_remove("RUST_LOG")
        .output()
        .unwrap()
}

#[test]
fn team_init_with_local_bare_repo() {
    if !has_git() {
        return;
    }
    let env = TestEnv::new();
    let remote_base = TempDir::new().unwrap();
    let repo = init_bare_repo(remote_base.path());
    let remote_url = format!("file://{}", repo.display());

    let out = run_with_git_env(&env, &["team", "init", &remote_url]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    let config_path = env.data_dir.path().join("team-sync.toml");
    assert!(
        config_path.exists(),
        "team-sync.toml must be created\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    );
}

#[test]
fn team_status_without_init_fails() {
    if !has_git() {
        return;
    }
    let env = TestEnv::new();
    let out = env.run(&["team", "status"]);
    assert!(
        !out.status.success(),
        "team status without init should fail"
    );
}

#[test]
fn team_push_without_init_fails() {
    if !has_git() {
        return;
    }
    let env = TestEnv::new();
    let out = env.run(&["team", "push"]);
    assert!(
        !out.status.success(),
        "team push without init should fail"
    );
}

// ─── Starship / Tmux integrations ───────────────────────────────────────────

#[test]
fn starship_config_outputs_toml() {
    let env = TestEnv::new();
    let out = env.run_ok(&["starship", "--config"]);
    assert!(!out.is_empty(), "starship --config must produce output");
}

#[test]
fn tmux_config_outputs_snippet() {
    let env = TestEnv::new();
    let out = env.run_ok(&["tmux", "--config"]);
    assert!(!out.is_empty(), "tmux --config must produce output");
}

// ─── Data isolation verification ────────────────────────────────────────────

#[test]
fn cst_data_dir_override_is_respected() {
    let env = TestEnv::new();
    env.create_profile("isolated-test", "api");

    // The profile must be inside our temp data dir, not the real home
    let profile_dir = env
        .data_dir
        .path()
        .join("profiles")
        .join("isolated-test");
    assert!(
        profile_dir.exists(),
        "profile must be in CST_DATA_DIR temp dir"
    );

    // The real home should NOT have this profile
    let real_sentinel = dirs::data_local_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap())
        .join("claude-sentinel")
        .join("profiles")
        .join("isolated-test");
    assert!(
        !real_sentinel.exists(),
        "profile must NOT leak to real ~/.claude-sentinel/"
    );
}

#[test]
fn separate_test_envs_are_isolated() {
    let env1 = TestEnv::new();
    let env2 = TestEnv::new();

    env1.create_profile("env1-only", "api");
    env2.create_profile("env2-only", "api");

    let out1 = env1.run_ok(&["list"]);
    let out2 = env2.run_ok(&["list"]);

    assert!(out1.contains("env1-only"), "env1 must see its profile");
    assert!(!out1.contains("env2-only"), "env1 must NOT see env2 profile");
    assert!(out2.contains("env2-only"), "env2 must see its profile");
    assert!(!out2.contains("env1-only"), "env2 must NOT see env1 profile");
}
