#![forbid(unsafe_code)]
//! Terminal shell regions derived from viewport dimensions.

use crate::layout::{
    sidebar_width, split_col, split_row, Constraint, Rect, MIN_FULL_HEIGHT, MIN_FULL_WIDTH,
};
use crate::world::ShellRegions;

fn empty(compact: bool) -> ShellRegions {
    let zero = Rect::new(0, 0, 0, 0);
    ShellRegions::new(zero, zero, zero, zero, compact)
}

/// Build disjoint shell regions for a terminal viewport.
///
/// A zero-sized viewport returns empty regions. Full layouts reserve a
/// five-cell composer and one-cell status; narrow or short viewports use a
/// three-cell composer and collapse the sidebar.
#[must_use]
pub fn shell_layout(cols: u32, rows: u32, sidebar: bool) -> ShellRegions {
    if cols == 0 || rows == 0 {
        return empty(true);
    }

    let compact = cols < MIN_FULL_WIDTH || rows < MIN_FULL_HEIGHT;
    let content = Rect::new(0, 0, cols, rows.saturating_sub(1));
    let composer_height = if compact { 3 } else { 5 };
    let vertical = match split_col(
        content,
        &[Constraint::Flex(1), Constraint::Fixed(composer_height)],
    ) {
        Ok(parts) if parts.len() == 2 => parts,
        _ => return empty(compact),
    };
    let body = vertical[0];
    let composer = vertical[1];

    let (transcript, sidebar_rect) = if sidebar && !compact {
        let side = sidebar_width(cols);
        match split_row(body, &[Constraint::Flex(1), Constraint::Fixed(side)]) {
            Ok(parts) if parts.len() == 2 => (parts[0], parts[1]),
            _ => return empty(compact),
        }
    } else {
        (body, Rect::new(0, 0, 0, 0))
    };

    let status = Rect::new(0, rows.saturating_sub(1), cols, 1);
    ShellRegions::new(transcript, composer, sidebar_rect, status, compact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_layout_has_transcript_sidebar_composer_status() {
        let regions = shell_layout(120, 40, true);
        assert!(!regions.compact);
        assert_eq!(regions.transcript, Rect::new(0, 0, 90, 34));
        assert_eq!(regions.sidebar, Rect::new(90, 0, 30, 34));
        assert_eq!(regions.composer, Rect::new(0, 34, 120, 5));
        assert_eq!(regions.status, Rect::new(0, 39, 120, 1));
    }

    #[test]
    fn compact_width_collapses_sidebar() {
        let regions = shell_layout(79, 40, true);
        assert!(regions.compact);
        assert!(regions.sidebar.is_empty());
        assert_eq!(regions.transcript.w, 79);
        assert_eq!(regions.composer.h, 3);
    }

    #[test]
    fn zero_size_fails_closed() {
        let regions = shell_layout(0, 40, true);
        assert!(regions.paint_calls().is_empty());
        assert!(regions.transcript.is_empty());
        assert!(regions.composer.is_empty());
        assert!(regions.sidebar.is_empty());
        assert!(regions.status.is_empty());
        assert!(shell_layout(120, 0, true).paint_calls().is_empty());
    }

    #[test]
    fn hidden_sidebar_uses_transcript_width() {
        let regions = shell_layout(120, 40, false);
        assert!(!regions.compact);
        assert!(regions.sidebar.is_empty());
        assert_eq!(regions.transcript.w, 120);
        assert_eq!(regions.composer.h, 5);
    }

    #[test]
    fn regions_are_disjoint_in_full_layout() {
        let regions = shell_layout(120, 40, true);
        assert!(!regions.has_overlap());
        assert_eq!(regions.paint_calls().len(), 4);
    }

    #[test]
    fn regions_are_disjoint_in_compact_layout() {
        let regions = shell_layout(40, 10, true);
        assert!(regions.compact);
        assert!(!regions.has_overlap());
        assert_eq!(regions.paint_calls().len(), 3);
    }
}
