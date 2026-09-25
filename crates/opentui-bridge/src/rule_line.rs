#![forbid(unsafe_code)]
//! Horizontal rule line: `-` repeated to width, capped at 120.
//!
//! TS truth (`tui_entry.rs:486,511`): rule is `"-".repeat(width.min(120))`.
//! Width floors to 1 so a rule always renders one dash.
//!
//! `ponytail:` ASCII dash only; box-char variant when caller needs it.

/// Max rule width in chars; mirrors `width.min(120)` in `tui_entry.rs`.
pub const RULE_CAP: usize = 120;

/// Rule of `-` with length `width.max(1).min(RULE_CAP)`.
#[must_use]
pub fn rule_line(width: usize) -> String {
    "-".repeat(width.max(1).min(RULE_CAP))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_floors_to_one() {
        assert_eq!(rule_line(0), "-");
    }

    #[test]
    fn exact_width() {
        assert_eq!(rule_line(5), "-----");
    }

    #[test]
    fn caps_at_120() {
        let s = rule_line(200);
        assert_eq!(s.len(), RULE_CAP);
        assert!(s.chars().all(|c| c == '-'));
    }

    #[test]
    fn boundary_120() {
        assert_eq!(rule_line(120).len(), 120);
        assert_eq!(rule_line(121).len(), 120);
    }

    #[test]
    fn width_one() {
        assert_eq!(rule_line(1), "-");
    }
}
