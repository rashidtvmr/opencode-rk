#![forbid(unsafe_code)]
//! Full-patch revert guards (std-only).
//!
//! Hand parser mirrors `util/revert-diff.ts:1-18` `parsePatch`
//! (`---`/`+++` headers, `@@` hunks); revert flows at
//! `route/session/index.tsx:613,644,660` call it before revert.
//! Fail-closed: bad counts/hunks reject, messages truncate char-safe.

/// Cap on files per revert patch.
pub const MAX_FILES: usize = 32;
/// Cap on message chars kept.
pub const MSG_MAX: usize = 512;

/// True when `n` files fit one revert (1..=32).
#[must_use]
pub fn files_ok(n: usize) -> bool {
    n > 0 && n <= MAX_FILES
}

/// Truncate to 512 chars on char boundaries.
#[must_use]
pub fn msg_trunc(s: &str) -> String {
    s.chars().take(MSG_MAX).collect()
}

/// True when line opens a unified-diff hunk (`@@ ... @@`).
#[must_use]
pub fn hunk_ok(s: &str) -> bool {
    s.starts_with("@@")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_bounds() {
        assert!(files_ok(1));
        assert!(files_ok(32));
        assert!(!files_ok(0));
        assert!(!files_ok(33));
    }

    #[test]
    fn trunc_caps_at_512() {
        assert_eq!(msg_trunc(&"x".repeat(600)).chars().count(), MSG_MAX);
        assert_eq!(msg_trunc("hi"), "hi");
    }

    #[test]
    fn trunc_char_safe() {
        let s = "e".repeat(511) + "\u{1f600}";
        assert_eq!(msg_trunc(&format!("{s}x")).chars().count(), MSG_MAX);
    }

    #[test]
    fn hunk_prefix() {
        assert!(hunk_ok("@@ -1 +1 @@"));
        assert!(!hunk_ok("--- a"));
        assert!(!hunk_ok(""));
    }
}
