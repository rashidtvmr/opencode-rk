#![forbid(unsafe_code)]
//! Border style helpers (mirrors `packages/tui/src/ui/border.ts:1-21`).
//! ponytail: single horizontal char only; add when full charset needed.

/// Horizontal char: `-` for ascii else `─`.
#[must_use]
pub fn border_char(style: &str) -> char {
    if style == "ascii" {
        '-'
    } else {
        '─'
    }
}

/// Clamp width to minimum 2.
#[must_use]
pub fn border_width(w: u32) -> u32 {
    w.max(2)
}

/// True only for `rounded` style.
#[must_use]
pub fn is_rounded(style: &str) -> bool {
    style == "rounded"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_dash_else_box() {
        assert_eq!(border_char("ascii"), '-');
        assert_eq!(border_char("single"), '─');
        assert_eq!(border_char("rounded"), '─');
    }

    #[test]
    fn width_min_two() {
        assert_eq!(border_width(0), 2);
        assert_eq!(border_width(1), 2);
        assert_eq!(border_width(5), 5);
    }

    #[test]
    fn rounded_only() {
        assert!(is_rounded("rounded"));
        assert!(!is_rounded("ascii"));
        assert!(!is_rounded("single"));
    }
}
