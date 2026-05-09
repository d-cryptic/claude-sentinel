//! Persistent auto-switch event log — records every profile switch with reason + timestamp.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::PathBuf;

use crate::platform;

/// Why a profile switch occurred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwitchReason {
    /// User explicitly ran `cst use`.
    Manual,
    /// Daemon detected a rate limit.
    RateLimit,
    /// Quota estimated to have refilled → switched back.
    QuotaRefill,
    /// Time-based schedule triggered the switch.
    Schedule,
    /// `.cstrc` auto-detect in project directory.
    AutoDetect,
}

impl std::fmt::Display for SwitchReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwitchReason::Manual => write!(f, "manual"),
            SwitchReason::RateLimit => write!(f, "rate limit"),
            SwitchReason::QuotaRefill => write!(f, "quota refill"),
            SwitchReason::Schedule => write!(f, "schedule"),
            SwitchReason::AutoDetect => write!(f, "auto-detect"),
        }
    }
}

/// A single switch event entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchEvent {
    pub timestamp: DateTime<Utc>,
    pub from_profile: String,
    pub from_session: String,
    pub to_profile: String,
    pub to_session: String,
    pub reason: SwitchReason,
    /// Extra detail (e.g. "HTTP 429 – too many requests").
    #[serde(default)]
    pub detail: String,
}

/// Append-only JSONL log at `~/.claude-sentinel/switch-log.jsonl`.
pub struct SwitchLog {
    path: PathBuf,
}

impl SwitchLog {
    pub fn open() -> Self {
        Self {
            path: platform::data_dir().join("switch-log.jsonl"),
        }
    }

    /// Append one event to the log.
    pub fn append(&self, event: &SwitchEvent) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{}", serde_json::to_string(event)?)?;
        Ok(())
    }

    /// Read all events (oldest first).
    pub fn read_all(&self) -> Result<Vec<SwitchEvent>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let file = std::fs::File::open(&self.path)?;
        let reader = std::io::BufReader::new(file);
        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<SwitchEvent>(&line) {
                Ok(ev) => events.push(ev),
                Err(e) => tracing::debug!("skipping malformed switch-log line: {e}"),
            }
        }
        Ok(events)
    }

    /// Return the last N events without loading the entire file into memory.
    ///
    /// Reads only the last `READ_TAIL_BYTES` of the log, which covers several
    /// hundred events at typical JSON sizes. Falls back to `read_all` only when
    /// the file is smaller than the tail window.
    pub fn last_n(&self, n: usize) -> Result<Vec<SwitchEvent>> {
        use std::io::{Read, Seek, SeekFrom};
        const READ_TAIL_BYTES: u64 = 64 * 1024; // 64 KiB ≈ ~300 events at ~200 bytes each

        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let mut f = std::fs::File::open(&self.path)?;
        let len = f.metadata()?.len();
        let start = len.saturating_sub(READ_TAIL_BYTES);
        f.seek(SeekFrom::Start(start))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        let tail = String::from_utf8_lossy(&buf);

        let mut events: Vec<SwitchEvent> = tail
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();

        // If we may have cut the file mid-way, we might have more events than n;
        // drop the excess from the front to keep the last n in chronological order.
        if events.len() > n {
            events.drain(..events.len() - n);
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_event(from: &str, to: &str, reason: SwitchReason) -> SwitchEvent {
        SwitchEvent {
            timestamp: Utc::now(),
            from_profile: from.to_string(),
            from_session: "default".to_string(),
            to_profile: to.to_string(),
            to_session: "default".to_string(),
            reason,
            detail: String::new(),
        }
    }

    #[test]
    fn test_append_and_read_roundtrip() {
        let dir = TempDir::new().unwrap();
        let log = SwitchLog {
            path: dir.path().join("switch-log.jsonl"),
        };
        log.append(&make_event("work", "personal", SwitchReason::RateLimit))
            .unwrap();
        log.append(&make_event("personal", "work", SwitchReason::QuotaRefill))
            .unwrap();

        let events = log.read_all().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].from_profile, "work");
        assert_eq!(events[0].reason, SwitchReason::RateLimit);
        assert_eq!(events[1].reason, SwitchReason::QuotaRefill);
    }

    #[test]
    fn test_read_empty_log() {
        let dir = TempDir::new().unwrap();
        let log = SwitchLog {
            path: dir.path().join("switch-log.jsonl"),
        };
        assert!(log.read_all().unwrap().is_empty());
    }

    #[test]
    fn test_last_n() {
        let dir = TempDir::new().unwrap();
        let log = SwitchLog {
            path: dir.path().join("switch-log.jsonl"),
        };
        for i in 0..5 {
            log.append(&make_event(
                &format!("p{i}"),
                "personal",
                SwitchReason::Manual,
            ))
            .unwrap();
        }
        let last2 = log.last_n(2).unwrap();
        assert_eq!(last2.len(), 2);
        assert_eq!(last2[0].from_profile, "p3");
        assert_eq!(last2[1].from_profile, "p4");
    }

    #[test]
    fn test_reason_display() {
        assert_eq!(SwitchReason::RateLimit.to_string(), "rate limit");
        assert_eq!(SwitchReason::Manual.to_string(), "manual");
    }
}
