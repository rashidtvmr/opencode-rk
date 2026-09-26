#![forbid(unsafe_code)]
//! Char-cursor draft buffer (primitive for `prompt_ctx::PromptCtx`).
//! Pure data: bounded text, char-index cursor. No I/O.

/// Draft cap in chars (8KiB chars, mirrors `prompt_ctx::MAX_DRAFT_CHARS`).
pub const MAX_DRAFT_CHARS: usize = 8192;

/// Editable draft with char-index cursor.
#[derive(Debug, Clone, Default)]
pub struct DraftStore {
    text: String,
    cursor: usize,
}

impl DraftStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    fn byte_idx(&self, pos: usize) -> usize {
        self.text
            .char_indices()
            .nth(pos)
            .map_or(self.text.len(), |(i, _)| i)
    }
    /// Insert char at cursor; ignored at cap. Cursor moves past it.
    pub fn insert(&mut self, ch: char) {
        if self.text.chars().count() >= MAX_DRAFT_CHARS {
            return;
        }
        let i = self.byte_idx(self.cursor);
        self.text.insert(i, ch);
        self.cursor += 1;
    }
    /// Delete char before cursor. `false` when cursor at 0.
    pub fn backspace(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let end = self.byte_idx(self.cursor);
        let start = self.byte_idx(self.cursor - 1);
        self.text.drain(start..end);
        self.cursor -= 1;
        true
    }
    /// Move cursor by delta, clamped to `0..=len_chars`.
    pub fn move_cursor(&mut self, delta: isize) {
        let n = self.text.chars().count() as isize;
        let next = self.cursor as isize + delta;
        self.cursor = next.clamp(0, n) as usize;
    }
    /// Clear text, reset cursor to 0.
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_empty() {
        let d = DraftStore::new();
        assert_eq!(d.text(), "");
        assert_eq!(d.cursor(), 0);
    }
    #[test]
    fn insert_appends() {
        let mut d = DraftStore::new();
        d.insert('a');
        d.insert('b');
        assert_eq!(d.text(), "ab");
        assert_eq!(d.cursor(), 2);
    }
    #[test]
    fn insert_middle_moves() {
        let mut d = DraftStore::new();
        d.insert('a');
        d.insert('c');
        d.move_cursor(-1);
        d.insert('b');
        assert_eq!(d.text(), "abc");
        assert_eq!(d.cursor(), 2);
    }
    #[test]
    fn backspace_deletes_before() {
        let mut d = DraftStore::new();
        assert!(!d.backspace());
        d.insert('a');
        d.insert('b');
        d.move_cursor(-1);
        assert!(d.backspace());
        assert_eq!(d.text(), "b");
        assert_eq!(d.cursor(), 0);
    }
    #[test]
    fn cap_8kib_chars() {
        let mut d = DraftStore::new();
        for _ in 0..MAX_DRAFT_CHARS + 10 {
            d.insert('x');
        }
        assert_eq!(d.text().chars().count(), MAX_DRAFT_CHARS);
        assert_eq!(d.cursor(), MAX_DRAFT_CHARS);
    }
    #[test]
    fn move_cursor_clamps_and_clear() {
        let mut d = DraftStore::new();
        d.insert('a');
        d.move_cursor(99);
        assert_eq!(d.cursor(), 1);
        d.move_cursor(-99);
        assert_eq!(d.cursor(), 0);
        d.clear();
        assert_eq!(d.text(), "");
        assert_eq!(d.cursor(), 0);
    }
    #[test]
    fn unicode_char_index() {
        let mut d = DraftStore::new();
        d.insert('e');
        d.insert('é');
        d.move_cursor(-1);
        d.insert('X');
        assert_eq!(d.text(), "eXé");
        assert!(d.backspace());
        assert_eq!(d.text(), "eé");
    }
}
