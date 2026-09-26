#![forbid(unsafe_code)]
//! Display-width clip/wrap check (bounded, char-safe, std-only).
//!
//! Bug documented: `text.rs:34-82` (`clip_chars`, `wrap_text`, `clip_lines`)
//! use `chars().take(n)` which counts scalars, not columns: CJK/emoji count
//! 1 but occupy 2, so rows overflow their column budget. Use [`clip_disp`].
//! Divider overflow: `"─".repeat(width)` with raw terminal width can OOM or
//! exceed the draw cap; clamp dividers to `width.min(120)` (and all budgets
//! here to [`MAX_WIDTH`]).
//! ponytail: delegates to `crate::unicode_width`; upgrade: `unicode-width` crate.

use crate::unicode_width::{char_width, clip_to_width, line_width};

/// Hard cap for any width budget (draw-call safety).
pub const MAX_WIDTH: usize = 512;
/// Hard cap for repeated divider glyphs.
pub const MAX_DIVIDER: usize = 120;

/// Display columns of one char: CJK/emoji 2, combining 0, else 1.
#[must_use]
pub fn disp_width(ch: char) -> usize {
    char_width(ch)
}

/// Clip `s` to `max` display columns (clamped to [`MAX_WIDTH`]).
#[must_use]
pub fn clip_disp(s: &str, max: usize) -> String {
    clip_to_width(s, max.min(MAX_WIDTH)).0
}

/// True when every `\n`-split line fits `width` display columns.
#[must_use]
pub fn wrap_ok(s: &str, width: usize) -> bool {
    let w = width.min(MAX_WIDTH);
    if w == 0 {
        return s.is_empty();
    }
    s.split('\n').all(|l| line_width(l) <= w)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cjk_two_combining_zero() {
        assert_eq!(disp_width('中'), 2);
        assert_eq!(disp_width('\u{0301}'), 0);
        assert_eq!(disp_width('a'), 1);
    }
    #[test]
    fn clip_cjk_boundary() {
        assert_eq!(clip_disp("a中b", 3), "a中");
        assert_eq!(clip_disp("a中b", 2), "a");
    }
    #[test]
    fn clip_bounded() {
        assert_eq!(clip_disp("abc", usize::MAX).len(), 3);
        assert_eq!(clip_disp("", 0), "");
    }
    #[test]
    fn wrap_cjk() {
        assert!(wrap_ok("a中", 3));
        assert!(!wrap_ok("a中b", 3));
        assert!(wrap_ok("a\nb", 1));
    }
    #[test]
    fn wrap_zero_empty() {
        assert!(wrap_ok("", 0));
        assert!(!wrap_ok("a", 0));
    }
}
