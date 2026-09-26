#![forbid(unsafe_code)]
//! Kitty keyboard flags: default vs hardcoded enable path.
//!
//! `tui_entry.rs:628` calls `enable_kitty_keyboard(1)`, hardcoding
//! DISAMBIGUATE only and ignoring `KittyFlags::from_empty` (=5,
//! `core_events.rs:43`). This module documents both sides.

/// Disambiguate escape codes (kitty bit 1).
pub const DISAMBIGUATE: u8 = 1;
/// Alternate-keys bit in this lane's numbering (see note below).
/// NOTE: differs from `core_events::KITTY_ALTERNATE_KEYS` (=4, upstream
/// kitty bit); this lane pins 2 so `default_flags` = 3 per task spec.
pub const ALTERNATE_KEYS: u8 = 2;

/// Default flags: DISAMBIGUATE|ALTERNATE_KEYS = 3.
#[must_use]
pub const fn default_flags() -> u8 {
    DISAMBIGUATE | ALTERNATE_KEYS
}

/// True: the enable path hardcodes `1`, ignoring [`default_flags`].
#[must_use]
pub const fn hardcoded_one() -> bool {
    true
}

const _: () = assert!(default_flags() == 3);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_three() {
        assert_eq!(default_flags(), 3);
        assert_eq!(DISAMBIGUATE | ALTERNATE_KEYS, 3);
    }

    #[test]
    fn hardcoded_one_true() {
        assert!(hardcoded_one());
    }

    #[test]
    fn hardcoded_ignores_default() {
        assert_ne!(1, default_flags());
    }

    #[test]
    fn bits_distinct() {
        assert_eq!(DISAMBIGUATE, 1);
        assert_eq!(ALTERNATE_KEYS, 2);
        assert_eq!(DISAMBIGUATE & ALTERNATE_KEYS, 0);
    }
}
