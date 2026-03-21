//! Rate-limit pattern detection in Claude Code history / log lines.

/// Patterns that indicate a rate-limit or quota-exceeded response.
const RATE_LIMIT_PATTERNS: &[&str] = &[
    "rate limit",
    "rate_limit",
    "ratelimit",
    "quota exceeded",
    "quota_exceeded",
    "too many requests",
    "429",
    "overloaded",
    "usage limit reached",
    "usage_limit_reached",
    "api_error",          // Claude Code wraps 429s as api_error in history
];

/// Returns `true` if the given line contains a rate-limit signal.
pub fn is_rate_limit_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    RATE_LIMIT_PATTERNS.iter().any(|p| lower.contains(p))
}

/// Extract a brief reason string from a matching line (for the switch log).
pub fn extract_reason(line: &str) -> String {
    let lower = line.to_lowercase();
    if lower.contains("429") || lower.contains("too many requests") {
        "HTTP 429 — too many requests".to_string()
    } else if lower.contains("quota") {
        "quota exceeded".to_string()
    } else if lower.contains("rate limit") || lower.contains("rate_limit") {
        "rate limit hit".to_string()
    } else if lower.contains("overloaded") {
        "API overloaded".to_string()
    } else {
        "rate limit detected".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_rate_limit_text() {
        assert!(is_rate_limit_line("Error: rate limit exceeded"));
        assert!(is_rate_limit_line(r#"{"error":"rate_limit","message":"..."}"#));
        assert!(is_rate_limit_line("HTTP 429 Too Many Requests"));
        assert!(is_rate_limit_line("quota exceeded for this hour"));
        assert!(is_rate_limit_line("Usage limit reached for today"));
    }

    #[test]
    fn test_does_not_trigger_on_normal_lines() {
        assert!(!is_rate_limit_line("assistant: here is your code"));
        assert!(!is_rate_limit_line("function callTool() {}"));
        assert!(!is_rate_limit_line(""));
        assert!(!is_rate_limit_line("Error: file not found"));
    }

    #[test]
    fn test_extract_reason_429() {
        let r = extract_reason("status: 429 too many requests");
        assert!(r.contains("429"));
    }

    #[test]
    fn test_extract_reason_quota() {
        let r = extract_reason("quota exceeded");
        assert!(r.contains("quota"));
    }

    #[test]
    fn test_extract_reason_fallback() {
        let r = extract_reason("overloaded right now");
        assert!(r.contains("overloaded"));
    }
}
