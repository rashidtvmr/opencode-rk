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

    /// Opaque shortcut (alpha 255).
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Fully transparent.
    pub const fn transparent() -> Self {
        Self { r: 0, g: 0, b: 0, a: 0 }
    }

    /// Pack as RGB-intent lanes (high bytes zero).
    #[must_use]
    pub const fn to_packed(self) -> RgbaU16 {
        pack_rgba8(self.r, self.g, self.b, self.a, pack_meta(INTENT_RGB, 0))
    }

    /// Unpack color bytes; intent/meta ignored (fail-open display path).
    #[must_use]
    pub const fn from_packed(lanes: RgbaU16) -> Self {
        Self {
            r: (lanes[0] & 0xff) as u8,
            g: (lanes[1] & 0xff) as u8,
            b: (lanes[2] & 0xff) as u8,
            a: (lanes[3] & 0xff) as u8,
        }
    }

    /// Parse `#rrggbb` / `#rgb` (leading `#` optional); fail-closed `None`.
    #[must_use]
    pub fn from_hex(hex: &str) -> Option<Self> {
        from_hex(hex).map(Self::from_packed)
    }

    fn clamp_f32(v: f32) -> u8 {
        if !v.is_finite() {
            return 0;
        }
        (v.clamp(0.0, 1.0) * 255.0).round() as u8
    }

    fn clamp_i32(v: i32) -> u8 {
        v.clamp(0, 255) as u8
    }

    /// Clamped 0.0-1.0 float lanes (NaN/Inf -> 0).
    #[must_use]
    pub fn from_f32(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r: Self::clamp_f32(r),
            g: Self::clamp_f32(g),
            b: Self::clamp_f32(b),
            a: Self::clamp_f32(a),
        }
    }

    /// Clamped integer lanes.
    #[must_use]
    pub fn from_i32(r: i32, g: i32, b: i32, a: i32) -> Self {
        Self {
            r: Self::clamp_i32(r),
            g: Self::clamp_i32(g),
            b: Self::clamp_i32(b),
            a: Self::clamp_i32(a),
        }
    }

    /// Opaque color from an ANSI-256 index.
    #[must_use]
    pub const fn from_ansi256(index: u8) -> Self {
        let (r, g, b) = ansi256_index_to_rgb(index);
        Self { r, g, b, a: 255 }
    }
}

/// Theme slots (TS `ThemeDef` order: fg bg accent success warning error muted border).
pub const THEME_SLOTS: usize = 8;
pub const SLOT_FG: u8 = 0;
pub const SLOT_BG: u8 = 1;
pub const SLOT_ACCENT: u8 = 2;
pub const SLOT_SUCCESS: u8 = 3;
pub const SLOT_WARNING: u8 = 4;
pub const SLOT_ERROR: u8 = 5;
pub const SLOT_MUTED: u8 = 6;
pub const SLOT_BORDER: u8 = 7;

/// Semantic name -> slot index. Fail-closed `None`.
#[must_use]
pub const fn theme_slot(name: &str) -> Option<u8> {
    // ponytail: match on &str not const-friendly; upgrade: perfect-hash map.
    let bytes = name.as_bytes();
    match bytes.len() {
        2 => {
            if bytes[0] == b'f' && bytes[1] == b'g' {
                Some(SLOT_FG)
            } else if bytes[0] == b'b' && bytes[1] == b'g' {
                Some(SLOT_BG)
            } else {
                None
            }
        }
        5 => {
            if bytes[0] == b'e' && bytes[1] == b'r' && bytes[2] == b'r' && bytes[3] == b'o' && bytes[4] == b'r' {
                Some(SLOT_ERROR)
            } else if bytes[0] == b'm' && bytes[1] == b'u' && bytes[2] == b't' && bytes[3] == b'e' && bytes[4] == b'd' {
                Some(SLOT_MUTED)
            } else {
                None
            }
        }
        6 => {
            if bytes[0] == b'a' && bytes[1] == b'c' && bytes[2] == b'c' && bytes[3] == b'e' && bytes[4] == b'n' && bytes[5] == b't' {
                Some(SLOT_ACCENT)
            } else if bytes[0] == b'b' && bytes[1] == b'o' && bytes[2] == b'r' && bytes[3] == b'd' && bytes[4] == b'e' && bytes[5] == b'r' {
                Some(SLOT_BORDER)
            } else {
                None
            }
        }
        7 => {
            if bytes[0] == b's' && bytes[1] == b'u' && bytes[2] == b'c' && bytes[3] == b'c' && bytes[4] == b'e' && bytes[5] == b's' && bytes[6] == b's' {
                Some(SLOT_SUCCESS)
            } else if bytes[0] == b'w' && bytes[1] == b'a' && bytes[2] == b'r' && bytes[3] == b'n' && bytes[4] == b'i' && bytes[5] == b'n' && bytes[6] == b'g' {
                Some(SLOT_WARNING)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Indexed-intent lanes for a theme slot (color bytes zero; resolver fills).
#[must_use]
pub const fn indexed(slot: u8) -> RgbaU16 {
    pack_rgba8(0, 0, 0, 0, pack_meta(INTENT_INDEXED, slot))
}

/// Default-intent lanes (terminal default fg/bg).
#[must_use]
pub const fn default_packed() -> RgbaU16 {
    pack_rgba8(0, 0, 0, 0, pack_meta(INTENT_DEFAULT, 0))
}

/// Meta `u32` back out of packed lanes (little-endian lane order).
#[must_use]
pub const fn meta_of(lanes: RgbaU16) -> u32 {
    ((lanes[0] >> 8) as u32 & 0xff)
        | (((lanes[1] >> 8) as u32 & 0xff) << 8)
        | (((lanes[2] >> 8) as u32 & 0xff) << 16)
        | (((lanes[3] >> 8) as u32 & 0xff) << 24)
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

    #[test]
    fn rgba_packed_roundtrip() {
        let c = Rgba::new(0x12, 0x34, 0x56, 0x78);
        assert_eq!(c.to_packed(), [0x12, 0x34, 0x56, 0x78]);
        assert_eq!(Rgba::from_packed(c.to_packed()), c);
        assert_eq!(Rgba::rgb(1, 2, 3).to_packed(), rgb(1, 2, 3));
        assert_eq!(Rgba::transparent().to_packed(), TRANSPARENT);
    }

    #[test]
    fn rgba_from_hex_rgb() {
        assert_eq!(Rgba::from_hex("#ff0000"), Some(Rgba::rgb(0xff, 0x00, 0x00)));
        assert_eq!(Rgba::from_hex("#f00"), Some(Rgba::rgb(0xff, 0x00, 0x00)));
        assert_eq!(Rgba::from_hex("00ff00"), Some(Rgba::rgb(0x00, 0xff, 0x00)));
        assert_eq!(Rgba::from_hex(""), None);
        assert_eq!(Rgba::from_hex("#ff"), None);
        assert_eq!(Rgba::from_hex("#gggggg"), None);
    }

    #[test]
    fn rgba_from_f32_clamps() {
        assert_eq!(Rgba::from_f32(1.0, 0.0, 0.5, 1.0), Rgba::new(255, 0, 128, 255));
        assert_eq!(Rgba::from_f32(-1.0, 2.0, 0.0, -0.5), Rgba::new(0, 255, 0, 0));
        assert_eq!(Rgba::from_f32(f32::NAN, 0.0, 0.0, 1.0).r, 0);
    }

    #[test]
    fn rgba_from_i32_clamps() {
        assert_eq!(Rgba::from_i32(255, 0, 128, 255), Rgba::new(255, 0, 128, 255));
        assert_eq!(Rgba::from_i32(-5, 300, 0, 70000), Rgba::new(0, 255, 0, 255));
    }

    #[test]
    fn rgba_from_ansi256_spots() {
        assert_eq!(Rgba::from_ansi256(0), Rgba::rgb(0, 0, 0));
        assert_eq!(Rgba::from_ansi256(15), Rgba::rgb(255, 255, 255));
        assert_eq!(Rgba::from_ansi256(196), Rgba::rgb(255, 0, 0));
        assert_eq!(Rgba::from_ansi256(255), Rgba::rgb(238, 238, 238));
    }

    #[test]
    fn theme_slots_indexed_roundtrip() {
        assert_eq!(THEME_SLOTS, 8);
        assert_eq!(theme_slot("fg"), Some(SLOT_FG));
        assert_eq!(theme_slot("border"), Some(SLOT_BORDER));
        assert_eq!(theme_slot("nope"), None);
        let lanes = indexed(SLOT_ACCENT);
        assert_eq!(intent_of(meta_of(lanes)), INTENT_INDEXED);
        assert_eq!(slot_of(meta_of(lanes)), SLOT_ACCENT);
        let d = default_packed();
        assert_eq!(intent_of(meta_of(d)), INTENT_DEFAULT);
        assert_eq!(Rgba::from_packed(rgb(1, 2, 3)), Rgba::rgb(1, 2, 3));
    }
}
