//! `cst team` — git-based team profile sharing.

use anyhow::{bail, Result};
use cst_core::team_sync::{self, MergeStrategy};

pub fn init(remote_url: &str, branch: &str) -> Result<()> {
    team_sync::init(remote_url, branch)?;
    println!();
    println!("Next steps:");
    println!("  cst team push     # upload your profiles");
    println!("  cst team pull     # download team profiles on another machine");
    Ok(())
}

pub fn push() -> Result<()> {
    team_sync::push()
}

pub fn pull(strategy: Option<String>) -> Result<()> {
    let parsed = match strategy.as_deref() {
        None => None,
        Some("theirs") => Some(MergeStrategy::Theirs),
        Some("ours") => Some(MergeStrategy::Ours),
        Some("merge") => Some(MergeStrategy::Merge),
        Some(other) => bail!(
            "unknown merge strategy '{}' — expected: theirs, ours, merge",
            other
        ),
    };
    team_sync::pull_with_strategy(parsed)
}

pub fn status() -> Result<()> {
    team_sync::status()
}
