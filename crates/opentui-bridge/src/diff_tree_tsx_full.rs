#![forbid(unsafe_code)]
//! TSX file-tree selection slot: bounded path list + cursor.
//!
//! TS truth (`diff-viewer-file-tree.tsx:1-40`): `files` + `selectedFileIndex`
//! props drive highlight; rows come from `flattenFileTree`. This slot stores
//! the flattened `paths` (64 cap, 512 chars each) + `cursor` selection index.
//!
//! `ponytail:` flat list + clamped cursor; add when caller needs tree/expand.

/// Bounded path list with selection cursor.
pub struct DiffTree {
    paths: Vec<String>,
    cursor: usize,
}

impl DiffTree {
    /// Empty list, cursor 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            paths: Vec::new(),
            cursor: 0,
        }
    }

    /// Push `path` (512-char cap); evicts oldest past 64.
    pub fn push(&mut self, path: &str) {
        if self.paths.len() >= 64 {
            self.paths.remove(0);
            self.cursor = self.cursor.saturating_sub(1);
        }
        self.paths.push(path.chars().take(512).collect());
    }

    /// Move cursor by `delta`, clamped; no-op when empty.
    pub fn move_cursor(&mut self, delta: isize) {
        if self.paths.is_empty() {
            self.cursor = 0;
            return;
        }
        let max = self.paths.len() as isize - 1;
        self.cursor = (self.cursor as isize + delta).clamp(0, max) as usize;
    }

    /// Selected path, or `None` when empty.
    #[must_use]
    pub fn selected(&self) -> Option<&str> {
        self.paths.get(self.cursor).map(String::as_str)
    }
}

impl Default for DiffTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_empty() {
        assert_eq!(DiffTree::new().selected(), None);
    }

    #[test]
    fn push_selects_first() {
        let mut d = DiffTree::new();
        d.push("src/a.ts");
        assert_eq!(d.selected(), Some("src/a.ts"));
    }

    #[test]
    fn push_caps_path_chars() {
        let mut d = DiffTree::new();
        d.push(&"x".repeat(600));
        assert_eq!(d.selected().unwrap().chars().count(), 512);
    }

    #[test]
    fn push_evicts_oldest_past_64() {
        let mut d = DiffTree::new();
        for i in 0..65 {
            d.push(&format!("f{i}"));
        }
        assert_eq!(d.selected(), Some("f1"));
        d.move_cursor(-64);
        assert_eq!(d.selected(), Some("f1"));
    }

    #[test]
    fn move_cursor_clamps() {
        let mut d = DiffTree::new();
        d.push("a");
        d.push("b");
        d.move_cursor(99);
        assert_eq!(d.selected(), Some("b"));
        d.move_cursor(-99);
        assert_eq!(d.selected(), Some("a"));
        d.move_cursor(1);
        assert_eq!(d.selected(), Some("b"));
    }
}
