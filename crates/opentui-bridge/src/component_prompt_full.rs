#![forbid(unsafe_code)]

//! Full prompt component state.
//! TS truth: packages/tui/src/component/prompt/index.tsx (TextareaRenderable).

/// Prompt text buffer, char-capped with char-offset cursor.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CompPrompt {
    text: String,
    cursor: usize,
}

const MAX_CHARS: usize = 4096;
const PREVIEW_CHARS: usize = 128;

fn byte_idx(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .map(|(i, _)| i)
        .nth(char_idx)
        .unwrap_or(s.len())
}

impl CompPrompt {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn insert(&mut self, s: &str) {
        let room = MAX_CHARS.saturating_sub(self.text.chars().count());
        let take: String = s.chars().take(room).collect();
        let n = take.chars().count();
        let idx = byte_idx(&self.text, self.cursor);
        self.text.insert_str(idx, &take);
        self.cursor += n;
    }
    pub fn move_cursor(&mut self, delta: isize) {
        let len = self.text.chars().count() as isize;
        self.cursor = (self.cursor as isize + delta).clamp(0, len) as usize;
    }
    pub fn preview(&self) -> String {
        self.text.chars().take(PREVIEW_CHARS).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn insert_moves_cursor_to_end() {
        let mut p = CompPrompt::new();
        p.insert("hi");
        p.move_cursor(-1);
        p.insert("X");
        assert_eq!(p.text(), "hXi");
        assert_eq!(p.cursor(), 2);
    }
    #[test]
    fn cursor_clamps() {
        let mut p = CompPrompt::new();
        p.insert("ab");
        p.move_cursor(99);
        assert_eq!(p.cursor(), 2);
        p.move_cursor(-99);
        assert_eq!(p.cursor(), 0);
    }
    #[test]
    fn insert_caps_at_4kib_chars() {
        let mut p = CompPrompt::new();
        p.insert(&"a".repeat(5000));
        assert_eq!(p.text().chars().count(), 4096);
        assert_eq!(p.cursor(), 4096);
    }
    #[test]
    fn preview_first_128_chars() {
        let mut p = CompPrompt::new();
        p.insert(&"b".repeat(200));
        assert_eq!(p.preview().chars().count(), 128);
    }
}
