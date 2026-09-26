#![forbid(unsafe_code)]
//! In-memory text clipboard (mirrors `packages/tui/src/clipboard.ts:120-124` `write(text)`).
//! Divergence: in-memory only; native/OSC52 routing lives in `clipboard.rs` + `terminal.rs`.

/// Max stored chars (64 KiB chars; char-boundary truncation keeps UTF-8 valid).
pub const MAX_CHARS: usize = 64 * 1024;

/// Last-write-wins text store.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Clipboard {
    text: String,
}

impl Clipboard {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Store `text`, truncating to [`MAX_CHARS`] chars.
    pub fn copy(&mut self, text: &str) {
        self.text = if text.chars().count() > MAX_CHARS {
            text.chars().take(MAX_CHARS).collect()
        } else {
            text.to_string()
        };
    }

    #[must_use]
    pub fn paste(&self) -> &str {
        &self.text
    }

    /// Empty stored text.
    pub fn clear(&mut self) {
        self.text.clear();
    }

    /// Stored char count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.text.chars().count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut c = Clipboard::new();
        c.copy("hi");
        assert_eq!(c.paste(), "hi");
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn empty_roundtrip() {
        let mut c = Clipboard::new();
        c.copy("");
        assert_eq!(c.paste(), "");
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn truncates_overlong() {
        let mut c = Clipboard::new();
        c.copy(&"x".repeat(MAX_CHARS + 1));
        assert_eq!(c.len(), MAX_CHARS);
    }

    #[test]
    fn clear_empties() {
        let mut c = Clipboard::new();
        c.copy("hi");
        c.clear();
        assert_eq!(c.paste(), "");
        assert_eq!(c.len(), 0);
    }
}
