#![forbid(unsafe_code)]
//! Draw options: border flag + bounded pad (mirrors
//! `packages/core/src/lib/border.ts:39-92` `packDrawOptions`).

/// Max pad cells (`border.ts:39-92` bound).
pub const MAX_PAD: u8 = 8;

/// Packed draw options: border flag + pad cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DrawOpts {
    pub border: bool,
    pub pad: u8,
}

/// Pack border flag + pad, capping pad at 8. Pure, bounded.
#[must_use]
pub const fn pack(border: bool, pad: u8) -> DrawOpts {
    DrawOpts {
        border,
        pad: if pad > MAX_PAD { MAX_PAD } else { pad },
    }
}

impl DrawOpts {
    /// True when a border is drawn.
    #[must_use]
    pub const fn is_bordered(&self) -> bool {
        self.border
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_border_and_pad() {
        // `border.ts:39-92` packDrawOptions pattern.
        let o = pack(true, 3);
        assert!(o.is_bordered());
        assert_eq!(o.pad, 3);
    }

    #[test]
    fn caps_pad_at_eight() {
        // `border.ts:39-92` packDrawOptions pattern.
        assert_eq!(pack(true, 9).pad, 8);
        assert_eq!(pack(false, u8::MAX).pad, 8);
    }

    #[test]
    fn unbordered_flag_off() {
        // `border.ts:39-92` packDrawOptions pattern.
        let o = pack(false, 0);
        assert!(!o.is_bordered());
        assert_eq!(
            o,
            DrawOpts {
                border: false,
                pad: 0
            }
        );
    }
}
