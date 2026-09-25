#![forbid(unsafe_code)]
//! Typed layout callers for the native terminal bridge.

use crate::frame_layout::shell_layout;
use crate::layout::{split_col, split_row, Constraint, Rect};

const POPUP_WIDTH: u32 = 60;
const POPUP_HEIGHT: u32 = 12;

fn empty() -> Rect {
    Rect::new(0, 0, 0, 0)
}

/// Return the three always-painted chat regions.
#[must_use]
pub fn chat_regions(cols: u32, rows: u32) -> (Rect, Rect, Rect) {
    let regions = shell_layout(cols, rows, false);
    (regions.transcript, regions.composer, regions.status)
}

/// Return the optional sidebar region, or `None` when the shell is compact.
#[must_use]
pub fn sidebar_regions(cols: u32, rows: u32) -> Option<Rect> {
    let regions = shell_layout(cols, rows, true);
    if regions.sidebar.is_empty() {
        None
    } else {
        Some(regions.sidebar)
    }
}

/// Return a centered 60x12 palette popup clamped to the viewport.
#[must_use]
pub fn palette_popup(cols: u32, rows: u32) -> Rect {
    if cols == 0 || rows == 0 {
        return empty();
    }

    let width = POPUP_WIDTH.min(cols);
    let height = POPUP_HEIGHT.min(rows);
    let left = (cols - width) / 2;
    let right = cols - width - left;
    let top = (rows - height) / 2;
    let bottom = rows - height - top;
    let area = Rect::new(0, 0, cols, rows);
    let vertical = match split_col(
        area,
        &[
            Constraint::Fixed(top),
            Constraint::Fixed(height),
            Constraint::Fixed(bottom),
        ],
    ) {
        Ok(parts) if parts.len() == 3 => parts,
        _ => return empty(),
    };
    let middle = vertical[1];
    match split_row(
        Rect::new(0, middle.y, cols, middle.h),
        &[
            Constraint::Fixed(left),
            Constraint::Fixed(width),
            Constraint::Fixed(right),
        ],
    ) {
        Ok(parts) if parts.len() == 3 => parts[1],
        _ => empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_rows_sum_to_viewport() {
        let (transcript, composer, status) = chat_regions(120, 40);
        assert_eq!(transcript.h + composer.h + status.h, 40);
        assert_eq!(transcript.w, 120);
        assert_eq!(status.h, 1);
    }

    #[test]
    fn zero_size_is_closed() {
        let (transcript, composer, status) = chat_regions(0, 0);
        assert!(transcript.is_empty());
        assert!(composer.is_empty());
        assert!(status.is_empty());
        assert!(sidebar_regions(120, 0).is_none());
        assert!(palette_popup(0, 40).is_empty());
    }

    #[test]
    fn sidebar_is_none_when_narrow() {
        assert!(sidebar_regions(79, 40).is_none());
        assert!(sidebar_regions(120, 23).is_none());
    }

    #[test]
    fn sidebar_is_present_when_wide() {
        let sidebar = sidebar_regions(120, 40).expect("full shell has sidebar");
        assert_eq!(sidebar, Rect::new(90, 0, 30, 34));
    }

    #[test]
    fn popup_is_clamped_to_viewport() {
        assert_eq!(palette_popup(40, 8), Rect::new(0, 0, 40, 8));
        assert_eq!(palette_popup(60, 12), Rect::new(0, 0, 60, 12));
    }

    #[test]
    fn popup_is_centered() {
        assert_eq!(palette_popup(100, 30), Rect::new(20, 9, 60, 12));
        assert_eq!(palette_popup(101, 31), Rect::new(20, 9, 60, 12));
    }
}
