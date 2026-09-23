#![forbid(unsafe_code)]
//! Text attribute bitflags (mirrors `TextAttributes` from `@opentui/core`).
//!
//! # SOURCE EVIDENCE (TS checkout at /home/rashid/projects/opencode, commit a0d9c6b, NOT pinned 95daf90)
//! - `TextAttributes.BOLD` used across `packages/tui/src` (e.g.
//!   `packages/tui/src/component/bg-pulse-render.ts:64`,
//!   `packages/tui/src/component/logo.tsx:11`,
//!   `packages/opencode/src/cli/cmd/run/scrollback.shared.ts:32`).
//! - `TextAttributes.DIM` used in
//!   `packages/opencode/src/cli/cmd/run/scrollback.shared.ts:39,56`,
//!   `packages/opencode/src/cli/cmd/run/splash.ts:228,232`.
//! - `TextAttributes.STRIKETHROUGH` used in
//!   `packages/tui/src/routes/session/index.tsx:1952,1963,1970`.
//! - No other `TextAttributes.<MEMBER>` hits in `packages/tui/src` (rg `TextAttributes\.`).
//! - Bit positions 0-7 (Bold/Dim/Italic/Underline/Blink/Inverse/Hidden/Strikethrough)
//!   per `https://simonklee.dk/static/lab/opentui-explained` section 03
//!   ("Bit 0: Bold ... Bit 7: Strikethrough"); full member names
//!   (BOLD, DIM, ITALIC, UNDERLINE, BLINK, INVERSE, HIDDEN, STRIKETHROUGH)
//!   per lobehub opentui-dev skill snippet. Cross-check: Go port
//!   (`pkg.go.dev/github.com/sst/opentui/packages/go`) has
//!   `AttrBold=1<<0 ... AttrStrike=1<<6` (7 flags, `Reverse` naming, no Hidden).
//! - DIVERGENCE: `@opentui/core@0.4.3` source not vendored in this checkout
//!   (no `node_modules/@opentui`); exact TS numeric values unconfirmed.
//!   Bit values below assume TS matches the Zig 8-bit layout. `INVERSE` vs
//!   `REVERSE` naming unconfirmed in TS (Go uses `Reverse`); we use `INVERSE`
//!   per TS-docs snippet.
//! - Width: cell `attributes` lane is `u32` (low 8 bits attrs + 24 bits link ID)
//!   per explainer section 03; this `u16` struct carries the attribute bits and
//!   round-trips unknown bits untouched.

/// Text attribute flags (low 8 bits of the cell `attributes` lane).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextAttributes(u16);

impl TextAttributes {
    /// No attributes.
    pub const NONE: Self = Self(0);
    /// Bold (bit 0).
    pub const BOLD: Self = Self(1 << 0);
    /// Dim (bit 1).
    pub const DIM: Self = Self(1 << 1);
    /// Italic (bit 2).
    pub const ITALIC: Self = Self(1 << 2);
    /// Underline (bit 3).
    pub const UNDERLINE: Self = Self(1 << 3);
    /// Blink (bit 4).
    pub const BLINK: Self = Self(1 << 4);
    /// Inverse / reverse video (bit 5).
    pub const INVERSE: Self = Self(1 << 5);
    /// Hidden (bit 6).
    pub const HIDDEN: Self = Self(1 << 6);
    /// Strikethrough (bit 7).
    pub const STRIKETHROUGH: Self = Self(1 << 7);

    /// Empty set.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Raw bits.
    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Wrap raw bits (unknown bits preserved).
    #[must_use]
    pub const fn from_u16(bits: u16) -> Self {
        Self(bits)
    }

    /// Unwrap raw bits.
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    /// Union of two sets.
    #[must_use]
    pub const fn combine(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// True if all of `other`'s bits are set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Remove `other`'s bits.
    #[must_use]
    pub const fn remove(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// True if no bits set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl Default for TextAttributes {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        assert_eq!(TextAttributes::default(), TextAttributes::NONE);
        assert_eq!(TextAttributes::default().as_u16(), 0);
        assert!(TextAttributes::default().is_empty());
    }

    #[test]
    fn combine_and_contains() {
        let a = TextAttributes::BOLD.combine(TextAttributes::DIM);
        assert!(a.contains(TextAttributes::BOLD));
        assert!(a.contains(TextAttributes::DIM));
        assert!(!a.contains(TextAttributes::ITALIC));
        assert_eq!(a.as_u16(), 0b11);
    }

    #[test]
    fn remove_clears_only_target() {
        let a = TextAttributes::BOLD
            .combine(TextAttributes::DIM)
            .remove(TextAttributes::BOLD);
        assert!(!a.contains(TextAttributes::BOLD));
        assert!(a.contains(TextAttributes::DIM));
        assert_eq!(a, TextAttributes::DIM);
    }

    #[test]
    fn u16_roundtrip() {
        for bits in [0u16, 1, 0b1010_0101, 0x00FF, 0xFFFF] {
            assert_eq!(TextAttributes::from_u16(bits).as_u16(), bits);
        }
    }

    #[test]
    fn unknown_bits_preserved() {
        let a = TextAttributes::from_u16(0xFF00).combine(TextAttributes::BOLD);
        assert_eq!(a.as_u16(), 0xFF01);
        assert!(a.contains(TextAttributes::BOLD));
        let b = a.remove(TextAttributes::BOLD);
        assert_eq!(b.as_u16(), 0xFF00);
        assert!(!b.contains(TextAttributes::BOLD));
    }

    #[test]
    fn evidenced_bit_values() {
        assert_eq!(TextAttributes::BOLD.as_u16(), 1 << 0);
        assert_eq!(TextAttributes::DIM.as_u16(), 1 << 1);
        assert_eq!(TextAttributes::STRIKETHROUGH.as_u16(), 1 << 7);
    }
}
