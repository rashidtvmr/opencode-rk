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

#[cfg(test)]
mod tests {
    use super::*;

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
}
