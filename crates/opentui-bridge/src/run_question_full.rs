#![forbid(unsafe_code)]
//! Multi-question flow (port of footer.question.tsx tabbed + Confirm path).
use crate::question_gate::QuestionGate;
/// Max questions held.
pub const FLOW_CAP: usize = 8;
/// Ordered single-select gates; Confirm reads [`Self::done_answers`].
// ponytail: single-select only; add multi flag when TS multi-tab lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionFlow {
    pub gates: Vec<QuestionGate>,
}
impl QuestionFlow {
    pub fn new() -> Self {
        Self { gates: Vec::new() }
    }
    /// Push gate; false when full (8).
    pub fn ask(&mut self, title: &str, opts: Vec<String>) -> bool {
        if self.gates.len() >= FLOW_CAP {
            return false;
        }
        self.gates.push(QuestionGate::ask_single(title, opts));
        true
    }
    /// Answer gate idx with option opt; OOB false.
    pub fn answer(&mut self, idx: usize, opt: usize) -> bool {
        match self.gates.get_mut(idx) {
            Some(g) => g.toggle(opt),
            None => false,
        }
    }
    /// Picked labels per gate, in order.
    pub fn done_answers(&self) -> Vec<Vec<String>> {
        self.gates.iter().map(|g| g.answers()).collect()
    }
}
impl Default for QuestionFlow {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn flow2() -> QuestionFlow {
        let mut f = QuestionFlow::new();
        f.ask("a", vec!["x".into(), "y".into()]);
        f.ask("b", vec!["p".into(), "q".into()]);
        f
    }
    #[test]
    fn new_empty() {
        assert!(QuestionFlow::new().done_answers().is_empty());
    }
    #[test]
    fn ask_true_until_cap() {
        let mut f = QuestionFlow::new();
        for i in 0..FLOW_CAP {
            assert!(f.ask("t", vec![format!("o{i}")]));
        }
        assert!(!f.ask("full", vec!["z".into()]));
        assert_eq!(f.gates.len(), FLOW_CAP);
    }
    #[test]
    fn answer_routes_to_gate() {
        let mut f = flow2();
        assert!(f.answer(1, 0));
        assert_eq!(f.done_answers()[1], vec!["p".to_string()]);
        assert!(f.done_answers()[0].is_empty());
    }
    #[test]
    fn answer_oob_false() {
        let mut f = flow2();
        assert!(!f.answer(9, 0));
        assert!(!f.answer(0, 9));
    }
    #[test]
    fn single_replaces_within_gate() {
        let mut f = flow2();
        f.answer(0, 0);
        f.answer(0, 1);
        assert_eq!(f.done_answers()[0], vec!["y".to_string()]);
    }
    #[test]
    fn done_answers_order() {
        let mut f = flow2();
        f.answer(0, 1);
        f.answer(1, 1);
        assert_eq!(
            f.done_answers(),
            vec![vec!["y".to_string()], vec!["q".to_string()]]
        );
    }
}
