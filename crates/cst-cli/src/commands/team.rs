//! `cst team` — git-based team profile sharing.

use anyhow::Result;
use cst_core::team_sync;

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

pub fn pull() -> Result<()> {
    team_sync::pull()
}

pub fn status() -> Result<()> {
    team_sync::status()
}
