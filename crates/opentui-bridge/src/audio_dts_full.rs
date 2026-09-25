#![forbid(unsafe_code)]
//! DTS path holder (mirrors `packages/tui/src/audio.d.ts:1-9`).
//!
//! TS truth declares `*.mp3 -> string` only; no codec info exists.
//! `ponytail:` capped String only; no PathBuf/OsStr until a caller needs fs.

/// Audio path with 64-char cap.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AudioFmt {
    name: String,
}

/// Max chars kept by [`AudioFmt::set`].
pub const MAX_NAME: usize = 64;

impl AudioFmt {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set(&mut self, name: &str) {
        self.name = name.chars().take(MAX_NAME).collect();
    }
    #[must_use]
    pub fn name_of(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub fn is_set(&self) -> bool {
        !self.name.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_by_default() {
        let f = AudioFmt::new();
        assert!(!f.is_set());
        assert_eq!(f.name_of(), "");
    }
    #[test]
    fn set_roundtrip() {
        let mut f = AudioFmt::new();
        f.set("bip-bop-01.mp3");
        assert!(f.is_set());
        assert_eq!(f.name_of(), "bip-bop-01.mp3");
    }
    #[test]
    fn cap_64() {
        let mut f = AudioFmt::new();
        f.set(&"a".repeat(100));
        assert_eq!(f.name_of().chars().count(), 64);
    }
}
