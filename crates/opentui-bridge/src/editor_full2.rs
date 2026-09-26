#![forbid(unsafe_code)]
//! Full in-memory editor doc: path + bounded lines. Pure, bounded, no IO.
// ponytail: no persistence; upgrade: load/save via bridge.

/// Byte cap for path.
pub const MAX_PATH_BYTES: usize = 512;
pub const MAX_LINES: usize = 1024;
/// Byte cap per line (4KiB).
pub const MAX_LINE_BYTES: usize = 4096;

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

/// Open document with bounded path and lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorFull {
    pub open: bool,
    pub path: String,
    pub lines: Vec<String>,
}

impl EditorFull {
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: false,
            path: String::new(),
            lines: Vec::new(),
        }
    }

    pub fn open_at(&mut self, path: &str) -> bool {
        if path.is_empty() {
            return false;
        }
        self.path = trunc(path, MAX_PATH_BYTES).to_string();
        self.lines.clear();
        self.open = true;
        true
    }

    pub fn insert(&mut self, line: &str) -> bool {
        if !self.open || self.lines.len() >= MAX_LINES {
            return false;
        }
        self.lines.push(trunc(line, MAX_LINE_BYTES).to_string());
        true
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

impl Default for EditorFull {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_closed_empty() {
        let e = EditorFull::new();
        assert!(!e.open && e.line_count() == 0);
    }

    #[test]
    fn open_empty_false() {
        let mut e = EditorFull::new();
        assert!(!e.open_at(""));
        assert!(!e.open);
    }

    #[test]
    fn open_sets_path_clears() {
        let mut e = EditorFull::new();
        assert!(e.open_at("a.ts"));
        assert!(e.insert("x"));
        assert!(e.open_at("b.ts"));
        assert_eq!(e.path, "b.ts");
        assert_eq!(e.line_count(), 0);
    }

    #[test]
    fn path_truncates() {
        let mut e = EditorFull::new();
        assert!(e.open_at(&"p".repeat(MAX_PATH_BYTES + 10)));
        assert_eq!(e.path.len(), MAX_PATH_BYTES);
    }

    #[test]
    fn insert_closed_false() {
        let mut e = EditorFull::new();
        assert!(!e.insert("x"));
    }

    #[test]
    fn insert_caps() {
        let mut e = EditorFull::new();
        assert!(e.open_at("a.ts"));
        assert!(e.insert(&"l".repeat(MAX_LINE_BYTES + 8)));
        assert_eq!(e.lines[0].len(), MAX_LINE_BYTES);
        e.lines.clear();
        for _ in 0..MAX_LINES {
            assert!(e.insert("a"));
        }
        assert!(!e.insert("full"));
        assert_eq!(e.line_count(), MAX_LINES);
        e.close();
        assert!(!e.open);
    }
}
