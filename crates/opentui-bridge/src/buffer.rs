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

#![forbid(unsafe_code)]

/// Opaque native buffer/renderer reference. `0` is always invalid.
pub type NativeHandle = u32;

/// RGBA color (mirrors `packages/core/src/lib/RGBA.ts` and `color.rs::Rgba`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn transparent() -> Self {
        Self { r: 0, g: 0, b: 0, a: 0 }
    }

    /// Alpha-blend `fg` over `bg`: `out = fg*a + bg*(1-a)`. Opaque fg => fg.
    #[must_use]
    pub fn blend_over(fg: Rgba, bg: Rgba) -> Rgba {
        if fg.a == 255 {
            return fg;
        }
        if fg.a == 0 {
            return bg;
        }
        let a = fg.a as u32;
        let inv = 255u32 - a;
        let r = (fg.r as u32 * a + bg.r as u32 * inv) / 255;
        let g = (fg.g as u32 * a + bg.g as u32 * inv) / 255;
        let b = (fg.b as u32 * a + bg.b as u32 * inv) / 255;
        Rgba::new(r as u8, g as u8, b as u8, 255)
    }
}

/// A single cell in the buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// Unicode codepoint stored in this cell. For wide chars the second cell
    /// is a continuation with ch = 0 (modeled after TS `CHAR_FLAG_CONTINUATION`).
    pub ch: u32,
    pub fg: Rgba,
    pub bg: Rgba,
    pub attrs: u32,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: b' ' as u32,
            fg: Rgba::rgb(255, 255, 255),
            bg: Rgba::transparent(),
            attrs: 0,
        }
    }
}

/// Display width of a character: ASCII=1, EastAsian wide=2, combining=0, control=0.
/// Inline wcwidth-equivalent (no dependency).
#[must_use]
pub fn char_width(ch: char) -> u32 {
    let cp = ch as u32;
    // Control characters
    if cp < 0x20 || cp == 0x7f {
        return 0;
    }
    // Combining characters (Mn category ranges approximated):
    // U+0300-U+036F, U+1AB0-U+1AFF, U+1DC0-U+1DFF, U+20D0-U+20FF, U+FE20-U+FE2F
    if (0x300..=0x36f).contains(&cp)
        || (0x1ab0..=0x1aff).contains(&cp)
        || (0x1dc0..=0x1dff).contains(&cp)
        || (0x20d0..=0x20ff).contains(&cp)
        || (0xfe20..=0xfe2f).contains(&cp)
    {
        return 0;
    }
    // EastAsian wide: CJK ideographs, Hiragana, Katakana, Hangul, fullwidth
    if (0x1100..=0x115f).contains(&cp)
        || (0x2e80..=0x3034).contains(&cp)
        || (0x303f..=0x33ff).contains(&cp)
        || (0x3400..=0x4dbf).contains(&cp)
        || (0x4e00..=0xa4cf).contains(&cp)
        || (0xac00..=0xd7a3).contains(&cp)
        || (0xf900..=0xfaff).contains(&cp)
        || (0xfe10..=0xfe19).contains(&cp)
        || (0xfe30..=0xfe6f).contains(&cp)
        || (0xff00..=0xff5f).contains(&cp)
        || (0xffe0..=0xffe6).contains(&cp)
        || (0x1f300..=0x1f64f).contains(&cp)
        || (0x1f900..=0x1f9ff).contains(&cp)
        || (0x20000..=0x3fffd).contains(&cp)
    {
        return 2;
    }
    // Default: narrow/ambiguous treated as 1.
    1
}

// ---------------------------------------------------------------------------
// Scissor / opacity stacks (from buffer.ts:529-552)
// ---------------------------------------------------------------------------

/// A scissor rectangle in cell coordinates (inclusive start, exclusive end).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScissorRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl ScissorRect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    /// Whether `(cx, cy)` is inside this scissor (inclusive bounds).
    #[must_use]
    pub fn contains(self, cx: u32, cy: u32) -> bool {
        cx >= self.x && cy >= self.y && cx < self.x + self.width && cy < self.y + self.height
    }
}

/// Cell buffer with real cell store, scissor stack, and opacity stack.
pub struct CellBuffer {
    /// Native handle (0 = unbacked stub for pure-Rust use).
    pub handle: NativeHandle,
    pub cols: u32,
    pub rows: u32,
    /// Backing cell store: row-major, index = y * cols + x.
    pub cells: Vec<Cell>,
    /// Active scissor rectangles (intersected, like TS pushScissorRect).
    scissor: Vec<ScissorRect>,
    /// Active opacity multipliers (0.0..=1.0, applied during blending).
    opacity: Vec<f32>,
}

impl CellBuffer {
    pub const fn new(cols: u32, rows: u32) -> Self {
        Self {
            handle: 0,
            cols,
            rows,
            cells: Vec::new(),
            scissor: Vec::new(),
            opacity: Vec::new(),
        }
    }

    /// Number of cells in the grid.
    #[must_use]
    pub fn len(&self) -> usize {
        (self.cols * self.rows) as usize
    }

    /// Whether the grid holds zero cells.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Allocate the cell store filled with `Cell::default()`.
    pub fn init(&mut self) {
        let len = self.len();
        self.cells = vec![Cell::default(); len];
    }

    /// Flat cell index, or None if out of bounds.
    #[must_use]
    pub fn index_of(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.cols || y >= self.rows {
            return None;
        }
        Some((y * self.cols + x) as usize)
    }

    /// Whether `(x, y)` is inside the current scissor stack (intersection).
    #[must_use]
    pub fn in_scissor(&self, x: u32, y: u32) -> bool {
        self.scissor.iter().all(|r| r.contains(x, y))
    }

    /// Current effective opacity (product of stack, default 1.0).
    #[must_use]
    pub fn current_opacity(&self) -> f32 {
        self.opacity.iter().copied().fold(1.0f32, |acc, o| acc * o)
    }

    /// Push a scissor rectangle onto the stack.
    pub fn push_scissor_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.scissor.push(ScissorRect::new(x, y, width, height));
    }

    /// Pop a scissor rectangle from the stack.
    pub fn pop_scissor_rect(&mut self) -> Option<ScissorRect> {
        self.scissor.pop()
    }

    /// Clear all scissor rectangles.
    pub fn clear_scissor_rects(&mut self) {
        self.scissor.clear();
    }

    /// Push an opacity multiplier (clamped to 0.0..=1.0).
    pub fn push_opacity(&mut self, opacity: f32) {
        let clamped = opacity.clamp(0.0, 1.0);
        self.opacity.push(clamped);
    }

    /// Pop an opacity multiplier from the stack.
    pub fn pop_opacity(&mut self) -> Option<f32> {
        self.opacity.pop()
    }

    /// Clear all opacity multipliers.
    pub fn clear_opacity(&mut self) {
        self.opacity.clear();
    }

    /// Set a single cell with bounds checking. Returns true if written.
    pub fn set_cell(&mut self, x: u32, y: u32, cell: Cell) -> bool {
        if !self.in_scissor(x, y) {
            return false;
        }
        let idx = match self.index_of(x, y) {
            Some(i) => i,
            None => return false,
        };
        if idx >= self.cells.len() {
            return false;
        }
        self.cells[idx] = cell;
        true
    }

    /// Get a cell reference at (x, y), or None if out of bounds.
    #[must_use]
    pub fn get_cell(&self, x: u32, y: u32) -> Option<Cell> {
        let idx = self.index_of(x, y)?;
        self.cells.get(idx).copied()
    }

    /// Draw text starting at (x, y). Wide chars consume 2 cols (continuation
    /// cell marked with ch=0). Combining chars advance by 0. Control chars
    /// advance by 0. Clips to buffer bounds and scissor.
    pub fn draw_text(&mut self, text: &str, x: u32, y: u32, fg: Rgba, bg: Rgba, attrs: u32) {
        if y >= self.rows {
            return;
        }
        let mut cx = x;
        for ch in text.chars() {
            let w = char_width(ch);
            if w == 0 {
                // Combining or control: no advance, no cell written
                continue;
            }
            let cp = ch as u32;
            if cx >= self.cols {
                break;
            }
            if self.in_scissor(cx, y) {
                if let Some(idx) = self.index_of(cx, y) {
                    if idx < self.cells.len() {
                        let existing = self.cells[idx];
                        let op = self.current_opacity();
                        let eff_bg_a = (bg.a as f32 * op) as u8;
                        let blended_bg = Rgba::blend_over(
                            Rgba::new(bg.r, bg.g, bg.b, eff_bg_a),
                            existing.bg,
                        );
                        let eff_fg_a = (fg.a as f32 * op) as u8;
                        let blended_fg = if eff_fg_a == 255 {
                            Rgba::new(fg.r, fg.g, fg.b, 255)
                        } else {
                            Rgba::blend_over(
                                Rgba::new(fg.r, fg.g, fg.b, eff_fg_a),
                                existing.fg,
                            )
                        };
                        self.cells[idx] = Cell {
                            ch: cp,
                            fg: blended_fg,
                            bg: blended_bg,
                            attrs,
                        };
                    }
                }
            }
            // Wide char: write continuation cell
            if w == 2 {
                if cx + 1 < self.cols {
                    if self.in_scissor(cx + 1, y) {
                        if let Some(idx) = self.index_of(cx + 1, y) {
                            let op = self.current_opacity();
                            if idx < self.cells.len() {
                                let existing = &mut self.cells[idx];
                                let eff_bg_a = (bg.a as f32 * op) as u8;
                                let blended_bg = Rgba::blend_over(
                                    Rgba::new(bg.r, bg.g, bg.b, eff_bg_a),
                                    existing.bg,
                                );
                                *existing = Cell {
                                    ch: 0,
                                    fg: existing.fg,
                                    bg: blended_bg,
                                    attrs,
                                };
                            }
                        }
                    }
                }
            }
            cx += w;
        }
    }

    /// Fill a rectangle with a background color. Clips to buffer bounds + scissor.
    pub fn fill_rect(&mut self, x: u32, y: u32, width: u32, height: u32, bg: Rgba) {
        if self.cells.is_empty() {
            return;
        }
        let op = self.current_opacity();
        let eff_a = (bg.a as f32 * op) as u8;
        let solid_bg = Rgba::new(bg.r, bg.g, bg.b, eff_a);
        let x_end = (x + width).min(self.cols);
        let y_end = (y + height).min(self.rows);
        let mut yy = y;
        while yy < y_end {
            if !self.in_scissor(x, yy) && !self.in_scissor(x_end.saturating_sub(1), yy) {
                // Check if any cell in this row could be in scissor
                // We still need to check each cell individually
            }
            let mut cx = x;
            while cx < x_end {
                if !self.in_scissor(cx, yy) {
                    cx += 1;
                    continue;
                }
                if let Some(idx) = self.index_of(cx, yy) {
                    if idx < self.cells.len() {
                        let existing = self.cells[idx];
                        let blended = Rgba::blend_over(solid_bg, existing.bg);
                        self.cells[idx] = Cell {
                            ch: existing.ch,
                            fg: existing.fg,
                            bg: blended,
                            attrs: existing.attrs,
                        };
                    }
                }
                cx += 1;
            }
            yy += 1;
        }
    }

    /// Copy `src` cells into `self` at `(dx, dy)`, clipped to dst bounds
    /// and scissor. Mirrors `CellBuffer::blit` geometry contract.
    pub fn blit(&mut self, src: &CellBuffer, dx: u32, dy: u32) {
        if self.cells.is_empty() || src.cells.is_empty() {
            return;
        }
        for sy in 0..src.rows {
            let ty = dy + sy;
            if ty >= self.rows {
                break;
            }
            for sx in 0..src.cols {
                let tx = dx + sx;
                if tx >= self.cols {
                    break;
                }
                if !self.in_scissor(tx, ty) {
                    continue;
                }
                if let (Some(si), Some(ti)) = (src.index_of(sx, sy), self.index_of(tx, ty)) {
                    if si < src.cells.len() && ti < self.cells.len() {
                        self.cells[ti] = src.cells[si];
                    }
                }
            }
        }
    }

    /// Draw a grid/border rectangle at `(x, y)` with `width` x `height`.
    /// `chars` are 11 border glyphs in `border.ts` order: topLeft,
    /// topRight, bottomLeft, bottomRight, horizontal, vertical, topT,
    /// bottomT, leftT, rightT, cross. `sides` bitmask: bit0 left, bit1
    /// bottom, bit2 right, bit3 top (matches `packDrawOptions` decoded in
    /// `lib.zig:2032-2041`). `fill` fills the interior with `bg`.
    pub fn draw_grid(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        chars: &[char; 11],
        fg: Rgba,
        bg: Rgba,
        sides: u32,
        fill: bool,
    ) {
        if self.cells.is_empty() || width == 0 || height == 0 {
            return;
        }
        const LEFT: u32 = 1;
        const BOTTOM: u32 = 2;
        const RIGHT: u32 = 4;
        const TOP: u32 = 8;
        // Local writer avoids closure borrow conflicts with &mut self.
        fn put(slf: &mut CellBuffer, cx: u32, cy: u32, ch: char, fg: Rgba, bg: Rgba) {
            if ch == '\u{0}' {
                return;
            }
            slf.set_cell(cx, cy, Cell { ch: ch as u32, fg, bg, attrs: 0 });
        }
        let x1 = x + width.saturating_sub(1);
        let y1 = y + height.saturating_sub(1);
        if fill {
            self.fill_rect(x, y, width, height, bg);
        }
        // Corners depend on which sides are present.
        if sides & TOP != 0 && sides & LEFT != 0 {
            put(self, x, y, chars[0], fg, bg);
        }
        if sides & TOP != 0 && sides & RIGHT != 0 {
            put(self, x1, y, chars[1], fg, bg);
        }
        if sides & BOTTOM != 0 && sides & LEFT != 0 {
            put(self, x, y1, chars[2], fg, bg);
        }
        if sides & BOTTOM != 0 && sides & RIGHT != 0 {
            put(self, x1, y1, chars[3], fg, bg);
        }
        // Horizontal edges.
        if width > 2 {
            let mut cx = x + 1;
            while cx < x1 {
                if sides & TOP != 0 {
                    put(self, cx, y, chars[4], fg, bg);
                }
                if sides & BOTTOM != 0 {
                    put(self, cx, y1, chars[4], fg, bg);
                }
                cx += 1;
            }
        }
        // Vertical edges.
        if height > 2 {
            let mut cy = y + 1;
            while cy < y1 {
                if sides & LEFT != 0 {
                    put(self, x, cy, chars[5], fg, bg);
                }
                if sides & RIGHT != 0 {
                    put(self, x1, cy, chars[5], fg, bg);
                }
                cy += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live(cols: u32, rows: u32) -> CellBuffer {
        let mut b = CellBuffer::new(cols, rows);
        b.init();
        b
    }

    fn ascii_grid() -> [char; 11] {
        ['+', '+', '+', '+', '-', '|', 'T', 'T', 'T', 'T', '+']
    }

    #[test]
    fn set_get_roundtrip() {
        let mut b = live(4, 3);
        let c = Cell { ch: 'A' as u32, fg: Rgba::rgb(1, 2, 3), bg: Rgba::rgb(4, 5, 6), attrs: 7 };
        assert!(b.set_cell(1, 2, c));
        assert_eq!(b.get_cell(1, 2), Some(c));
    }

    #[test]
    fn set_out_of_bounds_rejected() {
        let mut b = live(2, 2);
        assert!(!b.set_cell(5, 0, Cell::default()));
        assert_eq!(b.get_cell(5, 0), None);
    }

    #[test]
    fn draw_text_ascii_advances_one() {
        let mut b = live(8, 1);
        b.draw_text("hi", 0, 0, Rgba::rgb(255, 255, 255), Rgba::transparent(), 0);
        assert_eq!(b.get_cell(0, 0).unwrap().ch, 'h' as u32);
        assert_eq!(b.get_cell(1, 0).unwrap().ch, 'i' as u32);
        assert_eq!(b.get_cell(2, 0).unwrap().ch, ' ' as u32);
    }

    #[test]
    fn draw_text_wide_char_marks_continuation() {
        let mut b = live(8, 1);
        b.draw_text("一x", 0, 0, Rgba::rgb(255, 255, 255), Rgba::transparent(), 0);
        assert_eq!(b.get_cell(0, 0).unwrap().ch, '一' as u32);
        assert_eq!(b.get_cell(1, 0).unwrap().ch, 0);
        assert_eq!(b.get_cell(2, 0).unwrap().ch, 'x' as u32);
    }

    #[test]
    fn fill_rect_clips_to_bounds() {
        let mut b = live(4, 4);
        b.fill_rect(2, 2, 10, 10, Rgba::rgb(9, 9, 9));
        assert_eq!(b.get_cell(3, 3).unwrap().bg, Rgba::rgb(9, 9, 9));
        assert_eq!(b.get_cell(0, 0).unwrap().bg, Rgba::transparent());
    }

    #[test]
    fn alpha_blend_partial() {
        let half = Rgba::new(255, 0, 0, 128);
        let out = Rgba::blend_over(half, Rgba::rgb(0, 0, 0));
        assert!(out.r > 100 && out.r < 160);
        assert_eq!(out.b, 0);
    }

    #[test]
    fn scissor_blocks_outside_writes() {
        let mut b = live(6, 2);
        b.push_scissor_rect(0, 0, 2, 2);
        assert!(!b.set_cell(4, 0, Cell { ch: 'z' as u32, ..Cell::default() }));
        assert!(b.set_cell(1, 0, Cell { ch: 'y' as u32, ..Cell::default() }));
        b.pop_scissor_rect();
        assert!(b.set_cell(4, 0, Cell::default()));
    }

    #[test]
    fn opacity_stack_product() {
        let mut b = live(2, 2);
        b.push_opacity(0.5);
        b.push_opacity(0.5);
        assert!((b.current_opacity() - 0.25).abs() < 1e-6);
        b.pop_opacity();
        assert!((b.current_opacity() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn draw_grid_borders_and_corners() {
        let mut b = live(6, 4);
        let g = ascii_grid();
        b.draw_grid(0, 0, 6, 4, &g, Rgba::rgb(255, 255, 255), Rgba::transparent(), 0b1111, false);
        assert_eq!(b.get_cell(0, 0).unwrap().ch, '+' as u32);
        assert_eq!(b.get_cell(5, 0).unwrap().ch, '+' as u32);
        assert_eq!(b.get_cell(0, 3).unwrap().ch, '+' as u32);
        assert_eq!(b.get_cell(5, 3).unwrap().ch, '+' as u32);
        assert_eq!(b.get_cell(2, 0).unwrap().ch, '-' as u32);
        assert_eq!(b.get_cell(0, 2).unwrap().ch, '|' as u32);
        // Interior untouched without fill.
        assert_eq!(b.get_cell(2, 2).unwrap().ch, ' ' as u32);
    }

    #[test]
    fn blit_copies_region() {
        let mut src = live(2, 2);
        src.set_cell(0, 0, Cell { ch: 'Q' as u32, ..Cell::default() });
        let mut dst = live(4, 4);
        dst.blit(&src, 2, 1);
        assert_eq!(dst.get_cell(2, 1).unwrap().ch, 'Q' as u32);
        assert_eq!(dst.get_cell(0, 0).unwrap().ch, ' ' as u32);
    }
}
