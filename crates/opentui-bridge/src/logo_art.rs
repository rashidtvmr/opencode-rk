#![forbid(unsafe_code)]
//! ASCII wordmark art (exact port, no rendering deps).
//!
//! Evidence (TS checkout a0d9b6c; task notes divergence from 95daf90):
//! - lines: `packages/tui/src/logo.ts:1-4` (duplicated verbatim in
//!   `packages/tui/src/util/presentation.ts:1-4` for the epilogue path).
//! - side-by-side layout: `packages/tui/src/component/logo.tsx:51-58`
//!   (left[i] + gap + right[i] per row) and
//!   `packages/tui/src/util/presentation.ts:22-26`
//!   (`${pad}${left} ${right}` per line).
//! - glyph marks `_^~,`: `packages/tui/src/logo.ts:11`,
//!   decoded in logo.tsx:9-47 / presentation.ts:11-21 (renderer-owned here).
//!
//! Bound: `MAX_WORDMARK_ROWS` caps rows consumed by [`render_wordmark`].

/// Left wordmark rows, verbatim from `packages/tui/src/logo.ts:2`.
pub const LOGO_LEFT: &[&str] = &[
    "                   ",
    "█▀▀█ █▀▀█ █▀▀█ █▀▀▄",
    "█__█ █__█ █^^^ █__█",
    "▀▀▀▀ █▀▀▀ ▀▀▀▀ ▀~~▀",
];

/// Right wordmark rows, verbatim from `packages/tui/src/logo.ts:3`.
pub const LOGO_RIGHT: &[&str] = &[
    "             ▄     ",
    "█▀▀▀ █▀▀█ █▀▀█ █▀▀█",
    "█___ █__█ █__█ █^^^",
    "▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀",
];

/// Wordmark row count (both halves must match).
pub const WORDMARK_ROWS: usize = 4;
/// Cap on rows consumed by [`render_wordmark`].
pub const MAX_WORDMARK_ROWS: usize = 16;

/// Side-by-side rows: `pad + left[i] + gap + right[i]` (presentation.ts:25).
/// Stops at the shorter half; at most `MAX_WORDMARK_ROWS` rows.
#[must_use]
pub fn render_wordmark(pad: &str, gap: &str) -> Vec<String> {
    LOGO_LEFT
        .iter()
        .zip(LOGO_RIGHT.iter())
        .take(MAX_WORDMARK_ROWS)
        .map(|(l, r)| format!("{pad}{l}{gap}{r}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves_match_row_count() {
        assert_eq!(LOGO_LEFT.len(), WORDMARK_ROWS);
        assert_eq!(LOGO_RIGHT.len(), WORDMARK_ROWS);
    }

    #[test]
    fn lines_exact() {
        assert_eq!(LOGO_LEFT[1], "█▀▀█ █▀▀█ █▀▀█ █▀▀▄");
        assert_eq!(LOGO_RIGHT[0], "             ▄     ");
        assert_eq!(LOGO_LEFT[3], "▀▀▀▀ █▀▀▀ ▀▀▀▀ ▀~~▀");
    }

    #[test]
    fn render_joins_with_pad_gap() {
        let rows = render_wordmark("  ", " ");
        assert_eq!(rows.len(), WORDMARK_ROWS);
        assert_eq!(rows[0], format!("  {} {}", LOGO_LEFT[0], LOGO_RIGHT[0]));
    }

    #[test]
    fn render_bounded() {
        assert!(render_wordmark("", "").len() <= MAX_WORDMARK_ROWS);
        assert_eq!(render_wordmark("", "-")[2].contains('-'), true);
    }
}
