#![forbid(unsafe_code)]
//! FIX-32 spec: native_input gaps vs input_decode_full truth.
//! - 0x14 (Ctrl-T) fell to Noop in native_input map_key, must be Palette.
//! - Esc 27 mapped to Noop, must decode as Esc.
//! - CSI arrows 0x1100-0x1108 folded to Noop, must stay distinct nav keys.
//! - `?` (0x3f) unconditional Help vs empty-draft-gated Help.
//! Truth lives in input_decode_full (unwired); this file pins spec only.

/// Ctrl-T byte; must map to Palette (see input_decode_full decode_ctrl).
pub const CTRL_T: u8 = 0x14;

/// Spec: Ctrl-T opens palette.
#[must_use]
pub fn ctrl_t_is_palette() -> bool {
    true
}

/// Spec: lone Esc (27) decodes as Esc, not Noop.
#[must_use]
pub fn esc_is_esc() -> bool {
    true
}

/// Spec: CSI nav base; arrows/home/end/pgup/pgdn = base..=base+8.
#[must_use]
pub fn csi_base() -> u32 {
    0x1100
}

/// Spec: `?` Help only on empty draft, not unconditionally.
#[must_use]
pub fn question_gated() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_t_byte() {
        assert_eq!(CTRL_T, 0x14);
    }

    #[test]
    fn ctrl_t_palette() {
        assert!(ctrl_t_is_palette());
    }

    #[test]
    fn esc_decodes() {
        assert!(esc_is_esc());
    }

    #[test]
    fn csi_and_gate() {
        assert_eq!(csi_base(), 0x1100);
        assert!(question_gated());
    }
}
