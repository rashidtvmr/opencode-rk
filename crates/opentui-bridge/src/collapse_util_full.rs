#![forbid(unsafe_code)]
//! Full-list collapse: head + hidden marker + tail.

/// Default visible line budget when caller passes no limit.
pub const COLLAPSE_DEFAULT: usize = 20;

/// Collapse lines to at most `max` visible lines plus one marker line.
/// Under budget returns all lines unchanged.
#[must_use]
pub fn collapse_lines(lines: &[String], max: usize) -> Vec<String> {
    if lines.len() <= max {
        return lines.to_vec();
    }
    if max == 0 {
        return vec![format!("... ({} lines hidden) ...", lines.len())];
    }
    let hidden = lines.len() - max;
    let head_n = (max + 1) / 2;
    let tail_n = max - head_n;
    let mut out = Vec::with_capacity(max + 1);
    out.extend_from_slice(&lines[..head_n]);
    out.push(format!("... ({hidden} lines hidden) ..."));
    if tail_n > 0 {
        out.extend_from_slice(&lines[lines.len() - tail_n..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn under_max_returns_all() {
        assert_eq!(collapse_lines(&s(&["a", "b"]), 5), s(&["a", "b"]));
    }

    #[test]
    fn exact_max_returns_all() {
        assert_eq!(collapse_lines(&s(&["a", "b"]), 2), s(&["a", "b"]));
    }

    #[test]
    fn over_max_head_marker_tail() {
        let lines = s(&["a", "b", "c", "d", "e"]);
        assert_eq!(
            collapse_lines(&lines, 4),
            s(&["a", "b", "... (1 lines hidden) ...", "d", "e"])
        );
    }

    #[test]
    fn odd_max_splits_head_heavy() {
        let lines = s(&["a", "b", "c", "d", "e", "f"]);
        assert_eq!(
            collapse_lines(&lines, 3),
            s(&["a", "b", "... (3 lines hidden) ...", "f"])
        );
    }

    #[test]
    fn zero_max_only_marker() {
        assert_eq!(
            collapse_lines(&s(&["a", "b"]), 0),
            s(&["... (2 lines hidden) ..."])
        );
    }

    #[test]
    fn default_const_is_20() {
        assert_eq!(COLLAPSE_DEFAULT, 20);
    }
}
