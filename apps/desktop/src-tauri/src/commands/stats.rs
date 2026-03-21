//! Stats Tauri commands.

use cst_core::platform;
use cst_core::profile::ProfileManager;
use cst_core::session::SessionManager;
use cst_core::stats::SessionStats;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct StatsDto {
    pub profile: String,
    pub session: String,
    pub session_count: u64,
    pub rate_limit_hits: u64,
    pub key_rotations: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub estimated_cost_usd: f64,
    pub first_used: Option<String>,
    pub last_used: Option<String>,
}

#[tauri::command]
pub fn get_stats(profile: Option<String>, session: Option<String>) -> Result<Vec<StatsDto>, String> {
    let mgr = ProfileManager::new(platform::profiles_dir());
    let profiles = mgr.list().map_err(|e| e.to_string())?;
    let mut result = Vec::new();

    for p in &profiles {
        if let Some(ref filter_p) = profile {
            if &p.name != filter_p { continue; }
        }
        let profile_dir = platform::profile_dir(&p.name);
        let smgr = SessionManager::new(profile_dir.join("sessions"));
        let sessions = smgr.list().unwrap_or_default();

        for s in &sessions {
            if let Some(ref filter_s) = session {
                if &s.name != filter_s { continue; }
            }
            let session_dir = profile_dir.join("sessions").join(&s.name);
            let stats = SessionStats::load(&session_dir).unwrap_or_default();
            result.push(StatsDto {
                profile: p.name.clone(),
                session: s.name.clone(),
                session_count: stats.session_count,
                rate_limit_hits: stats.rate_limit_hits,
                key_rotations: stats.key_rotations,
                tokens_in: stats.tokens_in,
                tokens_out: stats.tokens_out,
                estimated_cost_usd: stats.estimated_cost_usd,
                first_used: stats.first_used.map(|dt| dt.to_rfc3339()),
                last_used: stats.last_used.map(|dt| dt.to_rfc3339()),
            });
        }
    }

    Ok(result)
}
