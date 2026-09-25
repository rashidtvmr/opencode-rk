//! Session question prompt: title plus picked option index.
//! Mirrors `packages/tui/src/routes/session/question.tsx:14` `QuestionPrompt`.
#![forbid(unsafe_code)]

pub const TITLE_CAP: usize = 128;
pub struct SessQues {
    title: String,
    picked: Option<usize>,
}
impl SessQues {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            picked: None,
        }
    }
    pub fn ask(&mut self, title: &str) {
        self.title = title.chars().take(TITLE_CAP).collect();
        self.picked = None;
    }
    pub fn pick(&mut self, idx: usize) {
        self.picked = Some(idx);
    }
    pub fn picked(&self) -> Option<usize> {
        self.picked
    }
    pub fn title(&self) -> &str {
        &self.title
    }
}
impl Default for SessQues {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ask_caps_title_and_clears_pick() {
        let mut q = SessQues::new();
        q.pick(2);
        q.ask(&"t".repeat(200));
        assert_eq!(q.title().chars().count(), TITLE_CAP);
        assert_eq!(q.picked(), None);
    }
    #[test]
    fn pick_roundtrip() {
        let mut q = SessQues::new();
        assert_eq!(q.picked(), None);
        q.pick(1);
        assert_eq!(q.picked(), Some(1));
    }
    #[test]
    fn ask_empty_title() {
        let mut q = SessQues::default();
        q.ask("");
        assert_eq!(q.title(), "");
        assert_eq!(q.picked(), None);
    }
}
