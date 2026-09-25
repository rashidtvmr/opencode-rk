#![forbid(unsafe_code)]
//! Todo checkbox line: `[x]`/`[ ]` + truncated title, char-safe.
//!
//! TS truth (`component/todo-item.tsx`): row box with status glyph
//! (`[x]` completed, `[ ]` else) plus content text. No theme here;
//! caller paints. `ponytail:` no status enum, add when caller needs it.

/// Single todo row; title truncated to 256 chars at construction.
pub struct TodoItem {
    title: String,
    done: bool,
}

impl TodoItem {
    /// Build from title; truncates to 256 chars, `done` false.
    #[must_use]
    pub fn new(title: &str) -> Self {
        Self {
            title: title.chars().take(256).collect(),
            done: false,
        }
    }

    /// Flip completion flag.
    pub fn toggle(&mut self) {
        self.done = !self.done;
    }

    /// Rendered line: `[x] title` or `[ ] title`.
    #[must_use]
    pub fn line(&self) -> String {
        format!("{} {}", if self.done { "[x]" } else { "[ ]" }, self.title)
    }

    /// Completion flag.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchecked_line() {
        assert_eq!(TodoItem::new("buy milk").line(), "[ ] buy milk");
    }

    #[test]
    fn toggle_marks_done() {
        let mut t = TodoItem::new("a");
        assert!(!t.is_done());
        t.toggle();
        assert!(t.is_done());
        assert_eq!(t.line(), "[x] a");
    }

    #[test]
    fn toggle_twice_unchecks() {
        let mut t = TodoItem::new("a");
        t.toggle();
        t.toggle();
        assert!(!t.is_done());
        assert_eq!(t.line(), "[ ] a");
    }

    #[test]
    fn title_capped_at_256() {
        let long = "a".repeat(300);
        let t = TodoItem::new(&long);
        assert_eq!(t.title.chars().count(), 256);
    }

    #[test]
    fn empty_title_line() {
        assert_eq!(TodoItem::new("").line(), "[ ] ");
    }
}
