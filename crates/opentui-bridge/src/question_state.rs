//! Question dialog state: tabs, per-tab answers, custom text, multi gate.
//!
//! Mirrors `run_question.rs` gate conventions (`TAB_CAP`, per-tab answer
//! slots, confirm-gated submit that clears): answers must all be non-empty,
//! unless a custom response bypasses the per-tab requirement.

#![forbid(unsafe_code)]

/// Maximum tabs held; extras dropped at construction.
pub const TAB_CAP: usize = 8;

/// Maximum chars retained in the custom response.
pub const CUSTOM_CAP: usize = 512;

/// Dialog state for a tabbed/multi question prompt.
pub struct QuestionState {
    tabs: Vec<String>,
    answers: Vec<String>,
    custom: String,
    multi: bool,
}

impl QuestionState {
    /// Build from tab labels; truncates tabs to [`TAB_CAP`].
    pub fn new(mut tabs: Vec<String>, multi: bool) -> Self {
        tabs.truncate(TAB_CAP);
        let answers = vec![String::new(); tabs.len()];
        Self {
            tabs,
            answers,
            custom: String::new(),
            multi,
        }
    }

    /// Select is a bounds check; `false` when `idx` is out of range.
    pub fn select(&self, idx: usize) -> bool {
        idx < self.tabs.len()
    }

    /// Store an answer; out-of-range indexes return `false`.
    pub fn answer(&mut self, idx: usize, text: String) -> bool {
        match self.answers.get_mut(idx) {
            Some(slot) => {
                *slot = text;
                true
            }
            None => false,
        }
    }

    /// Set custom text, truncating to [`CUSTOM_CAP`] chars.
    pub fn set_custom(&mut self, text: String) {
        self.custom = text.chars().take(CUSTOM_CAP).collect();
    }

    /// Ready when every tab answered, or custom text is non-empty.
    pub fn confirm(&self) -> bool {
        if !self.custom.is_empty() {
            return true;
        }
        !self.tabs.is_empty() && self.answers.iter().all(|a| !a.is_empty())
    }

    /// Take answers (custom wins when set); clears slots after submit.
    pub fn submit(&mut self) -> Option<Vec<String>> {
        if !self.confirm() {
            return None;
        }
        let out = if self.custom.is_empty() {
            self.answers.clone()
        } else {
            vec![self.custom.clone()]
        };
        for slot in &mut self.answers {
            slot.clear();
        }
        self.custom.clear();
        Some(out)
    }

    /// Tab labels.
    pub fn tabs(&self) -> &[String] {
        &self.tabs
    }

    /// Current answer slots.
    pub fn answers(&self) -> &[String] {
        &self.answers
    }

    /// Custom response text.
    pub fn custom(&self) -> &str {
        &self.custom
    }

    /// Whether multi-answer mode was requested.
    pub fn is_multi(&self) -> bool {
        self.multi
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(n: usize) -> QuestionState {
        QuestionState::new((0..n).map(|i| format!("t{i}")).collect(), false)
    }

    #[test]
    fn cap_truncates_at_8() {
        let q = QuestionState::new((0..12).map(|i| format!("t{i}")).collect(), true);
        assert_eq!(q.tabs().len(), TAB_CAP);
        assert_eq!(q.answers().len(), TAB_CAP);
        assert!(q.is_multi());
    }

    #[test]
    fn select_oob_false() {
        let q = state(2);
        assert!(q.select(0));
        assert!(!q.select(9));
    }

    #[test]
    fn answer_oob_false() {
        let mut q = state(2);
        assert!(!q.answer(5, "x".to_string()));
        assert_eq!(q.answers().len(), 2);
    }

    #[test]
    fn custom_truncates_and_bypasses_confirm() {
        let mut q = state(2);
        q.set_custom("y".repeat(600));
        assert_eq!(q.custom().chars().count(), CUSTOM_CAP);
        assert!(q.confirm());
        assert_eq!(q.submit(), Some(vec!["y".repeat(CUSTOM_CAP)]));
    }

    #[test]
    fn confirm_gated_until_all_answered() {
        let mut q = state(2);
        assert!(!q.confirm());
        q.answer(0, "a".to_string());
        assert!(!q.confirm());
        q.answer(1, "b".to_string());
        assert!(q.confirm());
    }

    #[test]
    fn submit_none_until_confirmed_then_clears() {
        let mut q = state(1);
        q.answer(0, "a".to_string());
        q.set_custom(String::new());
        assert!(q.confirm());
        assert_eq!(q.submit(), Some(vec!["a".to_string()]));
        assert!(q.answers().iter().all(|a| a.is_empty()));
        assert_eq!(q.submit(), None);
    }
}
