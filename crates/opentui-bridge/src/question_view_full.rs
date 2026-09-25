#![forbid(unsafe_code)]
//! Rendered rows for [`crate::question_gate::QuestionGate`] (single-select).
//!
//! TS truth (`crate::question_view` cursor/pick model surfaced through
//! `crate::question_gate::QuestionGate`): read-only here, never edited.
//!
//! `ponytail:` joined answers + static hint, char-clip only; upgrade to
//! per-option cursor rows when a real caller needs them.

use crate::question_gate::QuestionGate;

/// Max rows returned by [`QuestionRender::lines`].
pub const MAX_ROWS: usize = 8;

/// Hint appended after answers.
pub const HINT: &str = "enter to confirm";

fn clip(s: &str, width: usize) -> String {
    s.chars().take(width.max(1)).collect()
}

/// Row renderer over a single-select gate.
#[derive(Debug, Clone)]
pub struct QuestionRender {
    pub gate: QuestionGate,
}

impl QuestionRender {
    pub fn new(gate: QuestionGate) -> Self {
        Self { gate }
    }
    /// Joined answers plus hint, each clipped to `width`, capped at 8 rows.
    pub fn lines(&self, width: usize) -> Vec<String> {
        let answers = self.gate.answers();
        let mut rows = Vec::with_capacity(2);
        if !answers.is_empty() {
            rows.push(clip(&answers.join(", "), width));
        }
        rows.push(clip(HINT, width));
        rows.truncate(MAX_ROWS);
        rows
    }

    /// Number of picked answers.
    pub fn count(&self) -> usize {
        self.gate.answers().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn render(picks: &[usize]) -> QuestionRender {
        let mut gate = QuestionGate::ask_single("pick", vec!["a".into(), "b".into(), "c".into()]);
        for &i in picks {
            gate.toggle(i);
        }
        QuestionRender::new(gate)
    }

    #[test]
    fn empty_shows_hint_only() {
        let r = render(&[]);
        assert_eq!(r.lines(80), vec![HINT.to_string()]);
        assert_eq!(r.count(), 0);
    }

    #[test]
    fn answer_joined_before_hint() {
        let r = render(&[1]);
        assert_eq!(r.lines(80), vec!["b".to_string(), HINT.to_string()]);
        assert_eq!(r.count(), 1);
    }

    #[test]
    fn lines_clip_to_width() {
        let r = render(&[0]);
        for row in r.lines(3) {
            assert!(row.chars().count() <= 3);
        }
    }

    #[test]
    fn rows_capped_and_width_zero_safe() {
        let r = render(&[2]);
        assert!(r.lines(0).len() <= MAX_ROWS);
        assert_eq!(MAX_ROWS, 8);
    }
}
