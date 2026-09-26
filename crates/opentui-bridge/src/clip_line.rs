#![forbid(unsafe_code)]
//! Clip / pad one line to a display-width budget.

use crate::unicode_width::{clip_to_width, line_width};

/// Clip `s` to `width` display columns.
#[must_use]
pub fn clip_line(s: &str, width: usize) -> String {
    clip_to_width(s, width).0
}

/// Clip then space-pad to exactly `width` display columns.
#[must_use]
pub fn pad_line(s: &str, width: usize) -> String {
    let clipped = clip_line(s, width);
    let w = line_width(&clipped);
    let mut out = clipped;
    for _ in w..width {
        out.push(' ');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_ascii() {
        assert_eq!(clip_line("abcdef", 3), "abc");
    }

    #[test]
    fn clip_wide_boundary() {
        assert_eq!(clip_line("a\u{4e2d}b", 3), "a\u{4e2d}");
        assert_eq!(clip_line("a\u{4e2d}b", 2), "a");
    }

    #[test]
    fn pad_ascii() {
        assert_eq!(pad_line("ab", 4), "ab  ");
    }

    #[test]
    fn pad_truncates_and_keeps_width() {
        assert_eq!(pad_line("abcdef", 3), "abc");
        assert_eq!(pad_line("a\u{4e2d}", 4), "a\u{4e2d} ");
    }

    #[test]
    fn pad_empty_and_zero() {
        assert_eq!(pad_line("", 2), "  ");
        assert_eq!(pad_line("ab", 0), "");
    }
}
