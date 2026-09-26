//! Question tabs with per-tab answers and confirm-gated submit.
//!
//! Mirrors `run/question.shared.ts` `questionTabs`/`questionConfirm`: a
//! capped tab strip, one answer slot per tab, an explicit confirm step, and
//! a submit that only fires once confirmed (then clears the answers).

/// Maximum tabs held by [`QuestionTabs`]; extras are dropped at construction.
pub const TAB_CAP: usize = 8;

/// Tab strip with per-tab answer slots and a confirm gate before submit.
pub struct QuestionTabs {
    tabs: Vec<String>,
    selected: usize,
    answers: Vec<String>,
    confirmed: bool,
}

impl QuestionTabs {
    /// Build from tab labels; truncates to [`TAB_CAP`], selects tab 0.
    pub fn new(mut tabs: Vec<String>) -> Self {
        tabs.truncate(TAB_CAP);
        let answers = vec![String::new(); tabs.len()];
        Self {
            tabs,
            selected: 0,
            answers,
            confirmed: false,
        }
    }

    /// Select a tab; out-of-range indexes keep the current selection.
    pub fn select(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.selected = index;
            self.confirmed = false;
        }
    }

    /// Store an answer; out-of-range indexes are ignored (length unchanged).
    pub fn answer(&mut self, index: usize, text: String) {
        if let Some(slot) = self.answers.get_mut(index) {
            *slot = text;
            self.confirmed = false;
        }
    }

    /// Confirm only when every tab has a non-empty answer; else stays false.
    pub fn confirm(&mut self) {
        self.confirmed = !self.tabs.is_empty() && self.answers.iter().all(|a| !a.is_empty());
    }

    /// Take the answers iff confirmed; clears slots and resets the gate.
    pub fn submit(&mut self) -> Option<Vec<String>> {
        if !self.confirmed {
            return None;
        }
        self.confirmed = false;
        let out = self.answers.clone();
        for slot in &mut self.answers {
            slot.clear();
        }
        Some(out)
    }

    /// Tab labels.
    pub fn tabs(&self) -> &[String] {
        &self.tabs
    }

    /// Selected tab index.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Current answer slots.
    pub fn answers(&self) -> &[String] {
        &self.answers
    }

    /// Whether submit is armed.
    pub fn confirmed(&self) -> bool {
        self.confirmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabs(n: usize) -> QuestionTabs {
        QuestionTabs::new((0..n).map(|i| format!("t{i}")).collect())
    }

    #[test]
    fn tab_cap_truncates_at_8() {
        let q = tabs(12);
        assert_eq!(q.tabs().len(), TAB_CAP);
        assert_eq!(q.answers().len(), TAB_CAP);
    }

    #[test]
    fn select_oob_keeps_selection() {
        let mut q = tabs(3);
        q.select(1);
        q.select(9);
        assert_eq!(q.selected(), 1);
    }

    #[test]
    fn answer_oob_keeps_len() {
        let mut q = tabs(2);
        q.answer(5, "x".to_string());
        assert_eq!(q.answers().len(), 2);
        assert!(q.answers().iter().all(|a| a.is_empty()));
    }

    #[test]
    fn confirm_gated_until_all_answered() {
        let mut q = tabs(2);
        q.answer(0, "a".to_string());
        q.confirm();
        assert!(!q.confirmed());
        q.answer(1, "b".to_string());
        q.confirm();
        assert!(q.confirmed());
    }

    #[test]
    fn submit_none_until_confirmed() {
        let mut q = tabs(1);
        q.answer(0, "a".to_string());
        assert_eq!(q.submit(), None);
    }

    #[test]
    fn submit_returns_answers_and_clears() {
        let mut q = tabs(2);
        q.answer(0, "a".to_string());
        q.answer(1, "b".to_string());
        q.confirm();
        assert_eq!(q.submit(), Some(vec!["a".to_string(), "b".to_string()]));
        assert!(q.answers().iter().all(|a| a.is_empty()));
        assert!(!q.confirmed());
        assert_eq!(q.submit(), None);
    }
}

/// Maximum chars held by [`AnswerKind::Text`]; longer input is truncated.
pub const ANSWER_TEXT_CAP: usize = 512;

/// Answer payload for a single question: capped free text or option index.
pub enum AnswerKind {
    Text(String),
    Pick(usize),
}

impl AnswerKind {
    /// Build text, truncating to [`ANSWER_TEXT_CAP`] chars.
    pub fn text(s: String) -> Self {
        Self::Text(s.chars().take(ANSWER_TEXT_CAP).collect())
    }

    /// Build an option pick.
    pub fn pick(index: usize) -> Self {
        Self::Pick(index)
    }
}

/// Confirm-then-submit-once flow; mirrors the footer question verb gate.
pub struct QuestionFlow {
    pub confirmed: bool,
    pub submitted: bool,
}

impl QuestionFlow {
    /// Fresh flow: unconfirmed, unsubmitted.
    pub fn new() -> Self {
        Self {
            confirmed: false,
            submitted: false,
        }
    }

    /// Arm submit; ignored once submitted.
    pub fn confirm(&mut self) {
        if !self.submitted {
            self.confirmed = true;
        }
    }

    /// Fire once when confirmed; latches submitted, second call is false.
    pub fn submit(&mut self) -> bool {
        if self.confirmed && !self.submitted {
            self.submitted = true;
            return true;
        }
        false
    }

    /// Clear both flags back to pending.
    pub fn reset(&mut self) {
        self.confirmed = false;
        self.submitted = false;
    }

    /// "pending" | "confirmed" | "submitted".
    pub fn state_label(&self) -> &'static str {
        if self.submitted {
            "submitted"
        } else if self.confirmed {
            "confirmed"
        } else {
            "pending"
        }
    }
}

impl Default for QuestionFlow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests2 {
    use super::*;

    #[test]
    fn flow_submit_needs_confirm() {
        let mut f = QuestionFlow::new();
        assert!(!f.submit());
        assert_eq!(f.state_label(), "pending");
    }

    #[test]
    fn flow_submit_true_once_then_false() {
        let mut f = QuestionFlow::new();
        f.confirm();
        assert!(f.submit());
        assert!(!f.submit());
    }

    #[test]
    fn flow_reset_clears_flags() {
        let mut f = QuestionFlow::new();
        f.confirm();
        assert!(f.submit());
        f.reset();
        assert!(!f.confirmed);
        assert!(!f.submitted);
        assert_eq!(f.state_label(), "pending");
        assert!(!f.submit());
    }

    #[test]
    fn flow_state_labels() {
        let mut f = QuestionFlow::new();
        assert_eq!(f.state_label(), "pending");
        f.confirm();
        assert_eq!(f.state_label(), "confirmed");
        assert!(f.submit());
        assert_eq!(f.state_label(), "submitted");
    }

    #[test]
    fn flow_confirm_ignored_after_submit() {
        let mut f = QuestionFlow::new();
        f.confirm();
        assert!(f.submit());
        f.reset();
        f.confirm();
        assert_eq!(f.state_label(), "confirmed");
    }

    #[test]
    fn answer_text_truncates_at_512() {
        let long = "a".repeat(ANSWER_TEXT_CAP + 10);
        match AnswerKind::text(long) {
            AnswerKind::Text(t) => assert_eq!(t.chars().count(), ANSWER_TEXT_CAP),
            AnswerKind::Pick(_) => panic!("expected text"),
        }
        match AnswerKind::pick(2) {
            AnswerKind::Pick(i) => assert_eq!(i, 2),
            AnswerKind::Text(_) => panic!("expected pick"),
        }
    }
}
