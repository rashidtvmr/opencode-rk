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
}

/// Raw C-ABI buffer entry points. Every declaration matches an
/// `export fn` in `packages/native/src/lib.zig`. Nullable Zig pointers
/// (`?[*]`) map to raw pointers (null tolerated natively); colors are
/// `[u16; 4]` RGBA lanes. Linked only under `native`.
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
}
