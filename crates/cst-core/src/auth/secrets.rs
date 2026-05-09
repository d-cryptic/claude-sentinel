//! External secret providers for API key resolution.
//!
//! Supports retrieving API keys from:
//! - **macOS Keychain / libsecret / WinCred** (default, via `keyring` crate)
//! - **1Password CLI** (`op read "op://vault/item/field"`)
//! - **Doppler CLI** (`doppler secrets get KEY --plain`)
//! - **Environment variable** (`$MY_API_KEY`)
//!
//! A profile can pin any slot to any provider; the retrieval path is
//! transparent to the rest of cst-core.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;

/// Where to retrieve a secret from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum SecretSource {
    /// System credential store (macOS Keychain / libsecret / WinCred).
    /// `account` is the service account name used with the `keyring` crate.
    Keychain { account: String },

    /// 1Password CLI (`op` must be in PATH and signed in).
    ///
    /// `reference` is a full `op://` URI, e.g.:
    /// `op://Personal/Claude API Key/credential`
    OnePassword { reference: String },

    /// Doppler CLI (`doppler` must be in PATH and project configured).
    ///
    /// `secret_name` is the Doppler secret name, e.g. `ANTHROPIC_API_KEY`.
    /// Optional `project` and `config` override the Doppler context.
    Doppler {
        secret_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        project: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        config: Option<String>,
    },

    /// Plain environment variable.
    ///
    /// `var_name` is the variable to read, e.g. `MY_ANTHROPIC_KEY`.
    /// Useful for CI or when secrets are injected by another tool.
    EnvVar { var_name: String },
}

impl SecretSource {
    /// Retrieve the secret value from the configured provider.
    pub fn retrieve(&self) -> Result<String> {
        match self {
            SecretSource::Keychain { account } => retrieve_keychain(account),
            SecretSource::OnePassword { reference } => retrieve_1password(reference),
            SecretSource::Doppler {
                secret_name,
                project,
                config,
            } => retrieve_doppler(secret_name, project.as_deref(), config.as_deref()),
            SecretSource::EnvVar { var_name } => retrieve_env_var(var_name),
        }
    }

    /// Human-readable description for `cst validate` output.
    pub fn describe(&self) -> String {
        match self {
            SecretSource::Keychain { account } => format!("keychain:{}", account),
            SecretSource::OnePassword { reference } => format!("1password:{}", reference),
            SecretSource::Doppler {
                secret_name,
                project,
                config,
            } => {
                let mut s = format!("doppler:{}", secret_name);
                if let Some(p) = project {
                    s.push_str(&format!(" (project={})", p));
                }
                if let Some(c) = config {
                    s.push_str(&format!(" config={}", c));
                }
                s
            }
            SecretSource::EnvVar { var_name } => format!("env:${}", var_name),
        }
    }

    /// Check whether the required CLI tool is available.
    pub fn check_tool_available(&self) -> Result<()> {
        match self {
            SecretSource::OnePassword { .. } => {
                which::which("op")
                    .context("1Password CLI `op` not found in PATH — install from https://1password.com/downloads/command-line/")?;
                Ok(())
            }
            SecretSource::Doppler { .. } => {
                which::which("doppler")
                    .context("Doppler CLI not found in PATH — install from https://docs.doppler.com/docs/install-cli")?;
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

// ─── Provider implementations ────────────────────────────────────────────────

fn retrieve_keychain(account: &str) -> Result<String> {
    let entry =
        keyring::Entry::new("claude-sentinel", account).context("creating keychain entry")?;
    entry
        .get_password()
        .context("retrieving key from keychain — run `cst add-key` to populate")
}

fn retrieve_1password(reference: &str) -> Result<String> {
    let out = Command::new("op")
        .args(["read", "--no-newline", reference])
        .output()
        .context("running `op read` — is 1Password CLI installed and signed in?")?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("op read failed: {}", stderr.trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn retrieve_doppler(
    secret_name: &str,
    project: Option<&str>,
    config: Option<&str>,
) -> Result<String> {
    let mut cmd = Command::new("doppler");
    cmd.args(["secrets", "get", secret_name, "--plain"]);
    if let Some(p) = project {
        cmd.args(["--project", p]);
    }
    if let Some(c) = config {
        cmd.args(["--config", c]);
    }

    let out = cmd
        .output()
        .context("running `doppler secrets get` — is Doppler CLI installed and configured?")?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("doppler secrets get failed: {}", stderr.trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn retrieve_env_var(var_name: &str) -> Result<String> {
    std::env::var(var_name).with_context(|| format!("environment variable ${} not set", var_name))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // std::env::set_var / remove_var are not thread-safe in multi-threaded
    // processes (UB since Rust 1.81). Serialise all env-mutating tests with
    // this process-wide lock so they cannot race with each other or with
    // platform::tests::test_data_dir_respects_env_override.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn env_var_source_reads_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: we hold ENV_LOCK, so no other test is mutating the environment.
        unsafe { std::env::set_var("_CST_TEST_KEY", "sk-ant-test") }
        let src = SecretSource::EnvVar {
            var_name: "_CST_TEST_KEY".to_string(),
        };
        let result = src.retrieve();
        unsafe { std::env::remove_var("_CST_TEST_KEY") }
        assert_eq!(result.unwrap(), "sk-ant-test");
    }

    #[test]
    fn env_var_source_missing_errors() {
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: we hold ENV_LOCK.
        unsafe { std::env::remove_var("_CST_DEFINITELY_MISSING") }
        let src = SecretSource::EnvVar {
            var_name: "_CST_DEFINITELY_MISSING".to_string(),
        };
        assert!(src.retrieve().is_err());
    }

    #[test]
    fn describe_keychain() {
        let s = SecretSource::Keychain {
            account: "work-slot1".to_string(),
        };
        assert_eq!(s.describe(), "keychain:work-slot1");
    }

    #[test]
    fn describe_1password() {
        let s = SecretSource::OnePassword {
            reference: "op://Personal/Claude/cred".to_string(),
        };
        assert!(s.describe().starts_with("1password:"));
    }

    #[test]
    fn describe_doppler() {
        let s = SecretSource::Doppler {
            secret_name: "ANTHROPIC_KEY".to_string(),
            project: Some("myapp".to_string()),
            config: None,
        };
        assert!(s.describe().contains("doppler:ANTHROPIC_KEY"));
        assert!(s.describe().contains("project=myapp"));
    }

    #[test]
    fn describe_env_var() {
        let s = SecretSource::EnvVar {
            var_name: "MY_KEY".to_string(),
        };
        assert_eq!(s.describe(), "env:$MY_KEY");
    }

    #[test]
    fn serde_roundtrip_keychain() {
        let s = SecretSource::Keychain {
            account: "work-slot1".to_string(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: SecretSource = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn serde_roundtrip_1password() {
        let s = SecretSource::OnePassword {
            reference: "op://P/I/F".to_string(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: SecretSource = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn serde_roundtrip_doppler() {
        let s = SecretSource::Doppler {
            secret_name: "KEY".to_string(),
            project: None,
            config: Some("prd".to_string()),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: SecretSource = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn serde_roundtrip_env_var() {
        let s = SecretSource::EnvVar {
            var_name: "MY_KEY".to_string(),
        };
        let toml = toml::to_string(&s).unwrap();
        let back: SecretSource = toml::from_str(&toml).unwrap();
        assert_eq!(s, back);
    }

    // ── describe security ─────────────────────────────────────────────────

    #[test]
    fn describe_1password_contains_reference_not_secret() {
        // describe() must show the REFERENCE (vault path), not a retrieved value
        let reference = "op://Personal/Claude/credential";
        let s = SecretSource::OnePassword {
            reference: reference.to_string(),
        };
        let desc = s.describe();
        assert!(
            desc.contains(reference),
            "describe should include the op:// reference for diagnostics"
        );
        assert!(
            !desc.contains("sk-ant"),
            "describe must never embed an actual secret"
        );
    }

    #[test]
    fn describe_doppler_no_project_no_config() {
        let s = SecretSource::Doppler {
            secret_name: "ANTHROPIC_KEY".to_string(),
            project: None,
            config: None,
        };
        let desc = s.describe();
        assert!(desc.contains("ANTHROPIC_KEY"));
        // No extra cruft when no project/config
        assert!(!desc.contains("project="));
        assert!(!desc.contains("config="));
    }

    #[test]
    fn describe_doppler_with_both_project_and_config() {
        let s = SecretSource::Doppler {
            secret_name: "KEY".to_string(),
            project: Some("myapp".to_string()),
            config: Some("prd".to_string()),
        };
        let desc = s.describe();
        assert!(desc.contains("project=myapp"));
        assert!(desc.contains("config=prd"));
    }

    // ── env var edge cases ────────────────────────────────────────────────

    #[test]
    fn env_var_empty_value_succeeds() {
        let _guard = ENV_LOCK.lock().unwrap();
        // An env var set to "" is present and retrievable (empty string is valid)
        // SAFETY: we hold ENV_LOCK.
        unsafe { std::env::set_var("_CST_TEST_EMPTY", "") }
        let src = SecretSource::EnvVar {
            var_name: "_CST_TEST_EMPTY".to_string(),
        };
        let result = src.retrieve();
        unsafe { std::env::remove_var("_CST_TEST_EMPTY") }
        assert_eq!(result.unwrap(), "");
    }

    // ── check_tool_available ──────────────────────────────────────────────

    #[test]
    fn check_tool_available_keychain_always_ok() {
        let s = SecretSource::Keychain {
            account: "test".to_string(),
        };
        // Keychain provider needs no external CLI
        assert!(s.check_tool_available().is_ok());
    }

    #[test]
    fn check_tool_available_env_var_always_ok() {
        let s = SecretSource::EnvVar {
            var_name: "ANYTHING".to_string(),
        };
        assert!(s.check_tool_available().is_ok());
    }

    // ── serde tag field ───────────────────────────────────────────────────

    #[test]
    fn serde_json_tag_field_is_provider() {
        let s = SecretSource::Keychain {
            account: "acc".to_string(),
        };
        let json = serde_json::to_string(&s).unwrap();
        // The discriminant tag must be "provider" per #[serde(tag = "provider")]
        assert!(json.contains("\"provider\""));
        assert!(json.contains("\"keychain\""));
    }

    #[test]
    fn serde_roundtrip_doppler_with_options() {
        let s = SecretSource::Doppler {
            secret_name: "MY_SECRET".to_string(),
            project: Some("proj".to_string()),
            config: Some("dev".to_string()),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: SecretSource = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
