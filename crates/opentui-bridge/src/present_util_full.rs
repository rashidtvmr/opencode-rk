#![forbid(unsafe_code)]
//! Presentation string helpers.
//!
//! Mirrors TS truth: `packages/tui/src/util/presentation.ts`
//! (`sessionEpilogue` layout: fixed labels padded to width 10, sections
//! joined with blank rules). The TS file has no truncate/pad/rule units;
//! these are the generic std-only equivalents used by header/footer
//! rendering: middle-truncation with `...`, right-padding to a char
//! width, and a `-- title --` rule line.

/// Middle-truncate to at most `max` chars: `head...tail`, char-safe.
#[must_use]
pub fn truncate_middle(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    if max <= 3 {
        return chars.into_iter().take(max).collect();
    }
    let inner = max - 3;
    let head = (inner + 1) / 2;
    let tail = inner / 2;
    let h: String = chars.iter().take(head).collect();
    let t: String = chars.iter().skip(chars.len() - tail).collect();
    format!("{h}...{t}")
}

/// Right-pad with spaces to char-width `w`; never truncates.
#[must_use]
pub fn pad_right(s: &str, w: usize) -> String {
    let len = s.chars().count();
    if len >= w {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + (w - len));
    out.push_str(s);
    for _ in 0..(w - len) {
        out.push(' ');
    }
    out
}

/// `"-- {title} " + "-"` padded to char-width `width`; long titles truncated.
#[must_use]
pub fn header_rule(title: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if title.is_empty() {
        return "-".repeat(width);
    }
    let prefix = format!("-- {title} ");
    let n = prefix.chars().count();
    if n >= width {
        return truncate_middle(&prefix, width);
    }
    let mut out = prefix;
    for _ in 0..(width - n) {
        out.push('-');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_passthrough() {
        assert_eq!(truncate_middle("hi", 10), "hi");
        assert_eq!(truncate_middle("abc", 3), "abc");
    }

    #[test]
    fn middle_truncates_char_safe() {
        assert_eq!(truncate_middle("abcdefghij", 7), "ab...ij");
        assert_eq!(truncate_middle("héllo wörld", 7), "hé...ld");
    }

    #[test]
    fn tiny_max_no_ellipsis_room() {
        assert_eq!(truncate_middle("abcdef", 2), "ab");
        assert_eq!(truncate_middle("héllo", 2), "hé");
    }

    #[test]
    fn pad_right_pads_and_keeps() {
        assert_eq!(pad_right("ab", 5), "ab   ");
        assert_eq!(pad_right("abcde", 3), "abcde");
        assert_eq!(pad_right("hé", 4), "hé  ");
    }

    #[test]
    fn rule_shapes() {
        assert_eq!(header_rule("", 4), "----");
        assert_eq!(header_rule("hi", 8), "-- hi --");
        assert_eq!(header_rule("", 0), "");
        assert_eq!(header_rule("toolongtitle", 5).chars().count(), 5);
    }
}
