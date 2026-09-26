#![forbid(unsafe_code)]
//! Head+tail collapsed view (complements head-only `collapse.rs`).
//! Split on `\n` never splits UTF-8 sequences, so lines are char-safe.

/// Head+tail window: how many leading/trailing lines to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollapseView {
    pub head_lines: usize,
    pub tail_lines: usize,
}

impl CollapseView {
    #[must_use]
    pub fn new(head_lines: usize, tail_lines: usize) -> Self {
        Self {
            head_lines,
            tail_lines,
        }
    }

    #[must_use]
    pub fn preview(&self, text: &str) -> Vec<String> {
        preview_window(text, self.head_lines, self.tail_lines)
    }

    #[must_use]
    pub fn collapse(&self, text: &str) -> String {
        self.preview(text).join("\n")
    }
}

/// Keep all lines when `<= max`, else head `ceil(max/2)` + marker + tail `floor(max/2)`.
#[must_use]
pub fn collapse(text: &str, max: usize) -> String {
    preview(text, max).join("\n")
}

/// Same as [`collapse`] but as owned lines.
#[must_use]
pub fn preview(text: &str, max: usize) -> Vec<String> {
    preview_window(text, (max + 1) / 2, max / 2)
}

fn preview_window(text: &str, head: usize, tail: usize) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.len() <= head + tail {
        return lines.iter().map(|s| (*s).to_string()).collect();
    }
    let hidden = lines.len() - head - tail;
    let mut out = Vec::with_capacity(head + tail + 1);
    out.extend(lines[..head].iter().map(|s| (*s).to_string()));
    out.push(format!("... {hidden} lines collapsed ..."));
    if tail > 0 {
        out.extend(lines[lines.len() - tail..].iter().map(|s| (*s).to_string()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_passthrough() {
        assert_eq!(collapse("a\nb", 5), "a\nb");
        assert_eq!(preview("a\nb", 5), vec!["a", "b"]);
    }

    #[test]
    fn collapsed_marker() {
        assert_eq!(collapse("a\nb\nc\nd", 2), "a\n... 2 lines collapsed ...\nd");
    }

    #[test]
    fn head_tail_kept() {
        assert_eq!(
            collapse("a\nb\nc\nd\ne", 3),
            "a\nb\n... 2 lines collapsed ...\ne"
        );
        assert_eq!(
            CollapseView::new(2, 1).collapse("a\nb\nc\nd\ne"),
            "a\nb\n... 2 lines collapsed ...\ne"
        );
    }

    #[test]
    fn empty() {
        assert_eq!(collapse("", 3), "");
        assert!(preview("", 3).is_empty());
    }

    #[test]
    fn max_zero() {
        assert_eq!(collapse("a\nb", 0), "... 2 lines collapsed ...");
    }

    #[test]
    fn emoji_lines_intact() {
        assert_eq!(
            collapse("😀\n🎉\n🚀", 2),
            "😀\n... 1 lines collapsed ...\n🚀"
        );
    }
}
