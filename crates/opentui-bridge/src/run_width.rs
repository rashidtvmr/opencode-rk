#![forbid(unsafe_code)]
//! Footer width policy: compact vs full footer surfaces.
//!
//! TS truth: `packages/opencode/src/cli/cmd/run/footer.width.ts`
//! `footerWidthPolicy` (read-only ref, not vendored). Breakpoints:
//! under 60 cols hides status, under 100 cols compacts, else full.

/// Compact/full footer surface decision for a viewport width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidthPolicy {
    /// True when the footer must render in compact form.
    pub compact: bool,
    /// True when the status cell is hidden (narrowest tier).
    pub hide_status: bool,
    /// Echoed viewport width in columns.
    pub columns: u32,
}

impl WidthPolicy {
    #[must_use]
    pub const fn new(compact: bool, hide_status: bool, columns: u32) -> Self {
        Self {
            compact,
            hide_status,
            columns,
        }
    }
}

/// Pure breakpoint map: `<60` compact + hide status, `<100` compact,
/// otherwise full. Zero width fails closed to compact + hidden.
#[must_use]
pub const fn policy(width: u32) -> WidthPolicy {
    if width < 60 {
        WidthPolicy::new(true, true, width)
    } else if width < 100 {
        WidthPolicy::new(true, false, width)
    } else {
        WidthPolicy::new(false, false, width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_hides_status() {
        assert_eq!(policy(59), WidthPolicy::new(true, true, 59));
    }

    #[test]
    fn sixty_compacts_without_hiding() {
        assert_eq!(policy(60), WidthPolicy::new(true, false, 60));
    }

    #[test]
    fn ninety_nine_stays_compact() {
        assert_eq!(policy(99), WidthPolicy::new(true, false, 99));
    }

    #[test]
    fn hundred_is_full() {
        assert_eq!(policy(100), WidthPolicy::new(false, false, 100));
    }

    #[test]
    fn wide_is_full() {
        assert_eq!(policy(200), WidthPolicy::new(false, false, 200));
    }

    #[test]
    fn zero_fails_closed() {
        let p = policy(0);
        assert!(p.compact && p.hide_status);
    }
}
