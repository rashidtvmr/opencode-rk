#![forbid(unsafe_code)]
//! Prompt dialog (mirrors `packages/tui/src/ui/dialog-prompt.tsx:20` title/value + `confirm()` :28-31).
/// Max chars for dialog title (TS `title`, :10).
pub const MAX_TITLE: usize = 128;
/// Max chars for dialog value (TS `textarea.plainText`, :30).
pub const MAX_VALUE: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptDialog {
    pub title: String,
    pub value: String,
    pub done: bool,
}

impl PromptDialog {
    pub fn new(title: &str) -> Result<Self, &'static str> {
        if title.is_empty() || title.chars().count() > MAX_TITLE {
            return Err("bad title");
        }
        Ok(Self {
            title: title.to_string(),
            value: String::new(),
            done: false,
        })
    }
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.done
    }
    pub fn set_title(&mut self, title: &str) -> Result<(), &'static str> {
        if self.done {
            return Err("closed");
        }
        if title.is_empty() || title.chars().count() > MAX_TITLE {
            return Err("bad title");
        }
        self.title = title.to_string();
        Ok(())
    }
    pub fn type_text(&mut self, s: &str) -> Result<(), &'static str> {
        if self.done {
            return Err("closed");
        }
        if self.value.chars().count() + s.chars().count() > MAX_VALUE {
            return Err("value too long");
        }
        self.value.push_str(s);
        Ok(())
    }
    pub fn submit(&mut self) -> Option<String> {
        if self.done {
            return None;
        }
        self.done = true;
        Some(self.value.clone())
    }
    pub fn cancel(&mut self) {
        self.done = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_rejects_bad_title() {
        assert!(PromptDialog::new("").is_err());
        assert!(PromptDialog::new(&"t".repeat(MAX_TITLE + 1)).is_err());
        assert!(PromptDialog::new("Rename session").is_ok());
    }
    #[test]
    fn type_text_appends_and_caps() {
        let mut d = PromptDialog::new("t").unwrap();
        d.type_text("ab").unwrap();
        d.type_text("cd").unwrap();
        assert_eq!(d.value(), "abcd");
        assert!(d.type_text(&"x".repeat(MAX_VALUE)).is_err());
    }
    #[test]
    fn type_text_counts_chars_not_bytes() {
        let mut d = PromptDialog::new("t").unwrap();
        d.type_text(&"e".repeat(MAX_VALUE - 1)).unwrap();
        d.type_text("⚡").unwrap(); // 1 char, 3 bytes: fills cap exactly
        assert!(d.type_text("y").is_err());
    }
    #[test]
    fn submit_returns_value_marks_done() {
        let mut d = PromptDialog::new("t").unwrap();
        d.type_text("hi").unwrap();
        assert_eq!(d.submit(), Some("hi".to_string()));
        assert!(d.is_done());
        assert_eq!(d.submit(), None); // idempotent: second submit None
    }
    #[test]
    fn cancel_blocks_submit() {
        let mut d = PromptDialog::new("t").unwrap();
        d.type_text("hi").unwrap();
        d.cancel();
        assert!(d.is_done());
        assert_eq!(d.submit(), None);
    }
    #[test]
    fn set_title_ok_rejects_closed() {
        let mut d = PromptDialog::new("a").unwrap();
        d.set_title("b").unwrap();
        assert_eq!(d.title(), "b");
        assert!(d.set_title("").is_err());
        d.cancel();
        assert!(d.set_title("c").is_err());
        assert!(d.type_text("z").is_err());
    }
}
