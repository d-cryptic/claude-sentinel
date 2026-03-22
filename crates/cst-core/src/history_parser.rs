//! Parse Claude Code's `history.jsonl` for live token counts.
//!
//! Claude Code writes a JSONL file where each line is a JSON event.
//! API response events carry `usage` objects with `input_tokens` /
//! `output_tokens` (and optional cache fields). We scan all lines and sum them.

use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

/// Aggregated token counts from a `history.jsonl` file.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HistoryTokens {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
}

impl HistoryTokens {
    /// Total tokens (input + output).
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }

    /// Estimated cost in USD (Claude Sonnet 4.x pricing as baseline).
    ///
    /// Input:  $3.00 / MTok
    /// Output: $15.00 / MTok
    /// Cache creation: $3.75 / MTok
    /// Cache read: $0.30 / MTok
    pub fn estimated_cost_usd(&self) -> f64 {
        let input = self.input_tokens as f64 * 3.0 / 1_000_000.0;
        let output = self.output_tokens as f64 * 15.0 / 1_000_000.0;
        let cache_create = self.cache_creation_tokens as f64 * 3.75 / 1_000_000.0;
        let cache_read = self.cache_read_tokens as f64 * 0.30 / 1_000_000.0;
        input + output + cache_create + cache_read
    }
}

/// Parse a `history.jsonl` file and return summed token usage.
///
/// Invalid or non-usage lines are silently skipped.
pub fn parse_tokens(path: &Path) -> Result<HistoryTokens> {
    let content = std::fs::read_to_string(path)?;
    Ok(sum_tokens(&content))
}

/// Parse `history.jsonl` content from a string (exposed for testing).
pub fn sum_tokens(jsonl: &str) -> HistoryTokens {
    let mut totals = HistoryTokens::default();
    for line in jsonl.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<HistoryEntry>(line) {
            accumulate(&mut totals, entry.usage.as_ref());
            if let Some(msg) = &entry.message {
                accumulate(&mut totals, msg.usage.as_ref());
            }
        }
    }
    totals
}

fn accumulate(totals: &mut HistoryTokens, usage: Option<&Usage>) {
    let Some(u) = usage else { return };
    totals.input_tokens += u.input_tokens.unwrap_or(0);
    totals.output_tokens += u.output_tokens.unwrap_or(0);
    totals.cache_creation_tokens += u.cache_creation_input_tokens.unwrap_or(0);
    totals.cache_read_tokens += u.cache_read_input_tokens.unwrap_or(0);
}

// ─── Internal deserialization types ─────────────────────────────────────────

#[derive(Deserialize)]
struct HistoryEntry {
    usage: Option<Usage>,
    message: Option<NestedMessage>,
}

#[derive(Deserialize)]
struct NestedMessage {
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_returns_zeros() {
        assert_eq!(sum_tokens(""), HistoryTokens::default());
    }

    #[test]
    fn blank_lines_ignored() {
        let jsonl = "\n\n  \n";
        assert_eq!(sum_tokens(jsonl), HistoryTokens::default());
    }

    #[test]
    fn invalid_lines_skipped() {
        let jsonl = "not json\n{\"usage\":{\"input_tokens\":42,\"output_tokens\":7}}\nnot json";
        let r = sum_tokens(jsonl);
        assert_eq!(r.input_tokens, 42);
        assert_eq!(r.output_tokens, 7);
    }

    #[test]
    fn top_level_usage() {
        let jsonl = r#"{"type":"assistant","usage":{"input_tokens":100,"output_tokens":50}}"#;
        let r = sum_tokens(jsonl);
        assert_eq!(r.input_tokens, 100);
        assert_eq!(r.output_tokens, 50);
    }

    #[test]
    fn nested_message_usage() {
        let jsonl = r#"{"type":"api_response","message":{"usage":{"input_tokens":200,"output_tokens":80}}}"#;
        let r = sum_tokens(jsonl);
        assert_eq!(r.input_tokens, 200);
        assert_eq!(r.output_tokens, 80);
    }

    #[test]
    fn multiple_lines_summed() {
        let jsonl = [
            r#"{"usage":{"input_tokens":100,"output_tokens":50}}"#,
            r#"{"usage":{"input_tokens":200,"output_tokens":80}}"#,
            r#"{"type":"tool_use"}"#,
        ]
        .join("\n");
        let r = sum_tokens(&jsonl);
        assert_eq!(r.input_tokens, 300);
        assert_eq!(r.output_tokens, 130);
    }

    #[test]
    fn cache_tokens_accumulated() {
        let jsonl = r#"{"usage":{"input_tokens":10,"output_tokens":5,"cache_creation_input_tokens":100,"cache_read_input_tokens":200}}"#;
        let r = sum_tokens(jsonl);
        assert_eq!(r.cache_creation_tokens, 100);
        assert_eq!(r.cache_read_tokens, 200);
    }

    #[test]
    fn total_sums_in_and_out() {
        let t = HistoryTokens { input_tokens: 300, output_tokens: 130, ..Default::default() };
        assert_eq!(t.total(), 430);
    }

    #[test]
    fn cost_estimate_nonzero_for_nonzero_tokens() {
        let t = HistoryTokens { input_tokens: 10_000, output_tokens: 5_000, ..Default::default() };
        assert!(t.estimated_cost_usd() > 0.0);
    }

    #[test]
    fn parse_tokens_reads_file() {
        use tempfile::NamedTempFile;
        use std::io::Write;
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"usage":{{"input_tokens":42,"output_tokens":7}}}}"#).unwrap();
        let r = parse_tokens(f.path()).unwrap();
        assert_eq!(r.input_tokens, 42);
    }
}
