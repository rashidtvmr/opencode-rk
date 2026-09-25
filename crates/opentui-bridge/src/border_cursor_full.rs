#![forbid(unsafe_code)]
//! Border frame + cursor mark over rule/clip primitives.
//!
//! `ponytail:` ASCII `|` bars + `-` rule only; unicode box variant when needed.

use crate::clip_line::pad_line;
use crate::rule_line::{rule_line, RULE_CAP};

/// Max framed rows; extras dropped.
pub const FRAME_CAP: usize = 64;
/// Max chars in `cursor_mark` output (line + block).
pub const CURSOR_CAP: usize = 257;
/// Block cursor cell appended by `cursor_mark`.
pub const CURSOR_CELL: char = '\u{2588}';

/// Top rule; delegates to `rule_line`.
#[must_use]
pub fn border_top(width: usize) -> String {
    rule_line(width)
}

/// Frame each line as `|padded|`; first `FRAME_CAP` rows only.
#[must_use]
pub fn border_frame(lines: &[String], width: usize) -> Vec<String> {
    let w = width.min(RULE_CAP);
    lines
        .iter()
        .take(FRAME_CAP)
        .map(|l| format!("|{}|", pad_line(l, w)))
        .collect()
}

/// Append block cursor; output capped at `CURSOR_CAP` chars.
#[must_use]
pub fn cursor_mark(line: &str) -> String {
    let mut s: String = line.chars().take(CURSOR_CAP - 1).collect();
    s.push(CURSOR_CELL);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_delegates_to_rule() {
        assert_eq!(border_top(3), "---");
        assert_eq!(border_top(0), "-");
        assert_eq!(border_top(500).len(), RULE_CAP);
    }

    #[test]
    fn frame_bars_and_pad() {
        assert_eq!(border_frame(&["ab".to_string()], 4), vec!["|ab  |"]);
    }

    #[test]
    fn frame_clips_wide() {
        assert_eq!(border_frame(&["abcdef".to_string()], 3), vec!["|abc|"]);
    }

    #[test]
    fn frame_caps_rows() {
        let lines: Vec<String> = (0..70).map(|i| i.to_string()).collect();
        let out = border_frame(&lines, 2);
        assert_eq!(out.len(), FRAME_CAP);
        assert!(out[0].starts_with('|') && out[0].ends_with('|'));
    }

    #[test]
    fn frame_empty() {
        assert!(border_frame(&[], 4).is_empty());
    }

    #[test]
    fn cursor_appends_block() {
        assert_eq!(cursor_mark("ab"), "ab\u{2588}");
    }

    #[test]
    fn cursor_caps_length() {
        let long: String = "x".repeat(300);
        let out = cursor_mark(&long);
        assert_eq!(out.chars().count(), CURSOR_CAP);
        assert!(out.ends_with(CURSOR_CELL));
    }
}
