#![forbid(unsafe_code)]
//! Legacy style paintbox state.
//!
//! Pure state only: bounded character grid with per-cell style built by the
//! caller from legacy paint events. No rendering, no IO, no threads; the
//! caller executes all drawing against the real terminal.
//!
//! Bounds: grid area (`w * h`) is capped at [`MAX_CELLS`]; `set`/`get`
//! reject out-of-bounds coordinates with an error instead of panicking.

/// Maximum cells retained in one grid.
pub const MAX_CELLS: usize = 10_000;

/// Default fill character used by [`PaintGrid::new`] and [`PaintGrid::clear`].
pub const DEFAULT_FILL: char = ' ';

/// Legacy paint style for a cell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PaintStyle {
    /// No decoration.
    #[default]
    Plain,
    /// Cell is part of a border.
    Bordered,
    /// Dimmed cell.
    Dim,
}

/// One styled character cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaintCell {
    /// Grapheme placeholder (single `char` by legacy contract).
    pub ch: char,
    /// Legacy style for this cell.
    pub style: PaintStyle,
}

impl Default for PaintCell {
    fn default() -> Self {
        Self {
            ch: DEFAULT_FILL,
            style: PaintStyle::Plain,
        }
    }
}

impl PaintCell {
    /// Build a cell from parts.
    #[must_use]
    pub fn new(ch: char, style: PaintStyle) -> Self {
        Self { ch, style }
    }
}

/// Error for grid construction and cell access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintError {
    /// Coordinates outside the grid.
    OutOfBounds {
        /// Requested x.
        x: usize,
        /// Requested y.
        y: usize,
        /// Grid width.
        w: usize,
        /// Grid height.
        h: usize,
    },
    /// Requested area exceeds [`MAX_CELLS`].
    TooLarge {
        /// Requested width.
        w: usize,
        /// Requested height.
        h: usize,
        /// Cell budget.
        max: usize,
    },
    /// Zero-sized grid.
    Empty,
}

impl core::fmt::Display for PaintError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::OutOfBounds { x, y, w, h } => {
                write!(f, "paint coords ({x},{y}) out of bounds {w}x{h}")
            }
            Self::TooLarge { w, h, max } => {
                write!(f, "paint grid {w}x{h} exceeds cell budget {max}")
            }
            Self::Empty => write!(f, "paint grid must be non-empty"),
        }
    }
}

impl std::error::Error for PaintError {}

/// Bounded styled-character grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaintGrid {
    w: usize,
    h: usize,
    cells: Vec<PaintCell>,
}

impl PaintGrid {
    /// Build a `w` x `h` grid filled with [`PaintCell::default`].
    ///
    /// # Errors
    /// Returns [`PaintError::Empty`] for zero area and
    /// [`PaintError::TooLarge`] when `w * h` exceeds [`MAX_CELLS`].
    pub fn new(w: usize, h: usize) -> Result<Self, PaintError> {
        if w == 0 || h == 0 {
            return Err(PaintError::Empty);
        }
        let area = w.saturating_mul(h);
        if area > MAX_CELLS {
            return Err(PaintError::TooLarge {
                w,
                h,
                max: MAX_CELLS,
            });
        }
        Ok(Self {
            w,
            h,
            cells: vec![PaintCell::default(); area],
        })
    }

    /// Grid width in cells.
    #[must_use]
    pub fn width(&self) -> usize {
        self.w
    }

    /// Grid height in cells.
    #[must_use]
    pub fn height(&self) -> usize {
        self.h
    }

    /// Cell count (`w * h`, always `<= MAX_CELLS`).
    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// True when the grid holds no cells (unreachable via `new`).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    fn index(&self, x: usize, y: usize) -> Result<usize, PaintError> {
        if x >= self.w || y >= self.h {
            return Err(PaintError::OutOfBounds {
                x,
                y,
                w: self.w,
                h: self.h,
            });
        }
        Ok(y.saturating_mul(self.w).saturating_add(x))
    }

    /// Write one cell.
    ///
    /// # Errors
    /// Returns [`PaintError::OutOfBounds`] when `(x, y)` is outside the grid.
    pub fn set(&mut self, x: usize, y: usize, cell: PaintCell) -> Result<(), PaintError> {
        let i = self.index(x, y)?;
        self.cells[i] = cell;
        Ok(())
    }

    /// Read one cell.
    ///
    /// # Errors
    /// Returns [`PaintError::OutOfBounds`] when `(x, y)` is outside the grid.
    pub fn get(&self, x: usize, y: usize) -> Result<PaintCell, PaintError> {
        let i = self.index(x, y)?;
        Ok(self.cells[i])
    }

    /// Reset every cell to [`PaintCell::default`].
    pub fn clear(&mut self) {
        self.cells.fill(PaintCell::default());
    }
}

/// Whole-shell page renderer over [`PaintGrid`] (TUI-003 companion).
///
/// Pure state only: no IO, no threads. Mirrors
/// `crate::native_app::{AppView, ShellLayout}` compact rule (viewport below
/// 80x24 forces single column); sidebar divider drawn only in full mode.
use crate::native_app::AppView;

/// Full page: header with view label, body, stale/live status line.
/// Sidebar divider (`│`) only when `!compact && w >= 80 && h >= 24`.
/// Area capped at [`MAX_CELLS`] via [`PaintGrid::new`].
pub fn paint_view(view: AppView, w: usize, h: usize) -> Result<PaintGrid, PaintError> {
    let mut grid = PaintGrid::new(w, h)?;
    paint_into(&mut grid, view, false);
    Ok(grid)
}

/// Compact page: single column, never draws the sidebar divider.
pub fn paint_compact(view: AppView, w: usize, h: usize) -> Result<PaintGrid, PaintError> {
    let mut grid = PaintGrid::new(w, h)?;
    paint_into(&mut grid, view, true);
    Ok(grid)
}

fn blit(grid: &mut PaintGrid, x: usize, y: usize, s: &str, style: PaintStyle) {
    for (i, ch) in s.chars().enumerate() {
        if grid.set(x + i, y, PaintCell::new(ch, style)).is_err() {
            break;
        }
    }
}

fn paint_into(grid: &mut PaintGrid, view: AppView, compact: bool) {
    let (w, h) = (grid.width(), grid.height());
    blit(grid, 0, 0, &format!("opencode-rk {}", view.label()), PaintStyle::Dim);
    let full = !compact && w >= 80 && h >= 24;
    if full {
        let side_w = 30.min(w / 3).max(20.min(w));
        let dx = w.saturating_sub(side_w);
        for y in 1..h.saturating_sub(1) {
            let _ = grid.set(dx, y, PaintCell::new('│', PaintStyle::Bordered));
        }
        blit(grid, dx + 2, 1, "sidebar", PaintStyle::Dim);
    }
    let body: &[&str] = match view {
        AppView::Empty => &["create or open a session"],
        AppView::Loading => &["waiting for daemon..."],
        AppView::Offline => &["reconnect to daemon"],
        AppView::Error => &["retry or dismiss"],
        AppView::Actionable => &["transcript", "composer"],
    };
    for (i, line) in body.iter().enumerate() {
        if 1 + i < h.saturating_sub(1) {
            blit(grid, 1, 1 + i, line, PaintStyle::Plain);
        }
    }
    if h > 1 {
        let badge = if view.is_actionable() { "live" } else { "stale" };
        let status = match view.action_hint() {
            Some(hint) => format!("{badge} | {hint}"),
            None => badge.to_owned(),
        };
        blit(grid, 0, h - 1, &status, PaintStyle::Dim);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_app::AppView;

    #[test]
    fn oob_access_errors() {
        let mut grid = PaintGrid::new(2, 2).expect("small grid builds");
        let cell = PaintCell::new('#', PaintStyle::Bordered);
        assert!(grid.set(2, 0, cell).is_err());
        assert!(grid.set(0, 2, cell).is_err());
        assert!(grid.get(2, 0).is_err());
        assert!(grid.get(0, 2).is_err());
        // Failed writes leave defaults intact.
        assert_eq!(grid.get(0, 0), Ok(PaintCell::default()));
    }

    #[test]
    fn clear_empties_cells() {
        let mut grid = PaintGrid::new(3, 2).expect("small grid builds");
        grid.set(0, 0, PaintCell::new('x', PaintStyle::Dim))
            .expect("in-bounds write");
        grid.set(2, 1, PaintCell::new('y', PaintStyle::Bordered))
            .expect("in-bounds write");
        grid.clear();
        for y in 0..grid.height() {
            for x in 0..grid.width() {
                assert_eq!(grid.get(x, y), Ok(PaintCell::default()), "{x},{y}");
            }
        }
    }

    #[test]
    fn grid_size_capped() {
        assert_eq!(PaintGrid::new(0, 4), Err(PaintError::Empty));
        assert_eq!(PaintGrid::new(4, 0), Err(PaintError::Empty));
        // 101 * 100 = 10100 > MAX_CELLS.
        let err = PaintGrid::new(101, 100).expect_err("over-budget grid refused");
        assert_eq!(
            err,
            PaintError::TooLarge {
                w: 101,
                h: 100,
                max: MAX_CELLS
            }
        );
        // Exact budget still builds.
        assert!(PaintGrid::new(100, 100).is_ok());
    }

    // RED: page rendering over PaintGrid (all AppViews + compact).

    fn text_of(grid: &PaintGrid) -> String {
        let mut out = String::new();
        for y in 0..grid.height() {
            for x in 0..grid.width() {
                out.push(grid.get(x, y).map(|c| c.ch).unwrap_or('?'));
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn paint_view_covers_all_app_views_bounded() {
        for view in [
            AppView::Empty,
            AppView::Loading,
            AppView::Offline,
            AppView::Error,
            AppView::Actionable,
        ] {
            let grid = paint_view(view, 80, 24).expect("80x24 paints");
            assert!(grid.len() <= MAX_CELLS, "{view:?} exceeds budget");
            let text = text_of(&grid);
            assert!(
                text.contains(view.label()),
                "{view:?} page must show its label"
            );
        }
    }

    #[test]
    fn paint_compact_drops_sidebar_divider() {
        let compact = paint_compact(AppView::Actionable, 40, 10).expect("40x10 paints");
        assert!(compact.len() <= MAX_CELLS);
        assert!(
            !text_of(&compact).contains('│'),
            "compact must be single-column"
        );
        let full = paint_view(AppView::Actionable, 100, 30).expect("100x30 paints");
        assert!(full.len() <= MAX_CELLS);
        assert!(
            text_of(&full).contains('│'),
            "full actionable page keeps sidebar divider"
        );
    }

    #[test]
    fn paint_pages_reject_bad_sizes() {
        assert_eq!(
            paint_view(AppView::Empty, 101, 100),
            Err(PaintError::TooLarge {
                w: 101,
                h: 100,
                max: MAX_CELLS
            })
        );
        assert_eq!(
            paint_compact(AppView::Empty, 0, 4),
            Err(PaintError::Empty)
        );
    }

    #[test]
    fn non_actionable_pages_never_read_live() {
        for view in [
            AppView::Empty,
            AppView::Loading,
            AppView::Offline,
            AppView::Error,
        ] {
            let grid = paint_view(view, 80, 24).expect("paints");
            let text = text_of(&grid);
            assert!(text.contains("stale"), "{view:?} must badge stale");
            assert!(!text.contains("live"), "{view:?} must never read as live");
        }
        let live = text_of(&paint_view(AppView::Actionable, 80, 24).expect("paints"));
        assert!(live.contains("live"), "actionable page badges live");
    }
}
