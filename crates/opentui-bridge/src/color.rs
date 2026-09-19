#![forbid(unsafe_code)]
//! RGBA color (mirrors `packages/core/src/lib/RGBA.ts`).
//!
//! Packing mirrors `packRGBA8`/`packMeta`: each `u16` lane holds the color
//! byte in the low 8 bits and one byte of the 32-bit meta
//! (`slot | intent << 8`) in the high 8 bits, little-endian lane order.
//! Meta `0` = RGB intent.

/// Packed RGBA buffer: color byte + meta byte per lane.
pub type RgbaU16 = [u16; 4];

/// TS `ColorIntent` discriminants.
pub const INTENT_RGB: u8 = 0;
pub const INTENT_INDEXED: u8 = 1;
pub const INTENT_DEFAULT: u8 = 2;

/// Opaque RGB color, alpha 255. High bytes zero = meta 0 = RGB intent.
#[must_use]
pub const fn rgb(r: u8, g: u8, b: u8) -> RgbaU16 {
    [r as u16, g as u16, b as u16, 255]
}

/// Fully transparent.
pub const TRANSPARENT: RgbaU16 = [0, 0, 0, 0];

/// TS `packMeta`: low byte slot, high byte intent.
#[must_use]
pub const fn pack_meta(intent: u8, slot: u8) -> u32 {
    slot as u32 | ((intent as u32) << 8)
}

/// TS `packRGBA8`: color byte in each lane's low 8 bits, one meta byte per
/// lane's high 8 bits (little-endian lane order).
#[must_use]
pub const fn pack_rgba8(r: u8, g: u8, b: u8, a: u8, meta: u32) -> RgbaU16 {
    [
        r as u16 | (((meta) & 0xff) << 8) as u16,
        g as u16 | (((meta >> 8) & 0xff) << 8) as u16,
        b as u16 | (((meta >> 16) & 0xff) << 8) as u16,
        a as u16 | (((meta >> 24) & 0xff) << 8) as u16,
    ]
}

/// Intent byte back out of a packed meta.
#[must_use]
pub const fn intent_of(meta: u32) -> u8 {
    ((meta >> 8) & 0xff) as u8
}

/// Slot byte back out of a packed meta.
#[must_use]
pub const fn slot_of(meta: u32) -> u8 {
    (meta & 0xff) as u8
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Parse `#rrggbb` / `#rgb` (leading `#` optional) into packed RGB.
/// Fail-closed: anything else is `None`.
#[must_use]
pub fn from_hex(hex: &str) -> Option<RgbaU16> {
    let digits = hex.strip_prefix('#').unwrap_or(hex);
    let bytes = digits.as_bytes();
    let (r, g, b) = match bytes.len() {
        3 => {
            let expand = |i: usize| hex_nibble(bytes[i]).map(|v| v << 4 | v);
            (expand(0)?, expand(1)?, expand(2)?)
        }
        6 => {
            let byte_at = |i: usize| {
                let hi = hex_nibble(bytes[i])?;
                let lo = hex_nibble(bytes[i + 1])?;
                Some(hi << 4 | lo)
            };
            (byte_at(0)?, byte_at(2)?, byte_at(4)?)
        }
        _ => return None,
    };
    Some(pack_rgba8(r, g, b, 255, pack_meta(INTENT_RGB, 0)))
}

const fn cube_level(v: u8) -> u8 {
    match v {
        0 => 0,
        1 => 95,
        2 => 135,
        3 => 175,
        4 => 215,
        _ => 255,
    }
}

/// TS `ansi256IndexToRgb`: ANSI16 + 6x6x6 cube + grayscale ramp.
#[must_use]
pub const fn ansi256_index_to_rgb(index: u8) -> (u8, u8, u8) {
    if index < 16 {
        return match index {
            0 => (0x00, 0x00, 0x00),
            1 => (0x80, 0x00, 0x00),
            2 => (0x00, 0x80, 0x00),
            3 => (0x80, 0x80, 0x00),
            4 => (0x00, 0x00, 0x80),
            5 => (0x80, 0x00, 0x80),
            6 => (0x00, 0x80, 0x80),
            7 => (0xc0, 0xc0, 0xc0),
            8 => (0x80, 0x80, 0x80),
            9 => (0xff, 0x00, 0x00),
            10 => (0x00, 0xff, 0x00),
            11 => (0xff, 0xff, 0x00),
            12 => (0x00, 0x00, 0xff),
            13 => (0xff, 0x00, 0xff),
            14 => (0x00, 0xff, 0xff),
            _ => (0xff, 0xff, 0xff),
        };
    }
    if index < 232 {
        let cube = index - 16;
        return (
            cube_level(cube / 36),
            cube_level((cube / 6) % 6),
            cube_level(cube % 6),
        );
    }
    let value = 8 + (index - 232) * 10;
    (value, value, value)
}

/// RGBA color placeholder (no unsafe yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

const _: () = assert!(std::mem::size_of::<RgbaU16>() == 8);

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn rgb_packs_opaque_with_zero_meta() {
        assert_eq!(size_of::<RgbaU16>(), 8);
        assert_eq!(rgb(1, 2, 3), [1, 2, 3, 255]);
        assert_eq!(TRANSPARENT, [0, 0, 0, 0]);
        for lane in rgb(0x12, 0x34, 0x56) {
            assert_eq!(lane >> 8, 0);
        }
    }

    #[test]
    fn pack_meta_roundtrip() {
        assert_eq!(pack_meta(INTENT_RGB, 0), 0);
        let meta = pack_meta(INTENT_INDEXED, 7);
        assert_eq!(intent_of(meta), INTENT_INDEXED);
        assert_eq!(slot_of(meta), 7);
        let packed = pack_rgba8(1, 2, 3, 4, meta);
        assert_eq!([packed[0] & 0xff, packed[1] & 0xff, packed[2] & 0xff, packed[3] & 0xff], [1, 2, 3, 4]);
    }

    #[test]
    fn from_hex_ok_and_fail_closed() {
        assert_eq!(from_hex("#ff0000"), Some([0xff, 0x00, 0x00, 255]));
        assert_eq!(from_hex("#f00"), Some([0xff, 0x00, 0x00, 255]));
        assert_eq!(from_hex("00ff00"), Some([0x00, 0xff, 0x00, 255]));
        assert_eq!(from_hex(""), None);
        assert_eq!(from_hex("#ff"), None);
        assert_eq!(from_hex("#gggggg"), None);
        assert_eq!(from_hex("red"), None);
    }

    #[test]
    fn ansi256_spots() {
        assert_eq!(ansi256_index_to_rgb(0), (0, 0, 0));
        assert_eq!(ansi256_index_to_rgb(15), (255, 255, 255));
        assert_eq!(ansi256_index_to_rgb(16), (0, 0, 0));
        assert_eq!(ansi256_index_to_rgb(21), (0, 0, 255));
        assert_eq!(ansi256_index_to_rgb(46), (0, 255, 0));
        assert_eq!(ansi256_index_to_rgb(196), (255, 0, 0));
        assert_eq!(ansi256_index_to_rgb(231), (255, 255, 255));
        assert_eq!(ansi256_index_to_rgb(232), (8, 8, 8));
        assert_eq!(ansi256_index_to_rgb(255), (238, 238, 238));
    }
}
