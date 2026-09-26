//! Question machine: tab store, confirm gate, SDK reply/reject vs question.tsx:14.
#![forbid(unsafe_code)]
pub const TAB_COUNT: usize = 8;
pub const CUSTOM_MAX: usize = 512;
pub struct QuestionMachine {
    tabs: Vec<String>,
    answers: Vec<String>,
}
impl QuestionMachine {
    pub fn new(mut tabs: Vec<String>) -> Self {
        tabs.truncate(TAB_COUNT);
        let answers = vec![String::new(); tabs.len()];
        Self { tabs, answers }
    }
    pub fn store(&mut self, idx: usize, text: String) -> bool {
        match self.answers.get_mut(idx) {
            Some(slot) => {
                *slot = text;
                true
            }
            None => false,
        }
    }
    pub fn tab_at(&self, i: usize) -> usize {
        if i < self.tabs.len() {
            i
        } else {
            0
        }
    }
    pub fn confirm(&self) -> bool {
        !self.tabs.is_empty() && self.answers.iter().all(|a| !a.is_empty())
    }
    pub fn reply(&mut self) -> Option<Vec<String>> {
        if !self.confirm() {
            return None;
        }
        let out = self.answers.clone();
        self.reject();
        Some(out)
    }
    pub fn reject(&mut self) {
        for slot in &mut self.answers {
            slot.clear();
        }
    }
}
pub fn custom_ok(len: usize) -> bool {
    len <= CUSTOM_MAX
}
pub fn confirm_word() -> &'static str {
    "yes"
}
#[cfg(test)]
mod tests {
    use super::*;
    fn machine(n: usize) -> QuestionMachine {
        QuestionMachine::new((0..n).map(|i| format!("t{i}")).collect())
    }
    #[test]
    fn truncates_to_tab_count_fail_closed() {
        let m = machine(12);
        assert_eq!(m.tab_at(7), 7);
        assert_eq!(m.tab_at(8), 0);
        assert_eq!(m.tab_at(99), 0);
    }
    #[test]
    fn custom_ok_bounds_custom_max() {
        assert!(custom_ok(CUSTOM_MAX));
        assert!(!custom_ok(CUSTOM_MAX + 1));
    }
    #[test]
    fn confirm_word_is_yes() {
        assert_eq!(confirm_word(), "yes");
    }
    #[test]
    fn store_confirm_reply_reject() {
        let mut m = machine(1);
        assert!(!m.confirm());
        assert!(m.store(0, "a".to_string()));
        assert!(!m.store(9, "x".to_string()));
        assert_eq!(m.reply(), Some(vec!["a".to_string()]));
        assert_eq!(m.reply(), None);
    }
}
