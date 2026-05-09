use anyhow::Result;
use cst_core::{platform, stats::SessionStats, validate_profile_name, validate_session_name};

pub fn run(profile_session: Option<&str>) -> Result<()> {
    let (profile, session) = match profile_session {
        Some(ps) => cst_core::shell::parse_profile_session(ps),
        None => {
            let cfg = cst_core::GlobalConfig::load()?;
            (cfg.current_profile, cfg.current_session)
        }
    };
    validate_profile_name(&profile)?;
    validate_session_name(&session)?;
    let session_dir = platform::session_dir(&profile, &session);
    let stats = SessionStats::load(&session_dir)?;
    println!("Profile : {profile}:{session}");
    println!("Sessions started   : {}", stats.session_count);
    println!("Rate limit hits    : {}", stats.rate_limit_hits);
    println!("Key rotations      : {}", stats.key_rotations);
    println!("Tokens in          : {}", stats.tokens_in);
    println!("Tokens out         : {}", stats.tokens_out);
    println!("Est. cost          : ${:.4}", stats.estimated_cost_usd);
    Ok(())
}
