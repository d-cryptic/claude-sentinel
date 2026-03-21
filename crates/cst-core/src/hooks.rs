//! Pre/post profile-switch lifecycle hooks.
//!
//! Configured in `profiles/{p}/profile.toml` under `[hooks]`:
//! ```toml
//! [hooks]
//! pre_switch_in  = "echo 'Switching to work profile'"
//! post_switch_in = "~/scripts/notify-work.sh"
//! pre_switch_out = "cst sync"
//! post_switch_out = ""
//! ```

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::Command;

/// Profile-level lifecycle hooks.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileHooks {
    /// Run before switching *into* this profile.
    #[serde(default)]
    pub pre_switch_in: String,
    /// Run after switching *into* this profile.
    #[serde(default)]
    pub post_switch_in: String,
    /// Run before switching *away from* this profile.
    #[serde(default)]
    pub pre_switch_out: String,
    /// Run after switching *away from* this profile.
    #[serde(default)]
    pub post_switch_out: String,
}

impl ProfileHooks {
    /// Run `pre_switch_in` if non-empty. Errors are logged but not fatal.
    pub fn run_pre_switch_in(&self) -> Result<()> {
        run_hook("pre_switch_in", &self.pre_switch_in)
    }

    /// Run `post_switch_in` if non-empty.
    pub fn run_post_switch_in(&self) -> Result<()> {
        run_hook("post_switch_in", &self.post_switch_in)
    }

    /// Run `pre_switch_out` if non-empty.
    pub fn run_pre_switch_out(&self) -> Result<()> {
        run_hook("pre_switch_out", &self.pre_switch_out)
    }

    /// Run `post_switch_out` if non-empty.
    pub fn run_post_switch_out(&self) -> Result<()> {
        run_hook("post_switch_out", &self.post_switch_out)
    }
}

fn run_hook(name: &str, cmd: &str) -> Result<()> {
    if cmd.is_empty() {
        return Ok(());
    }
    tracing::debug!("Running hook {name}: {cmd}");

    #[cfg(unix)]
    let status = Command::new("sh").arg("-c").arg(cmd).status();

    #[cfg(windows)]
    let status = Command::new("cmd").args(["/C", cmd]).status();

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => {
            tracing::warn!("Hook {name} exited with status {s}: {cmd}");
            Ok(()) // hooks are non-fatal
        }
        Err(e) => {
            tracing::warn!("Hook {name} failed to spawn: {e}: {cmd}");
            Ok(()) // hooks are non-fatal
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_hook_is_noop() {
        let hooks = ProfileHooks::default();
        assert!(hooks.run_pre_switch_in().is_ok());
        assert!(hooks.run_post_switch_in().is_ok());
    }

    #[test]
    fn test_hook_with_true_command_succeeds() {
        let hooks = ProfileHooks {
            pre_switch_in: "true".to_string(),
            ..Default::default()
        };
        assert!(hooks.run_pre_switch_in().is_ok());
    }

    #[test]
    fn test_hook_with_failing_command_is_nonfatal() {
        // Hooks that fail should NOT propagate error
        let hooks = ProfileHooks {
            post_switch_in: "false".to_string(),
            ..Default::default()
        };
        assert!(hooks.run_post_switch_in().is_ok());
    }
}
