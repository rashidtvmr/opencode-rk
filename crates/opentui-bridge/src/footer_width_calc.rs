#![forbid(unsafe_code)]
//! Footer items width calc: sum chars + 3-col separators, clamped.
//!
//! TS truth: `packages/opencode/src/cli/cmd/run/footer.width.ts`
//! (`footerWidthPolicy`; breakpoints only, no item sum exported).
//! Rust ref (read-only): `crate::run_width.rs` breakpoint tiers.
//! This file adds item-level measuring: join separator ` | ` = 3 cols.

/// Raw width without clamp: sum chars + 3 per gap.
#[must_use]
fn raw_width(items: &[String]) -> usize {
    if items.is_empty() {
        return 0;
    }
    let chars: usize = items.iter().map(|s| s.chars().count()).sum();
    chars + 3 * (items.len() - 1)
}

/// Measured width clamped to `max`.
#[must_use]
pub fn width_for(items: &[String], max: usize) -> usize {
    raw_width(items).min(max)
}

/// True when items fit in `width` columns unclamped.
#[must_use]
pub fn fits(items: &[String], width: usize) -> bool {
    raw_width(items) <= width
}

/// Drop tail items until fits; keeps min 1 (even if still over).
#[must_use]
pub fn truncate_to(items: &[String], width: usize) -> Vec<String> {
    if items.is_empty() {
        return Vec::new();
    }
    let mut out = items.to_vec();
    while out.len() > 1 && !fits(&out, width) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_zero() {
        assert_eq!(width_for(&[], 80), 0);
        assert!(fits(&[], 0));
        assert!(truncate_to(&[], 10).is_empty());
    }

    #[test]
    fn single_sums_chars() {
        let items = vec!["hello".to_string()];
        assert_eq!(width_for(&items, 80), 5);
        assert!(fits(&items, 5));
        assert!(!fits(&items, 4));
    }

    #[test]
    fn multi_adds_three_per_gap() {
        let items = vec!["ab".to_string(), "cde".to_string(), "f".to_string()];
        // 2+3+1 + 3*2 = 12
        assert_eq!(width_for(&items, 80), 12);
    }

    #[test]
    fn clamps_to_max() {
        let items = vec!["abcdefgh".to_string(), "ijklmnop".to_string()];
        // raw 8+8+3=19
        assert_eq!(width_for(&items, 10), 10);
    }

    #[test]
    fn drops_tail_until_fits() {
        let items = vec!["aaa".to_string(), "bbb".to_string(), "ccc".to_string()];
        // raw 9+6=15; width 10 fits ["aaa","bbb"] (3+3+3=9)
        let out = truncate_to(&items, 10);
        assert_eq!(out, vec!["aaa".to_string(), "bbb".to_string()]);
    }

    #[test]
    fn keeps_min_one_when_nothing_fits() {
        let items = vec!["abcdef".to_string(), "gh".to_string()];
        let out = truncate_to(&items, 2);
        assert_eq!(out, vec!["abcdef".to_string()]);
    }
}
