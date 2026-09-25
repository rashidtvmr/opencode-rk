#![forbid(unsafe_code)]
//! Short error display + retry hint.
//!
//! Mirrors TS truth: `packages/tui/src/util/error.ts` (`errorMessage`
//! first-nonempty extraction). `short_error` is the one-line display
//! shape; retry helpers classify transient provider/network failures.

/// First line of `msg`, trimmed, capped at 256 chars.
#[must_use]
pub fn short_error(msg: &str) -> String {
    let first = msg.lines().next().unwrap_or("").trim();
    if first.is_empty() {
        return "unknown error".to_string();
    }
    if first.chars().count() > 256 {
        first.chars().take(256).collect()
    } else {
        first.to_string()
    }
}

/// True for transient failures worth retrying.
#[must_use]
pub fn is_retryable(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    ["rate", "timeout", "timed out", "429", "503", "econn"]
        .iter()
        .any(|needle| lower.contains(needle))
}

/// `"retry"` when [`is_retryable`], else `"fix input"`.
#[must_use]
pub fn retry_hint(msg: &str) -> &'static str {
    if is_retryable(msg) {
        "retry"
    } else {
        "fix input"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_line_trimmed() {
        assert_eq!(short_error("boom\nsecond"), "boom");
        assert_eq!(short_error("  spaced  \nrest"), "spaced");
    }

    #[test]
    fn empty_falls_back() {
        assert_eq!(short_error(""), "unknown error");
        assert_eq!(short_error("  \n  "), "unknown error");
    }

    #[test]
    fn caps_at_256_chars() {
        let long = "x".repeat(300);
        assert_eq!(short_error(&long).chars().count(), 256);
    }

    #[test]
    fn retryable_patterns() {
        assert!(is_retryable("rate limit exceeded"));
        assert!(is_retryable("request timed out"));
        assert!(is_retryable("timeout waiting"));
        assert!(is_retryable("HTTP 429"));
        assert!(is_retryable("HTTP 503"));
        assert!(is_retryable("ECONNREFUSED"));
        assert!(!is_retryable("invalid API key"));
    }

    #[test]
    fn hint_branches() {
        assert_eq!(retry_hint("429 too many"), "retry");
        assert_eq!(retry_hint("bad input"), "fix input");
    }
}
