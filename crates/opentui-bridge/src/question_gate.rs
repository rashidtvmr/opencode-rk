#![forbid(unsafe_code)]
//! Single-question gate over [`QuestionFull`] (single-select path).

use crate::question_full::QuestionFull;

/// Single-select gate; answers latch via `toggle`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionGate {
    pub full: QuestionFull,
}

impl QuestionGate {
    /// Build single-select gate from title + options.
    pub fn ask_single(title: &str, opts: Vec<String>) -> Self {
        Self {
            full: QuestionFull::new(title.to_owned(), title.to_owned(), opts, false),
        }
    }
    /// Select index; OOB false.
    pub fn toggle(&mut self, i: usize) -> bool {
        self.full.toggle(i)
    }
    /// Picked labels.
    pub fn answers(&self) -> Vec<String> {
        self.full.done()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn gate() -> QuestionGate {
        QuestionGate::ask_single("pick", vec!["a".into(), "b".into()])
    }
    #[test]
    fn ask_single_empty_answers() {
        assert!(gate().answers().is_empty());
    }
    #[test]
    fn toggle_selects_answer() {
        let mut g = gate();
        assert!(g.toggle(1));
        assert_eq!(g.answers(), vec!["b".to_string()]);
    }
    #[test]
    fn toggle_oob_false() {
        let mut g = gate();
        assert!(!g.toggle(9));
        assert!(g.answers().is_empty());
    }
    #[test]
    fn single_replaces() {
        let mut g = gate();
        g.toggle(0);
        g.toggle(1);
        assert_eq!(g.answers(), vec!["b".to_string()]);
    }
    #[test]
    fn title_flows_to_id_header() {
        let g = gate();
        assert_eq!(g.full.id(), "pick");
        assert_eq!(g.full.header(), "pick");
    }
}
