#![forbid(unsafe_code)]
//! Present TS-full bounded helpers (std only).
//!
//! No upstream equivalent: `packages/tui/src/util/presentation.ts:1-38`
//! holds only wordmark/sessionEpilogue. Bounds below are fail-closed port
//! caps. Char-safe clip mirrors `message_render.rs` `trunc_line`.
//!
//! ponytail: skipped wiring `pub mod present_ts_full` into lib.rs (out of
//! scope for this lane); add when bridge integrates this module.

/// Max chars kept by [`present_line`].
pub const PRESENT_LINE_CAP: usize = 512;

/// Clip to `width` chars, capped at [`PRESENT_LINE_CAP`]. Zero yields `""`.
#[must_use]
pub fn present_line(s: &str, width: usize) -> String {
    let max = width.min(PRESENT_LINE_CAP);
    if max == 0 {
        return String::new();
    }
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect()
}

/// Clamp counts for display: never above 999.
#[must_use]
pub fn present_count(n: usize) -> usize {
    n.min(999)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_to_width_char_safe() {
        assert_eq!(present_line("abcdef", 3), "abc");
        assert_eq!(present_line("héllo", 2), "hé");
    }

    #[test]
    fn caps_at_512_and_zero() {
        assert_eq!(present_line("ab", 0), "");
        assert_eq!(present_line(&"x".repeat(600), 600).chars().count(), 512);
        assert_eq!(present_line(&"x".repeat(600), 10).chars().count(), 10);
    }

    #[test]
    fn count_clamps_to_999() {
        assert_eq!(present_count(5), 5);
        assert_eq!(present_count(999), 999);
        assert_eq!(present_count(1000), 999);
    }
}
