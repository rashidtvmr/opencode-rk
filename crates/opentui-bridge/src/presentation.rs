#![forbid(unsafe_code)]
//! Presentation helpers (std only).
//!
//! No upstream equivalent: `packages/tui/src/util/presentation.ts:1-38`
//! holds only wordmark/sessionEpilogue. Bounds below are fail-closed port
//! caps. Char-safe style mirrors `message_render.rs` `trunc_line`.
//!
//! ponytail: skipped wiring `pub mod presentation` into lib.rs (out of scope
//! for this lane); add when bridge integrates this module.

/// Max indent spaces applied by [`indent`].
pub const INDENT_CAP: usize = 32;

/// Keep first `max` lines. Fail-closed: `max == 0` or empty yields `""`.
#[must_use]
pub fn clamp_lines(text: &str, max: usize) -> String {
    if max == 0 || text.is_empty() {
        return String::new();
    }
    text.lines().take(max).collect::<Vec<_>>().join("\n")
}

/// Passthrough when `chars <= max`, else first `max` chars + `"..."`.
/// Fail-closed: `max == 0` yields `""`.
#[must_use]
pub fn ellipsize(text: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if text.chars().count() <= max {
        return text.to_string();
    }
    let head: String = text.chars().take(max).collect();
    format!("{head}...")
}

/// Prefix every line with `n` spaces (`n` clamped to [`INDENT_CAP`]).
#[must_use]
pub fn indent(text: &str, n: usize) -> String {
    let pad = " ".repeat(n.min(INDENT_CAP));
    text.split('\n')
        .map(|l| format!("{pad}{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_keeps_head() {
        assert_eq!(clamp_lines("a\nb\nc", 2), "a\nb");
    }

    #[test]
    fn clamp_zero_empty() {
        assert_eq!(clamp_lines("a\nb", 0), "");
        assert_eq!(clamp_lines("", 3), "");
    }

    #[test]
    fn ellipsis_truncates() {
        assert_eq!(ellipsize("abcdef", 3), "abc...");
        assert_eq!(ellipsize("😀😀😀😀", 2), "😀😀...");
    }

    #[test]
    fn ellipsis_short_passthrough() {
        assert_eq!(ellipsize("ab", 5), "ab");
        assert_eq!(ellipsize("abc", 3), "abc");
    }

    #[test]
    fn indent_prefix() {
        assert_eq!(indent("a\nb", 2), "  a\n  b");
        assert_eq!(indent("x", 100).len(), INDENT_CAP + 1);
    }
}
