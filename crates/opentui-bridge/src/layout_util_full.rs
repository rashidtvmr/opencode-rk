#![forbid(unsafe_code)]
//! Full chrome row-split + centering helpers (companion to `sibling_margin.rs`).
//!
//! TS truth `packages/tui/src/util/layout.ts:1-25` covers only the
//! pre-layout sibling margin (ported in `sibling_margin.rs`); this file adds
//! the pure chrome geometry every screen needs: clamp header/footer into the
//! frame height, hand the rest to the body, center dialogs in the width.
//! Boundary: pure fns, no Yoga/renderable access.

/// Max header rows kept before clipping (fail-closed chrome bound).
pub const MAX_HEADER_ROWS: usize = 4;
/// Max footer rows kept before clipping.
pub const MAX_FOOTER_ROWS: usize = 4;
/// Max combined chrome rows (header + footer) reserved from any frame.
pub const MAX_CHROME_ROWS: usize = 8;

/// Split `height` into clamped `(header, body, footer)` rows.
/// Header wins over footer; body takes the remainder (possibly 0).
#[must_use]
pub fn split_rows(height: usize, header: usize, footer: usize) -> (usize, usize, usize) {
    let h = header.min(MAX_HEADER_ROWS).min(height);
    let f = footer.min(MAX_FOOTER_ROWS).min(height.saturating_sub(h));
    let f = f.min(MAX_CHROME_ROWS.saturating_sub(h));
    (h, height.saturating_sub(h + f), f)
}

/// Left padding to center `inner` cols inside `width` cols (floor, 0 if over).
#[must_use]
pub const fn center_in(width: usize, inner: usize) -> usize {
    if inner >= width {
        0
    } else {
        (width - inner) / 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_even_fit() {
        assert_eq!(split_rows(10, 2, 2), (2, 6, 2));
    }

    #[test]
    fn header_clamps_to_height() {
        assert_eq!(split_rows(2, 9, 1), (2, 0, 0));
    }

    #[test]
    fn footer_yields_to_header_and_caps() {
        assert_eq!(split_rows(6, 4, 4), (4, 0, 2));
        assert_eq!(split_rows(0, 1, 1), (0, 0, 0));
    }

    #[test]
    fn center_exact_and_narrow() {
        assert_eq!(center_in(80, 80), 0);
        assert_eq!(center_in(10, 20), 0);
    }

    #[test]
    fn center_pads_floor() {
        assert_eq!(center_in(80, 60), 10);
        assert_eq!(center_in(11, 10), 0);
    }
}
