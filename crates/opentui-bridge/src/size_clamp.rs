#![forbid(unsafe_code)]
//! Terminal size clamp for `tui_entry native_terminal_size`.
//!
//! Evidence: `crates/cli/src/tui_entry.rs:428` (`native_terminal_size`
//! returns `(u32, u32)`, default `(80, 24)`) + `tui_entry.rs:466-467`
//! (`width.max(20)`, `height.max(8)`). Saturating `u16` output fits
//! crossterm `SetSize(u16, u16)`.

/// Minimum usable columns (tui_entry.rs:466).
pub const MIN_COLS: u16 = 20;
/// Minimum usable rows (tui_entry.rs:467).
pub const MIN_ROWS: u16 = 8;

/// Clamp raw `(cols, rows)` to `[MIN, u16::MAX]` per axis.
#[must_use]
pub fn clamp_size(cols: u32, rows: u32) -> (u16, u16) {
    // ponytail: single max().min() chain; upgrade to per-axis struct when callers need it.
    let c = cols.max(u32::from(MIN_COLS)).min(u32::from(u16::MAX)) as u16;
    let r = rows.max(u32::from(MIN_ROWS)).min(u32::from(u16::MAX)) as u16;
    (c, r)
}

/// True when the clamped size differs (exact cell compare, no epsilon).
#[must_use]
pub fn size_changed(a: (u16, u16), b: (u16, u16)) -> bool {
    a != b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_size_passes_through() {
        assert_eq!(clamp_size(80, 24), (80, 24));
    }

    #[test]
    fn tiny_and_zero_clamp_to_min() {
        assert_eq!(clamp_size(0, 0), (MIN_COLS, MIN_ROWS));
        assert_eq!(clamp_size(5, 3), (MIN_COLS, MIN_ROWS));
        assert_eq!(clamp_size(19, 7), (MIN_COLS, MIN_ROWS));
    }

    #[test]
    fn huge_saturates_at_u16_max() {
        assert_eq!(clamp_size(u32::MAX, u32::MAX), (u16::MAX, u16::MAX));
        assert_eq!(clamp_size(70_000, 100), (u16::MAX, 100));
    }

    #[test]
    fn changed_detects_any_axis() {
        assert!(!size_changed((80, 24), (80, 24)));
        assert!(size_changed((80, 24), (81, 24)));
        assert!(size_changed((80, 24), (80, 25)));
        assert!(size_changed((MIN_COLS, MIN_ROWS), (80, 24)));
    }
}
