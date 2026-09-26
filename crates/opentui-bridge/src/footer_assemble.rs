#![forbid(unsafe_code)]
//! Footer assembly: truncate items then join into one clipped line.
//!
//! TS truth `packages/tui/src/routes/session/footer.tsx` lays out a
//! directory slot plus status items in one row (`flexDirection="row"`);
//! this helper owns the join-and-clip step. Width math reuses
//! `crate::footer_width_calc::truncate_to` (separator ` | ` = 3 cols).

use crate::footer_width_calc::truncate_to;

/// Assemble one footer line, char-safe clipped to `width` cols.
/// Empty items fall back to `vec![slot]`; else join kept items
/// with `" | "` and clip. Always returns exactly one line.
#[must_use]
pub fn assemble_footer(slot: &str, items: &[String], width: usize) -> Vec<String> {
    if items.is_empty() {
        return vec![slot.to_string()];
    }
    let kept = truncate_to(items, width);
    let line = kept.join(" | ");
    vec![line.chars().take(width).collect()]
}

/// Footer height in lines: always one row.
#[must_use]
pub fn footer_height() -> usize {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_items_returns_slot() {
        assert_eq!(assemble_footer("dir", &[], 80), vec!["dir".to_string()]);
    }

    #[test]
    fn joins_with_separator() {
        assert_eq!(
            assemble_footer("dir", &strs(&["1 LSP", "2 MCP"]), 80),
            vec!["1 LSP | 2 MCP".to_string()]
        );
    }

    #[test]
    fn drops_tail_via_truncate_to() {
        // raw 15 cols; width 10 keeps ["aaa","bbb"] per truncate_to.
        let out = assemble_footer("dir", &strs(&["aaa", "bbb", "ccc"]), 10);
        assert_eq!(out, vec!["aaa | bbb".to_string()]);
    }

    #[test]
    fn clips_long_line_char_safe() {
        let out = assemble_footer("dir", &strs(&["abcdefgh"]), 3);
        assert_eq!(out, vec!["abc".to_string()]);
    }

    #[test]
    fn clips_unicode_char_safe() {
        let out = assemble_footer("dir", &strs(&["\u{25b3}\u{25b3}\u{25b3}"]), 2);
        assert_eq!(out, vec!["\u{25b3}\u{25b3}".to_string()]);
    }

    #[test]
    fn height_is_one_and_width_zero_is_empty_line() {
        assert_eq!(footer_height(), 1);
        assert_eq!(
            assemble_footer("dir", &strs(&["ab"]), 0),
            vec!["".to_string()]
        );
    }
}
