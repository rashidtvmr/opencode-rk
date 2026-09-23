#![forbid(unsafe_code)]
//! Collapse tool output (mirrors `collapseToolOutput` in
//! `packages/tui/src/util/collapse-tool-output.ts`, TS checkout a0d9b6c).
//!
//! Call sites (`packages/tui/src/routes/session/index.tsx:1796`, `:2046`,
//! `:2349`) pass dynamic limits, no fixed defaults:
//! `maxLines` 3 / 10 / 4 with `maxChars = maxLines * max(20, width - 6)`.

/// Collapsed preview plus whether content was cut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collapse {
    pub preview: String,
    pub overflow: bool,
}

/// TS `collapseToolOutput`: split on `\n`, overflow when line count or char
/// count exceeds max; char count in chars (JS `Array.from` code points).
/// Fail-closed: zero line/char limit yields empty preview + overflow.
#[must_use]
pub fn collapse_tool_output(output: &str, max_lines: usize, max_chars: usize) -> Collapse {
    if max_lines == 0 || max_chars == 0 {
        return Collapse { preview: String::new(), overflow: true };
    }
    let lines: Vec<&str> = output.split('\n').collect();
    if lines.len() <= max_lines && output.chars().count() <= max_chars {
        return Collapse { preview: output.to_string(), overflow: false };
    }
    let head = &lines[..max_lines.min(lines.len())];
    let preview = head.join("\n");
    if preview.chars().count() > max_chars {
        let cut: String = preview.chars().take(max_chars.saturating_sub(1)).collect();
        return Collapse { preview: format!("{cut}…"), overflow: true };
    }
    let mut parts: Vec<&str> = head.to_vec();
    parts.push("…");
    Collapse { preview: parts.join("\n"), overflow: true }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn under_limits_passthrough() {
        assert_eq!(
            collapse_tool_output("a\nb", 10, 100),
            Collapse { preview: "a\nb".to_string(), overflow: false }
        );
    }

    #[test]
    fn line_overflow_appends_ellipsis_line() {
        assert_eq!(
            collapse_tool_output("a\nb\nc\nd", 2, 100),
            Collapse { preview: "a\nb\n…".to_string(), overflow: true }
        );
    }

    #[test]
    fn char_overflow_truncates_with_ellipsis() {
        assert_eq!(
            collapse_tool_output("abcdefgh", 10, 5),
            Collapse { preview: "abcd…".to_string(), overflow: true }
        );
    }

    #[test]
    fn zero_limits_fail_closed() {
        for (l, c) in [(0, 0), (0, 10), (10, 0)] {
            assert_eq!(
                collapse_tool_output("abc", l, c),
                Collapse { preview: String::new(), overflow: true }
            );
        }
    }

    #[test]
    fn emoji_counts_one_char() {
        assert_eq!(
            collapse_tool_output("😀😀😀", 10, 3),
            Collapse { preview: "😀😀😀".to_string(), overflow: false }
        );
        assert_eq!(
            collapse_tool_output("😀😀😀", 10, 2),
            Collapse { preview: "😀…".to_string(), overflow: true }
        );
    }
}
