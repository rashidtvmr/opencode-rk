#![forbid(unsafe_code)]
//! Mime-tagged clipboard (mirrors `packages/tui/src/context/clipboard.tsx:4`). In-memory only.

/// Max mime chars.
pub const MAX_MIME: usize = 64;
/// Max data chars (64 KiB, char-boundary truncation keeps UTF-8 valid).
pub const MAX_DATA: usize = 64 * 1024;

/// Last-write-wins mime + data pair.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CtxClip {
    mime: String,
    data: String,
}

impl CtxClip {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Store `mime`/`data`, truncating to [`MAX_MIME`]/[`MAX_DATA`] chars.
    pub fn set(&mut self, mime: &str, data: &str) {
        self.mime = if mime.chars().count() > MAX_MIME {
            mime.chars().take(MAX_MIME).collect()
        } else {
            mime.to_string()
        };
        self.data = if data.chars().count() > MAX_DATA {
            data.chars().take(MAX_DATA).collect()
        } else {
            data.to_string()
        };
    }

    #[must_use]
    pub fn read(&self) -> (&str, &str) {
        (&self.mime, &self.data)
    }

    /// Empty both fields.
    pub fn clear(&mut self) {
        self.mime.clear();
        self.data.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut c = CtxClip::new();
        c.set("text/plain", "hi");
        assert_eq!(c.read(), ("text/plain", "hi"));
    }

    #[test]
    fn overwrite() {
        let mut c = CtxClip::new();
        c.set("a", "1");
        c.set("b", "2");
        assert_eq!(c.read(), ("b", "2"));
    }

    #[test]
    fn truncates_caps() {
        let mut c = CtxClip::new();
        c.set(&"m".repeat(MAX_MIME + 1), &"x".repeat(MAX_DATA + 1));
        assert_eq!(c.read().0.chars().count(), MAX_MIME);
        assert_eq!(c.read().1.chars().count(), MAX_DATA);
    }

    #[test]
    fn trunc_keeps_utf8_boundary() {
        let mut c = CtxClip::new();
        c.set("t", &"\u{00e9}".repeat(MAX_DATA + 5));
        assert_eq!(c.read().1.chars().count(), MAX_DATA);
    }

    #[test]
    fn clear_empties() {
        let mut c = CtxClip::new();
        c.set("text/plain", "hi");
        c.clear();
        assert_eq!(c.read(), ("", ""));
    }
}
