#![forbid(unsafe_code)]
//! Startup loading (mirrors `StartupLoading` text switch,
//! `packages/tui/src/component/startup-loading.tsx:8`).

/// Max message length in chars (JS `Array.from` code points).
pub const MAX_MSG: usize = 256;

/// Loading message plus done flag.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartupLoad {
    pub msg: String,
    pub done: bool,
}

impl StartupLoad {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set message, truncating to [`MAX_MSG`] chars.
    pub fn set_msg(&mut self, msg: &str) {
        self.msg = msg.chars().take(MAX_MSG).collect();
    }

    /// Mark loading complete.
    pub fn finish(&mut self) {
        self.done = true;
    }

    /// Current line: `"ready"` once done, else the message.
    #[must_use]
    pub fn line(&self) -> String {
        if self.done {
            "ready".to_string()
        } else {
            self.msg.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_by_default() {
        let s = StartupLoad::new();
        assert_eq!(s.line(), "");
        assert!(!s.done);
    }

    #[test]
    fn set_msg_roundtrips() {
        let mut s = StartupLoad::new();
        s.set_msg("Loading plugins...");
        assert_eq!(s.line(), "Loading plugins...");
    }

    #[test]
    fn set_msg_caps_at_256_chars() {
        let mut s = StartupLoad::new();
        s.set_msg(&"a".repeat(300));
        assert_eq!(s.msg.chars().count(), MAX_MSG);
        // ponytail: char (not byte) truncation; upgrade path: grapheme clusters via unicode-segmentation.
    }

    #[test]
    fn finish_yields_ready() {
        let mut s = StartupLoad::new();
        s.set_msg("Loading plugins...");
        s.finish();
        assert!(s.done);
        assert_eq!(s.line(), "ready");
    }
}
