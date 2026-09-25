#![forbid(unsafe_code)]
//! In-memory clipboard context (mirrors `packages/tui/src/context/clipboard.tsx`).
//!
//! TS: clipboard.tsx:4-8 (`ClipboardContent`/`ClipboardService` shape,
//! provider defaults to native read/write). This type is the testable
//! in-memory fallback: last-write-wins text plus a bump counter so UI
//! can skip redundant pastes.

/// Max stored chars (8 KiB chars, char-boundary truncation keeps UTF-8 valid).
pub const MAX_CHARS: usize = 8 * 1024;

/// Last-write-wins clipboard text with a change counter.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ClipboardCtx {
    text: String,
    seq: u64,
}

impl ClipboardCtx {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Store `text`; empty input returns false and stores nothing.
    /// Over-long input truncates to [`MAX_CHARS`] chars. Success bumps
    /// `seq` saturating.
    pub fn copy(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        if text.chars().count() > MAX_CHARS {
            self.text = text.chars().take(MAX_CHARS).collect();
        } else {
            self.text = text.to_string();
        }
        self.seq = self.seq.saturating_add(1);
        true
    }

    #[must_use]
    pub fn paste(&self) -> &str {
        &self.text
    }

    /// Empty stored text; `seq` unchanged.
    pub fn clear(&mut self) {
        self.text.clear();
    }

    #[must_use]
    pub fn seq(&self) -> u64 {
        self.seq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_copy_false() {
        let mut c = ClipboardCtx::new();
        assert!(!c.copy(""));
        assert_eq!(c.paste(), "");
        assert_eq!(c.seq(), 0);
    }

    #[test]
    fn copy_paste_roundtrip() {
        let mut c = ClipboardCtx::new();
        assert!(c.copy("hi"));
        assert_eq!(c.paste(), "hi");
    }

    #[test]
    fn seq_bumps_per_copy() {
        let mut c = ClipboardCtx::new();
        c.copy("a");
        c.copy("b");
        assert_eq!(c.seq(), 2);
        assert_eq!(c.paste(), "b");
    }

    #[test]
    fn overlong_truncates() {
        let mut c = ClipboardCtx::new();
        let big = "x".repeat(MAX_CHARS + 10);
        assert!(c.copy(&big));
        assert_eq!(c.paste().chars().count(), MAX_CHARS);
    }

    #[test]
    fn trunc_keeps_utf8_boundary() {
        let mut c = ClipboardCtx::new();
        let big = "\u{00e9}".repeat(MAX_CHARS + 5);
        assert!(c.copy(&big));
        assert_eq!(c.paste().chars().count(), MAX_CHARS);
    }

    #[test]
    fn clear_empties_keeps_seq() {
        let mut c = ClipboardCtx::new();
        c.copy("hi");
        c.clear();
        assert_eq!(c.paste(), "");
        assert_eq!(c.seq(), 1);
    }
}
