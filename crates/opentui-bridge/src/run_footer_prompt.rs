#![forbid(unsafe_code)]
//! Footer prompt row (mirrors `footer.prompt.tsx` single-line composer:
//! `draft.text` + `area.cursorOffset` + `resetDraft`/`submitPrompt`).
//!
//! Divergences: TS textarea is multi-row/grapheme (`Bun.stringWidth`);
//! Rust stores byte `String`, cursor is a byte index kept on char
//! boundaries, `move_cursor` steps by chars. History/menus/parts are
//! host concerns, not modeled.

/// Max bytes for `PromptRow.text` (fail-closed bound).
pub const MAX_TEXT: usize = 4096;

/// Single-line prompt composer: text plus byte cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptRow {
    pub text: String,
    pub cursor: usize,
    pub focused: bool,
}

impl Default for PromptRow {
    fn default() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            focused: false,
        }
    }
}

impl PromptRow {
    pub fn new() -> Self {
        Self::default()
    }

    fn clamp_cursor(&mut self) {
        while self.cursor > 0 && !self.text.is_char_boundary(self.cursor) {
            self.cursor -= 1;
        }
        self.cursor = self.cursor.min(self.text.len());
    }

    /// Insert `ch` at cursor. False when it would exceed `MAX_TEXT`.
    pub fn insert(&mut self, ch: char) -> bool {
        let mut buf = [0u8; 4];
        if self.text.len() + ch.encode_utf8(&mut buf).len() > MAX_TEXT {
            return false;
        }
        self.clamp_cursor();
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
        true
    }

    /// Delete char before cursor. False when cursor is at start.
    pub fn backspace(&mut self) -> bool {
        self.clamp_cursor();
        if self.cursor == 0 {
            return false;
        }
        let prev = self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.text.remove(prev);
        self.cursor = prev;
        true
    }

    /// Move cursor by `delta` chars, clamped to `0..=len`.
    pub fn move_cursor(&mut self, delta: isize) {
        self.clamp_cursor();
        let chars: Vec<usize> = {
            let mut v: Vec<usize> = self.text.char_indices().map(|(i, _)| i).collect();
            v.push(self.text.len());
            v
        };
        let pos = chars.iter().position(|&i| i == self.cursor).unwrap_or(0) as isize;
        let next = (pos + delta).clamp(0, chars.len() as isize - 1) as usize;
        self.cursor = chars[next];
    }

    /// Drain text, reset cursor to 0 (mirrors `resetDraft` after submit).
    pub fn submit(&mut self) -> String {
        self.cursor = 0;
        std::mem::take(&mut self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_cap_rejects_overflow() {
        let mut r = PromptRow::new();
        r.text = "a".repeat(MAX_TEXT);
        r.cursor = r.text.len();
        assert!(!r.insert('x'));
        assert_eq!(r.text.len(), MAX_TEXT);
    }

    #[test]
    fn backspace_empty_false() {
        let mut r = PromptRow::new();
        assert!(!r.backspace());
    }

    #[test]
    fn cursor_clamps_both_ends() {
        let mut r = PromptRow::new();
        r.text = "hi".to_string();
        r.cursor = 2;
        r.move_cursor(99);
        assert_eq!(r.cursor, 2);
        r.move_cursor(-99);
        assert_eq!(r.cursor, 0);
    }

    #[test]
    fn submit_drains_and_resets() {
        let mut r = PromptRow::new();
        r.text = "hello".to_string();
        r.cursor = 5;
        assert_eq!(r.submit(), "hello");
        assert_eq!(r.text, "");
        assert_eq!(r.cursor, 0);
    }

    #[test]
    fn multibyte_safe() {
        let mut r = PromptRow::new();
        assert!(r.insert('e'));
        assert!(r.insert('é'));
        assert_eq!(r.text, "eé");
        assert!(r.backspace());
        assert_eq!(r.text, "e");
        assert!(r.backspace());
        assert_eq!(r.text, "");
        assert!(!r.backspace());
    }

    #[test]
    fn insert_at_middle_shifts() {
        let mut r = PromptRow::new();
        r.text = "ac".to_string();
        r.cursor = 1;
        assert!(r.insert('b'));
        assert_eq!(r.text, "abc");
        assert_eq!(r.cursor, 2);
    }
}

/// Max bytes for `PromptDraft.text` (fail-closed bound, 8 KiB).
pub const MAX_DRAFT_TEXT: usize = 8192;

/// Max entries retained in `PromptDraft.history`.
pub const MAX_DRAFT_HISTORY: usize = 50;

/// Draft composer with navigable submit history (mirrors `footer.prompt.tsx`
/// `draft` + `createPromptHistory`/`pushPromptHistory`/`movePromptHistory`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PromptDraft {
    pub text: String,
    pub history: Vec<String>,
    pub hist_cursor: Option<usize>,
}

impl PromptDraft {
    pub fn new() -> Self {
        Self::default()
    }

    fn truncate_to_cap(s: &str) -> &str {
        if s.len() <= MAX_DRAFT_TEXT {
            return s;
        }
        let mut end = MAX_DRAFT_TEXT;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }

    /// Push current text when non-blank, evicting oldest past cap.
    /// Resets `hist_cursor`. No-op when `text` is empty/whitespace.
    pub fn push_history(&mut self) {
        if self.text.trim().is_empty() {
            self.hist_cursor = None;
            return;
        }
        if self.text.len() > MAX_DRAFT_TEXT {
            self.text = Self::truncate_to_cap(&self.text).to_string();
        }
        let entry = self.text.clone();
        if self.history.len() >= MAX_DRAFT_HISTORY {
            let overflow = self.history.len() + 1 - MAX_DRAFT_HISTORY;
            self.history.drain(..overflow);
        }
        self.history.push(entry);
        self.hist_cursor = None;
    }

    /// Step to older entry. False when history empty or already oldest.
    pub fn hist_prev(&mut self) -> bool {
        if self.history.is_empty() {
            return false;
        }
        let next = match self.hist_cursor {
            None => self.history.len() - 1,
            Some(0) => return false,
            Some(i) => i.saturating_sub(1).min(self.history.len() - 1),
        };
        self.hist_cursor = Some(next);
        self.text = self.history[next].clone();
        true
    }

    /// Step to newer entry. False when not browsing or past newest
    /// (cursor resets to `None`; text kept).
    pub fn hist_next(&mut self) -> bool {
        match self.hist_cursor {
            None => false,
            Some(i) => {
                if i + 1 < self.history.len() {
                    self.hist_cursor = Some(i + 1);
                    self.text = self.history[i + 1].clone();
                    true
                } else {
                    self.hist_cursor = None;
                    false
                }
            }
        }
    }

    /// Whitespace-separated word count of `text`.
    pub fn word_count(&self) -> usize {
        self.text.split_whitespace().count()
    }

    /// True when `text` is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

#[cfg(test)]
mod tests2 {
    use super::*;

    #[test]
    fn push_appends_and_resets_cursor() {
        let mut d = PromptDraft::new();
        d.text = "hello".to_string();
        d.push_history();
        assert_eq!(d.history, vec!["hello".to_string()]);
        assert_eq!(d.hist_cursor, None);
    }

    #[test]
    fn push_skips_empty_and_whitespace() {
        let mut d = PromptDraft::new();
        d.push_history();
        d.text = "   ".to_string();
        d.push_history();
        assert!(d.history.is_empty());
    }

    #[test]
    fn prev_next_bounds() {
        let mut d = PromptDraft::new();
        assert!(!d.hist_prev());
        assert!(!d.hist_next());
        d.text = "a".to_string();
        d.push_history();
        d.text = "b".to_string();
        d.push_history();
        assert!(d.hist_prev());
        assert_eq!(d.text, "b");
        assert!(d.hist_prev());
        assert_eq!(d.text, "a");
        assert!(!d.hist_prev());
        assert!(d.hist_next());
        assert_eq!(d.text, "b");
        assert!(!d.hist_next());
    }

    #[test]
    fn evicts_oldest_at_cap() {
        let mut d = PromptDraft::new();
        for i in 0..(MAX_DRAFT_HISTORY + 1) {
            d.text = format!("m{i}");
            d.push_history();
        }
        assert_eq!(d.history.len(), MAX_DRAFT_HISTORY);
        assert_eq!(d.history[0], "m1");
    }

    #[test]
    fn word_count_splits_whitespace() {
        let mut d = PromptDraft::new();
        assert_eq!(d.word_count(), 0);
        d.text = "  hello   world\nnew ".to_string();
        assert_eq!(d.word_count(), 3);
    }

    #[test]
    fn is_empty_mirrors_text() {
        let mut d = PromptDraft::new();
        assert!(d.is_empty());
        d.text = "x".to_string();
        assert!(!d.is_empty());
    }
}
