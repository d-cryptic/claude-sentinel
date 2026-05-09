//! Rate-limit pattern detection in Claude Code history / log lines.

/// Returns `true` if the given line contains a rate-limit signal.
///
/// Detection rules are intentionally specific to avoid false positives from
/// user code or content. The rules below require context around the signal:
///
/// - `"429"` alone is too broad (matches `status_code = 429` in user code).
///   We require it alongside `"too many"` or as a JSON field value.
/// - `"api_error"` alone is too broad. We require a rate-limit sub-type.
/// - All other patterns are multi-word and sufficiently specific.
pub fn is_rate_limit_line(line: &str) -> bool {
    let lower = line.to_lowercase();

    // Multi-word patterns that are specific enough on their own
    if lower.contains("rate limit")
        || lower.contains("rate_limit")
        || lower.contains("ratelimit")
        || lower.contains("quota exceeded")
        || lower.contains("quota_exceeded")
        || lower.contains("too many requests")
        || lower.contains("usage limit reached")
        || lower.contains("usage_limit_reached")
        || lower.contains("overloaded_error")
    {
        return true;
    }

    // "429" only when adjacent to "too many" (HTTP reason phrase) or as a JSON status value
    if lower.contains("429") && lower.contains("too many") {
        return true;
    }
    if lower.contains("\"status\":429")
        || lower.contains("\"status\": 429")
        || lower.contains("\"code\":429")
        || lower.contains("\"code\": 429")
        || lower.contains("http/1.1 429")
        || lower.contains("http/2 429")
    {
        return true;
    }

    // "api_error" only when accompanied by a specific rate-limit error sub-type.
    // Using overloaded_error (not bare "overloaded") to avoid matching user prose.
    if lower.contains("api_error")
        && (lower.contains("rate_limit")
            || lower.contains("quota")
            || lower.contains("429")
            || lower.contains("overloaded_error"))
    {
        return true;
    }

    false
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
        assert!(is_rate_limit_line(
            r#"{"error":"rate_limit","message":"..."}"#
        ));
        assert!(is_rate_limit_line("HTTP 429 Too Many Requests"));
        assert!(is_rate_limit_line("quota exceeded for this hour"));
        assert!(is_rate_limit_line("Usage limit reached for today"));
        assert!(is_rate_limit_line(r#"{"status":429,"error":"too many requests"}"#));
        assert!(is_rate_limit_line(r#"{"type":"api_error","error":{"type":"rate_limit_error"}}"#));
        assert!(is_rate_limit_line(r#"{"error":{"type":"overloaded_error"}}"#));
    }

    #[test]
    fn test_does_not_trigger_on_normal_lines() {
        assert!(!is_rate_limit_line("assistant: here is your code"));
        assert!(!is_rate_limit_line("function callTool() {}"));
        assert!(!is_rate_limit_line(""));
        assert!(!is_rate_limit_line("Error: file not found"));
    }

    #[test]
    fn test_no_false_positive_bare_429() {
        // Bare 429 in user code must not trigger
        assert!(!is_rate_limit_line("status_code = 429"));
        assert!(!is_rate_limit_line("let section_429 = data[429]"));
        assert!(!is_rate_limit_line("# handles 429 and other codes"));
        assert!(!is_rate_limit_line("error_code: 429"));
    }

    #[test]
    fn test_no_false_positive_api_error() {
        // Standalone api_error without rate-limit context must not trigger
        assert!(!is_rate_limit_line(r#"{"type":"api_error","message":"invalid request"}"#));
        assert!(!is_rate_limit_line("api_error: malformed JSON body"));
    }

    #[test]
    fn test_no_false_positive_overloaded_prose() {
        // Bare "overloaded" in prose must not trigger; only overloaded_error type does
        assert!(!is_rate_limit_line("the system is overloaded with requests"));
        assert!(!is_rate_limit_line("overloaded variable from the module"));
        // api_error + bare "overloaded" prose must also not trigger
        assert!(!is_rate_limit_line("api_error: the system is overloaded with requests"));
    }

    #[test]
    fn test_detects_http_status_line_429() {
        assert!(is_rate_limit_line("HTTP/1.1 429 Too Many Requests"));
        assert!(is_rate_limit_line("http/2 429"));
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
        let r = extract_reason(r#"{"error":{"type":"overloaded_error"}}"#);
        assert!(r.contains("overloaded"));
    }
}
