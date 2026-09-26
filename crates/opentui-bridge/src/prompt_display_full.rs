#![forbid(unsafe_code)]
//! Prompt display helpers (first-line clip, count label, empty check).
//!
//! TS truth `packages/tui/src/prompt/display.ts`: width-aware slice/char
//! helpers over graphemes. This file keeps the small subset callers need:
//! first-line clipped preview via [`crate::unicode_width::clip_to_width`].

use crate::unicode_width::clip_to_width;

/// First line of `text`, clipped to `width` display columns, cluster-safe.
#[must_use]
pub fn display_text(text: &str, width: usize) -> String {
    let first = text.lines().next().unwrap_or("");
    clip_to_width(first, width).0
}

/// Label for a character count, e.g. `3 chars`.
#[must_use]
pub fn display_count(n: usize) -> String {
    format!("{n} chars")
}

/// True when `text` holds no bytes.
#[must_use]
pub fn is_empty(text: &str) -> bool {
    text.is_empty()
}
// ponytail: no plural/singular split; add when UI needs it.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_ascii() {
        assert_eq!(display_text("abcdef", 3), "abc");
    }

    #[test]
    fn first_line_only() {
        assert_eq!(display_text("ab\ncd", 10), "ab");
    }

    #[test]
    fn wide_boundary_safe() {
        assert_eq!(display_text("a\u{4e2d}b", 3), "a\u{4e2d}");
        assert_eq!(display_text("a\u{4e2d}b", 2), "a");
    }

    #[test]
    fn count_label() {
        assert_eq!(display_count(3), "3 chars");
        assert_eq!(display_count(0), "0 chars");
    }

    #[test]
    fn empty_check() {
        assert!(is_empty(""));
        assert!(!is_empty("a"));
    }
}
