use anyhow::Result;
use cst_core::{platform, shell::parse_profile_session, validate_profile_name, validate_session_name};

pub async fn run_with_profile(profile_session: &str, cmd: &[String]) -> Result<()> {
    let (profile, session) = parse_profile_session(profile_session);
    validate_profile_name(&profile)?;
    validate_session_name(&session)?;
    let config_dir = platform::claude_config_dir(&profile, &session);
    if cmd.is_empty() {
        anyhow::bail!("Usage: cst run <profile:session> -- <command> [args...]");
    }
    let status = tokio::process::Command::new(&cmd[0])
        .args(&cmd[1..])
        .env("CLAUDE_CONFIG_DIR", &config_dir)
        .env("CST_CURRENT", format!("{profile}:{session}"))
        .status()
        .await?;
    std::process::exit(status.code().unwrap_or(1));
}
