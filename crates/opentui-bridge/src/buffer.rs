//! Cell buffer + native buffer FFI (`TUI-001` slice).
//!
//! SOURCE EVIDENCE (upstream, read-only):
//! - Entry points: `packages/native/src/lib.zig` (`bufferDrawText:1800`,
//!   `bufferSetCell:1817`, `bufferFillRect:1827`, `bufferDrawBox:2015-2029`
//!   14 params, `bufferClear:1740`, `bufferResize:2064`,
//!   `createOptimizedBuffer:1444`, `destroyOptimizedBuffer:1472`,
//!   `bufferWriteResolvedChars:1791`,
//!   `bufferDrawGrid:1990` (borderChars/borderFg/borderBg/offsets/options),
//!   `getBufferWidth:1371`, `getBufferHeight:1376`).
//! - Pack layout: `packages/core/src/buffer.ts:18-52` (`packDrawOptions`);
//!   decoded in `lib.zig:2032-2041` (sides bit0 left/bit1 bottom/bit2
//!   right/bit3 top, fill bit 4, title bits 5-6, bottom-title bits 7-8).
//! - Char sets: `packages/core/src/lib/border.ts:39-92` (`BorderChars`,
//!   11 entries per style) via `borderCharsToArray` (`border.ts:148-162`);
//!   index order topLeft, topRight, bottomLeft, bottomRight, horizontal,
//!   vertical, topT, bottomT, leftT, rightT, cross.

/// Opaque native buffer/renderer reference. `0` is always invalid.
pub type NativeHandle = u32;

/// Cell buffer placeholder (no unsafe yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellBuffer {
    pub cols: u32,
    pub rows: u32,
}

impl CellBuffer {
    pub const fn new(cols: u32, rows: u32) -> Self {
        Self { cols, rows }
    }

    /// Copy `src` cells into `self` at `(dx, dy)`, clipped to dst bounds.
    /// Pure memory geometry, no FFI/unsafe.
    /// ponytail: no cell store yet (`size_of==8` stable, backing native);
    /// upgrade path: add `cells: Vec<Cell>`, `copy_within`/row loop here.
    /// Self-blit safe: geometry only, aliasing is no-op safe.
    pub fn blit(&mut self, src: &CellBuffer, dx: u32, dy: u32) {
        let _ = self.blit_region(src, dx, dy);
        // No store to copy yet; region math above is the contract.
    }

    /// Clipped copy region `(w, h)` for [`CellBuffer::blit`].
    /// `None` when fully outside bounds or either side empty.
    #[must_use]
    pub fn blit_region(&self, src: &CellBuffer, dx: u32, dy: u32) -> Option<(u32, u32)> {
        if src.cols == 0 || src.rows == 0 || self.cols == 0 || self.rows == 0 {
            return None;
        }
        if dx >= self.cols || dy >= self.rows {
            return None;
        }
        let w = src.cols.min(self.cols - dx);
        let h = src.rows.min(self.rows - dy);
        if w == 0 || h == 0 {
            None
        } else {
            Some((w, h))
        }
    }
}

/// Packed RGBA color used by [`Cell`] (see [`crate::color`]).
pub use crate::color::RgbaU16;

/// One styled terminal cell. `ch` is a single Unicode scalar; wide glyphs are
/// stored in the leading cell and the trailing cell is left blank, matching the
/// native cell layout (`char` + fg + bg lanes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cell {
    pub ch: char,
    pub fg: RgbaU16,
    pub bg: RgbaU16,
    pub attrs: u32,
}

impl Cell {
    /// Space on the terminal default colors, no attributes.
    pub const BLANK: Cell = Cell {
        ch: ' ',
        fg: [255, 255, 255, 255],
        bg: [0, 0, 0, 0],
        attrs: 0,
    };

    #[must_use]
    pub const fn new(ch: char, fg: RgbaU16, bg: RgbaU16) -> Self {
        Self { ch, fg, bg, attrs: 0 }
    }

    #[must_use]
    pub const fn with_attrs(mut self, attrs: u32) -> Self {
        self.attrs = attrs;
        self
    }

    /// Space with the default background.
    #[must_use]
    pub const fn is_blank(self) -> bool {
        self.ch == ' ' && self.attrs == 0
    }
}

/// Owned cell surface used for Rust-side scene composition and cell-level
/// assertions. Distinct from the size-frozen [`CellBuffer`] descriptor: this is
/// the actual store, with an explicit clip stack mirroring native
/// `bufferPushScissorRect`/`bufferPopScissorRect`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellGrid {
    width: u32,
    height: u32,
    cells: Vec<Cell>,
    clips: Vec<crate::layout::Rect>,
}

impl CellGrid {
    /// Blank grid. `width`/`height` may be 0 (all writes then fail closed).
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let len = width as usize * height as usize;
        Self {
            width,
            height,
            cells: vec![Cell::BLANK; len],
            clips: Vec::new(),
        }
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Immutable cell storage in row-major order.
    #[must_use]
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// Full grid bounds.
    #[must_use]
    pub const fn bounds(&self) -> crate::layout::Rect {
        crate::layout::Rect::new(0, 0, self.width, self.height)
    }

    /// Intersection of every pushed clip with the grid bounds. Writes outside
    /// this rectangle are dropped, exactly like native scissor clipping.
    #[must_use]
    pub fn clip(&self) -> crate::layout::Rect {
        let mut clip = self.bounds();
        for pushed in &self.clips {
            let x0 = clip.x.max(pushed.x);
            let y0 = clip.y.max(pushed.y);
            let x1 = clip
                .x
                .saturating_add(clip.w)
                .min(pushed.x.saturating_add(pushed.w));
            let y1 = clip
                .y
                .saturating_add(clip.h)
                .min(pushed.y.saturating_add(pushed.h));
            clip = crate::layout::Rect::new(x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0));
        }
        clip
    }

    /// Push a scissor rectangle; it intersects with the current clip.
    pub fn push_clip(&mut self, rect: crate::layout::Rect) {
        self.clips.push(rect);
    }

    /// Pop the last scissor rectangle. Returns `false` when none was pushed.
    pub fn pop_clip(&mut self) -> bool {
        self.clips.pop().is_some()
    }

    /// Read a cell; `None` outside the grid (clip is not consulted).
    #[must_use]
    pub fn get(&self, x: u32, y: u32) -> Option<Cell> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.cells.get(y as usize * self.width as usize + x as usize).copied()
    }

    /// Write one cell. Returns `false` (and writes nothing) outside the clip
    /// or the grid.
    pub fn set(&mut self, x: u32, y: u32, cell: Cell) -> bool {
        let clip = self.clip();
        let inside = x >= clip.x
            && y >= clip.y
            && x < clip.x.saturating_add(clip.w)
            && y < clip.y.saturating_add(clip.h);
        if !inside || x >= self.width || y >= self.height {
            return false;
        }
        let index = y as usize * self.width as usize + x as usize;
        self.cells[index] = cell;
        true
    }

    /// Fill `rect` with `cell`, clipped. Returns the number of cells written.
    pub fn fill(&mut self, rect: crate::layout::Rect, cell: Cell) -> u32 {
        let clip = self.clip();
        let x0 = rect.x.max(clip.x);
        let y0 = rect.y.max(clip.y);
        let x1 = rect
            .x
            .saturating_add(rect.w)
            .min(clip.x.saturating_add(clip.w))
            .min(self.width);
        let y1 = rect
            .y
            .saturating_add(rect.h)
            .min(clip.y.saturating_add(clip.h))
            .min(self.height);
        let mut written = 0;
        let mut y = y0;
        while y < y1 {
            let mut x = x0;
            while x < x1 {
                if self.set(x, y, cell) {
                    written += 1;
                }
                x += 1;
            }
            y += 1;
        }
        written
    }

    /// Copy all cells of `src` to `(dx, dy)`, clipped by this grid's clip and
    /// bounds. Self-blit is rejected (`false`) to avoid aliasing surprises.
    pub fn blit(&mut self, src: &CellGrid, dx: u32, dy: u32) -> bool {
        if std::ptr::eq(self, src) {
            return false;
        }
        let mut y = 0;
        while y < src.height {
            let mut x = 0;
            while x < src.width {
                if let Some(cell) = src.get(x, y) {
                    let _ = self.set(dx.saturating_add(x), dy.saturating_add(y), cell);
                }
                x += 1;
            }
            y += 1;
        }
        true
    }

    /// Row text with clips ignored (full grid), for cell-level assertions.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::with_capacity(self.height as usize);
        for y in 0..self.height {
            let mut line = String::with_capacity(self.width as usize);
            for x in 0..self.width {
                line.push(self.get(x, y).map_or(' ', |c| c.ch));
            }
            out.push(line);
        }
        out
    }
}

// Raw C-ABI buffer entry points. Every declaration matches an
// `export fn` in `packages/native/src/lib.zig`. Nullable Zig pointers
// (`?[*]`) map to raw pointers (null tolerated natively); colors are
// `[u16; 4]` RGBA lanes. Linked only under `native`.
#[cfg(feature = "native")]
#[link(name = "opentui")]
unsafe extern "C" {
    pub fn bufferDrawText(
        buffer_handle: NativeHandle,
        text: *const u8,
        textLen: u32,
        x: u32,
        y: u32,
        fg: *const u16,
        bg: *const u16,
        attributes: u32,
    );
    pub fn bufferSetCell(
        buffer_handle: NativeHandle,
        x: u32,
        y: u32,
        char: u32,
        fg: *const u16,
        bg: *const u16,
        attributes: u32,
    );
    pub fn bufferFillRect(
        buffer_handle: NativeHandle,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        bg: *const u16,
    );
    pub fn bufferDrawBox(
        buffer_handle: NativeHandle,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        borderChars: *const u32,
        packedOptions: u32,
        borderColor: *const u16,
        backgroundColor: *const u16,
        titleColor: *const u16,
        title: *const u8,
        titleLen: u32,
        bottomTitle: *const u8,
        bottomTitleLen: u32,
    );
    pub fn bufferClear(buffer_handle: NativeHandle, bg: *const u16);
    pub fn bufferResize(buffer_handle: NativeHandle, width: u32, height: u32);
    pub fn createOptimizedBuffer(
        width: u32,
        height: u32,
        respectAlpha: u8,
        widthMethod: u8,
        idPtr: *const u8,
        idLen: u32,
    ) -> NativeHandle;
    pub fn destroyOptimizedBuffer(buffer_handle: NativeHandle);
    pub fn bufferWriteResolvedChars(
        buffer_handle: NativeHandle,
        outputPtr: *mut u8,
        outputLen: u32,
        addLineBreaks: bool,
    ) -> u32;
    pub fn bufferDrawGrid(
        buffer_handle: NativeHandle,
        borderChars: *const u32,
        borderFg: *const u16,
        borderBg: *const u16,
        columnOffsets: *const i32,
        columnCount: u32,
        rowOffsets: *const i32,
        rowCount: u32,
        options: *const GridDrawOptions,
    );
    pub fn getBufferWidth(buffer_handle: NativeHandle) -> u32;
    pub fn getBufferHeight(buffer_handle: NativeHandle) -> u32;
}

/// `lib.zig:1985` `ExternalGridDrawOptions` (`draw_inner`, `draw_outer`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridDrawOptions {
    pub draw_inner: bool,
    pub draw_outer: bool,
}

/// Slot count of a border-char array. Matches TS `borderCharsToArray`
/// (11 slots) and Zig `BorderCharIndex` 0..=10.
pub const BORDER_CHARS_LEN: usize = 11;

/// Border look. Char sets mirror `border.ts:39-92`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum BorderStyle {
    /// TS `single` (also the `parseBorderStyle` fallback).
    #[default]
    Single,
    /// TS `double`.
    Double,
    /// TS `rounded` (single edges, rounded corners).
    Rounded,
    /// TS `heavy`.
    Heavy,
}

/// Title placement. Encodes as 0/1/2 (`buffer.ts:40-48` alignmentMap).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum TitleAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl TitleAlign {
    const fn encode(self) -> u32 {
        match self {
            Self::Left => 0,
            Self::Center => 1,
            Self::Right => 2,
        }
    }
}

/// Drawn sides. Bit layout mirrors `buffer.ts:27-34` /
/// `lib.zig:2033-2036`: bit0 left, bit1 bottom, bit2 right, bit3 top.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct BorderSides(pub u8);

impl BorderSides {
    pub const NONE: Self = Self(0);
    pub const LEFT: Self = Self(1);
    pub const BOTTOM: Self = Self(2);
    pub const RIGHT: Self = Self(4);
    pub const TOP: Self = Self(8);
    pub const ALL: Self = Self(0b1111);

    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }
}

impl std::ops::BitOr for BorderSides {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Pack draw options into the `packedOptions: u32` the native entrypoint
/// decodes (`buffer.ts:19-52`, `lib.zig:2032-2041`).
#[must_use]
pub const fn pack_options(
    sides: BorderSides,
    filled: bool,
    title: TitleAlign,
    bottom: TitleAlign,
) -> u32 {
    (sides.0 as u32) | ((filled as u32) << 4) | (title.encode() << 5) | (bottom.encode() << 7)
}

/// Border codepoints in `borderCharsToArray` order (topLeft, topRight,
/// bottomLeft, bottomRight, horizontal, vertical, topT, bottomT, leftT,
/// rightT, cross). Values are `codePointAt(0)` of the `border.ts` glyphs.
#[must_use]
pub const fn border_chars(style: BorderStyle) -> [u32; BORDER_CHARS_LEN] {
    match style {
        // single: ┌ ┐ └ ┘ ─ │ ┬ ┴ ├ ┤ ┼
        BorderStyle::Single => [
            9484, 9488, 9492, 9496, 9472, 9474, 9516, 9524, 9500, 9508, 9532,
        ],
        // double: ╔ ╗ ╚ ╝ ═ ║ ╦ ╩ ╠ ╣ ╬
        BorderStyle::Double => [
            9556, 9559, 9562, 9565, 9552, 9553, 9574, 9577, 9568, 9571, 9580,
        ],
        // rounded: ╭ ╮ ╰ ╯ + single edges/tees/cross
        BorderStyle::Rounded => [
            9581, 9582, 9584, 9583, 9472, 9474, 9516, 9524, 9500, 9508, 9532,
        ],
        // heavy: ┏ ┓ ┗ ┛ ━ ┃ ┳ ┻ ┣ ┫ ╋
        BorderStyle::Heavy => [
            9487, 9491, 9495, 9499, 9473, 9475, 9523, 9531, 9507, 9515, 9547,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn cell_buffer_size_stable() {
        assert_eq!(size_of::<CellBuffer>(), 8);
        assert_eq!(CellBuffer::new(10, 5).rows, 5);
    }

    #[test]
    fn side_bits_match_zig_layout() {
        // buffer.ts:30-33 / lib.zig:2033-2036.
        assert_eq!(BorderSides::LEFT.bits(), 0b0001);
        assert_eq!(BorderSides::BOTTOM.bits(), 0b0010);
        assert_eq!(BorderSides::RIGHT.bits(), 0b0100);
        assert_eq!(BorderSides::TOP.bits(), 0b1000);
    }

    #[test]
    fn pack_options_match_ts() {
        // buffer.ts packDrawOptions: all sides, no fill, left/left.
        assert_eq!(
            pack_options(BorderSides::ALL, false, TitleAlign::Left, TitleAlign::Left),
            0b1111
        );
        // border=true + fill + center title + right bottom title.
        assert_eq!(
            pack_options(
                BorderSides::ALL,
                true,
                TitleAlign::Center,
                TitleAlign::Right
            ),
            0b1111 | (1 << 4) | (1 << 5) | (2 << 7)
        );
        // single side, no fill.
        assert_eq!(
            pack_options(BorderSides::TOP, false, TitleAlign::Left, TitleAlign::Left),
            0b1000
        );
    }

    #[test]
    fn border_chars_match_ts_glyphs() {
        // ┌ U+250C, ┼ U+253C; ╔ U+2554; ╭ U+256D; ┏ U+250F.
        assert_eq!(border_chars(BorderStyle::Single)[0], 0x250C);
        assert_eq!(border_chars(BorderStyle::Single)[10], 0x253C);
        assert_eq!(border_chars(BorderStyle::Double)[0], 0x2554);
        assert_eq!(border_chars(BorderStyle::Rounded)[0], 0x256D);
        assert_eq!(border_chars(BorderStyle::Heavy)[10], 0x254B);
        // Rounded reuses single edges/tees/cross.
        assert_eq!(
            &border_chars(BorderStyle::Rounded)[4..],
            &border_chars(BorderStyle::Single)[4..]
        );
    }

    #[test]
    fn blit_region_clip_offset() {
        let dst = CellBuffer::new(10, 5);
        let src = CellBuffer::new(4, 3);
        assert_eq!(dst.blit_region(&src, 0, 0), Some((4, 3))); // full fit
        assert_eq!(dst.blit_region(&src, 2, 1), Some((4, 3))); // offset fit
        assert_eq!(dst.blit_region(&src, 8, 4), Some((2, 1))); // clip right/bottom
        assert_eq!(dst.blit_region(&src, 10, 0), None); // dx outside
        assert_eq!(dst.blit_region(&src, 0, 5), None); // dy outside
        assert_eq!(dst.blit_region(&CellBuffer::new(0, 3), 0, 0), None); // empty src
        assert_eq!(CellBuffer::new(0, 0).blit_region(&src, 0, 0), None); // empty dst
    }

    #[test]
    fn blit_no_panic_self_blit() {
        let mut dst = CellBuffer::new(10, 5);
        let src = CellBuffer::new(4, 3);
        dst.blit(&src, 8, 4); // clipped, no panic
        dst.blit(&src, 20, 20); // fully outside, no-op
        assert_eq!((dst.cols, dst.rows), (10, 5)); // dims unchanged
        let mut same = CellBuffer::new(6, 4);
        let copy = same; // CellBuffer: Copy, models alias-safe self-blit
        same.blit(&copy, 1, 1);
        assert_eq!((same.cols, same.rows), (6, 4));
    }

    #[test]
    fn cell_grid_set_get_bounds() {
        let mut grid = CellGrid::new(4, 2);
        let cell = Cell::new('x', [1, 2, 3, 255], [0, 0, 0, 0]);
        assert!(grid.set(3, 1, cell));
        assert_eq!(grid.get(3, 1), Some(cell));
        assert!(!grid.set(4, 1, cell), "x out of bounds");
        assert!(!grid.set(0, 2, cell), "y out of bounds");
        assert_eq!(grid.get(4, 1), None);
        // Untouched cells stay blank.
        assert_eq!(grid.get(0, 0), Some(Cell::BLANK));
        assert_eq!(grid.lines(), vec!["    ".to_owned(), "   x".to_owned()]);
        let empty = CellGrid::new(0, 0);
        assert_eq!(empty.clip().w, 0);
        assert!(empty.lines().is_empty());
    }

    #[test]
    fn cell_grid_clip_intersects_and_pops() {
        let mut grid = CellGrid::new(6, 3);
        grid.push_clip(crate::layout::Rect::new(1, 1, 3, 2));
        assert_eq!(grid.clip(), crate::layout::Rect::new(1, 1, 3, 2));
        // Nested clip intersects the outer one.
        grid.push_clip(crate::layout::Rect::new(2, 0, 10, 10));
        assert_eq!(grid.clip(), crate::layout::Rect::new(2, 1, 2, 2));
        assert!(!grid.set(1, 1, Cell::BLANK), "left of nested clip");
        assert!(grid.set(2, 1, Cell::new('a', [0; 4], [0; 4])));
        assert!(grid.pop_clip());
        assert!(grid.set(1, 1, Cell::new('b', [0; 4], [0; 4])));
        assert!(grid.pop_clip());
        assert!(!grid.pop_clip(), "empty stack");
        assert_eq!(grid.clip(), grid.bounds());
        assert_eq!(grid.lines()[1], " ba   ");
    }

    #[test]
    fn cell_grid_fill_and_blit_clip() {
        let mut grid = CellGrid::new(5, 2);
        let block = Cell::new('#', [0; 4], [0; 4]);
        // Over-large fill is intersected with the grid.
        assert_eq!(grid.fill(crate::layout::Rect::new(2, 0, 10, 10), block), 6);
        assert_eq!(grid.lines(), vec!["  ###".to_owned(), "  ###".to_owned()]);

        let mut src = CellGrid::new(3, 2);
        src.fill(crate::layout::Rect::new(0, 0, 3, 2), Cell::new('s', [0; 4], [0; 4]));
        let mut dst = CellGrid::new(4, 4);
        assert!(dst.blit(&src, 2, 3));
        assert_eq!(dst.lines()[3], "  ss");
        // `blit` cannot receive its own storage safely (aliasing `&mut self`
        // and `&src`), so the reject-on-self branch is a defensive guard only.
    }

    #[test]
    fn cell_helpers_are_stable() {
        assert!(Cell::BLANK.is_blank());
        assert!(!Cell::new('a', [0; 4], [0; 4]).is_blank());
        let styled = Cell::BLANK.with_attrs(4);
        assert_eq!(styled.attrs, 4);
        assert!(!styled.is_blank(), "attributes make a space meaningful");
        assert_eq!(Cell::new('x', [0; 4], [0; 4]).ch, 'x');
    }
}
