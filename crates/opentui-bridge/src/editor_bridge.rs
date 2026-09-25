#![forbid(unsafe_code)]
//! File:line bridge for `packages/tui/src/editor.ts:26` (`openEditor`).
//! Pure, bounded, no IO. ponytail: no spawn; upgrade: wire editor spawn.

/// Byte cap for file path.
pub const MAX_FILE_BYTES: usize = 512;
/// Byte cap for status string.
pub const MAX_STATUS_BYTES: usize = 600;

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

/// Open file:line pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorBridge {
    pub file: String,
    pub line: u32,
    pub open: bool,
}

impl EditorBridge {
    #[must_use]
    pub fn new() -> Self {
        Self {
            file: String::new(),
            line: 0,
            open: false,
        }
    }

    pub fn open(&mut self, file: &str, line: u32) -> Result<(), String> {
        if file.is_empty() {
            return Err("file is empty".to_string());
        }
        self.file = trunc(file, MAX_FILE_BYTES).to_string();
        self.line = line;
        self.open = true;
        Ok(())
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn goto(&mut self, line: u32) {
        self.line = line;
    }

    #[must_use]
    pub fn status(&self) -> String {
        let state = if self.open { "open" } else { "closed" };
        let s = format!("{}:{} {state}", self.file, self.line);
        trunc(&s, MAX_STATUS_BYTES).to_string()
    }
}

impl Default for EditorBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_errs() {
        let mut b = EditorBridge::new();
        assert!(b.open("", 1).is_err());
        assert!(!b.open);
    }

    #[test]
    fn open_close_roundtrip() {
        let mut b = EditorBridge::new();
        b.open("a.ts", 10).unwrap();
        assert!(b.open);
        assert_eq!(b.line, 10);
        b.close();
        assert!(!b.open);
        assert_eq!(b.file, "a.ts");
    }

    #[test]
    fn goto_updates_line() {
        let mut b = EditorBridge::new();
        b.open("a.ts", 1).unwrap();
        b.goto(42);
        assert_eq!(b.line, 42);
    }

    #[test]
    fn status_parts() {
        let mut b = EditorBridge::new();
        b.open("a.ts", 7).unwrap();
        assert_eq!(b.status(), "a.ts:7 open");
        b.close();
        assert_eq!(b.status(), "a.ts:7 closed");
    }

    #[test]
    fn file_truncates() {
        let mut b = EditorBridge::new();
        let long = "x".repeat(MAX_FILE_BYTES + 50);
        b.open(&long, 1).unwrap();
        assert_eq!(b.file.len(), MAX_FILE_BYTES);
    }

    #[test]
    fn status_truncates() {
        let mut b = EditorBridge::new();
        b.open(&"y".repeat(MAX_FILE_BYTES), u32::MAX).unwrap();
        assert!(b.status().len() <= MAX_STATUS_BYTES);
    }
}
