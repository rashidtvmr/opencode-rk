//! Single-select question view: title, options, wrapping cursor, latched answer.
//!
//! Mirrors `packages/tui/src/routes/session/question.tsx` single-select path
//! (`selected` cursor over `options`, `pick` latches one answer and replies).

#![forbid(unsafe_code)]

/// Max chars retained in the title.
pub const TITLE_CAP: usize = 256;
/// Max options retained.
pub const OPTION_CAP: usize = 8;
/// Max chars retained per option.
pub const OPTION_LEN_CAP: usize = 256;

fn cap(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Single-question view model with a latched answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionView {
    title: String,
    options: Vec<String>,
    cursor: usize,
    answered: Option<usize>,
}

impl QuestionView {
    /// Build from title and options; truncates title to [`TITLE_CAP`],
    /// options to [`OPTION_CAP`] entries of [`OPTION_LEN_CAP`] chars.
    pub fn new(title: impl Into<String>, options: Vec<String>) -> Self {
        let title = cap(&title.into(), TITLE_CAP);
        let options: Vec<String> = options
            .into_iter()
            .take(OPTION_CAP)
            .map(|o| cap(&o, OPTION_LEN_CAP))
            .collect();
        Self {
            title,
            options,
            cursor: 0,
            answered: None,
        }
    }

    /// Title text.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Option labels.
    pub fn options(&self) -> &[String] {
        &self.options
    }

    /// Cursor index into options.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Latched answer index, if any.
    pub fn answered(&self) -> Option<usize> {
        self.answered
    }

    /// Move cursor by `delta`, wrapping; no-op when empty.
    pub fn move_cursor(&mut self, delta: isize) {
        let n = self.options.len();
        if n == 0 {
            self.cursor = 0;
            return;
        }
        let n = n as isize;
        self.cursor = (self.cursor as isize + delta).rem_euclid(n) as usize;
    }

    /// Latch cursor as answer; `true` on first call, `false` once
    /// answered or when there are no options.
    pub fn answer(&mut self) -> bool {
        if self.options.is_empty() || self.answered.is_some() {
            return false;
        }
        self.answered = Some(self.cursor);
        true
    }

    /// Latched answer label, if any.
    pub fn answered_text(&self) -> Option<&str> {
        self.answered
            .and_then(|i| self.options.get(i).map(String::as_str))
    }

    /// Clear answer and return cursor to top.
    pub fn reset(&mut self) {
        self.answered = None;
        self.cursor = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> QuestionView {
        QuestionView::new("pick", vec!["a".into(), "b".into(), "c".into()])
    }

    #[test]
    fn cursor_wraps_forward() {
        let mut v = view();
        v.move_cursor(3);
        assert_eq!(v.cursor(), 0);
        v.move_cursor(4);
        assert_eq!(v.cursor(), 1);
    }

    #[test]
    fn cursor_wraps_backward() {
        let mut v = view();
        v.move_cursor(-1);
        assert_eq!(v.cursor(), 2);
    }

    #[test]
    fn answer_latches_cursor_text() {
        let mut v = view();
        v.move_cursor(1);
        assert!(v.answer());
        assert_eq!(v.answered(), Some(1));
        assert_eq!(v.answered_text(), Some("b"));
    }

    #[test]
    fn second_answer_returns_false() {
        let mut v = view();
        assert!(v.answer());
        v.move_cursor(1);
        assert!(!v.answer());
        assert_eq!(v.answered_text(), Some("a"));
    }

    #[test]
    fn reset_clears_answer_and_cursor() {
        let mut v = view();
        v.move_cursor(2);
        assert!(v.answer());
        v.reset();
        assert_eq!(v.answered(), None);
        assert_eq!(v.answered_text(), None);
        assert_eq!(v.cursor(), 0);
    }

    #[test]
    fn empty_options_answer_false() {
        let mut v = QuestionView::new("t", vec![]);
        v.move_cursor(1);
        assert!(!v.answer());
        assert_eq!(v.answered_text(), None);
    }

    #[test]
    fn caps_title_and_options() {
        let long = "x".repeat(300);
        let opts = vec!["y".repeat(300); 10];
        let v = QuestionView::new(long, opts);
        assert_eq!(v.title().chars().count(), TITLE_CAP);
        assert_eq!(v.options().len(), OPTION_CAP);
        assert!(v
            .options()
            .iter()
            .all(|o| o.chars().count() == OPTION_LEN_CAP));
    }
}
