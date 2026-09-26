#![forbid(unsafe_code)]
//! Pending open-request ctx for `packages/tui/src/context/editor.ts` labelState.
//! Pure, bounded, no IO. ponytail: single pending slot; upgrade: queue.

/// Byte cap for file path.
pub const MAX_FILE_BYTES: usize = 512;

fn trunc(s: &str, cap: usize) -> &str {
    if s.len() <= cap {
        return s;
    }
    let mut end = cap;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Pending editor open-request context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorCtx {
    file: String,
    pending: bool,
}

impl EditorCtx {
    #[must_use]
    pub fn new() -> Self {
        Self {
            file: String::new(),
            pending: false,
        }
    }

    pub fn request_open(&mut self, file: &str) -> bool {
        if file.is_empty() {
            return false;
        }
        self.file = trunc(file, MAX_FILE_BYTES).to_string();
        self.pending = true;
        true
    }

    pub fn ack(&mut self) -> bool {
        if self.pending {
            self.pending = false;
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }
}

impl Default for EditorCtx {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_returns_false() {
        let mut c = EditorCtx::new();
        assert!(!c.request_open(""));
        assert!(!c.pending);
        assert_eq!(c.file(), "");
    }

    #[test]
    fn ack_flow() {
        let mut c = EditorCtx::new();
        assert!(c.request_open("a.ts"));
        assert!(c.pending);
        assert!(c.ack());
        assert!(!c.pending);
        assert_eq!(c.file(), "a.ts");
    }

    #[test]
    fn double_ack_false() {
        let mut c = EditorCtx::new();
        assert!(c.request_open("a.ts"));
        assert!(c.ack());
        assert!(!c.ack());
    }

    #[test]
    fn truncates_long_file() {
        let mut c = EditorCtx::new();
        let long = "x".repeat(MAX_FILE_BYTES + 50);
        assert!(c.request_open(&long));
        assert_eq!(c.file().len(), MAX_FILE_BYTES);
    }

    #[test]
    fn default_is_empty() {
        let c = EditorCtx::default();
        assert_eq!(c.file(), "");
        assert!(!c.pending);
    }

    #[test]
    fn ack_without_request_false() {
        let mut c = EditorCtx::new();
        assert!(!c.ack());
    }
}
