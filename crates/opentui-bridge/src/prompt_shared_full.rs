//! Full prompt text with attachments (TS: `prompt.shared.ts`).
#![forbid(unsafe_code)]

pub const MAX_TEXT_CHARS: usize = 8192;
pub const MAX_ATTACHMENTS: usize = 16;
pub const MAX_ATTACHMENT_CHARS: usize = 512;

#[derive(Clone, Debug, Default)]
pub struct PromptSharedFull {
    text: String,
    attachments: Vec<String>,
}

impl PromptSharedFull {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, text: &str) {
        self.text = text.chars().take(MAX_TEXT_CHARS).collect();
    }

    pub fn attach(&mut self, path: &str) -> bool {
        if self.attachments.len() >= MAX_ATTACHMENTS {
            return false;
        }
        self.attachments
            .push(path.chars().take(MAX_ATTACHMENT_CHARS).collect());
        true
    }

    pub fn detach(&mut self, i: usize) -> bool {
        if i >= self.attachments.len() {
            return false;
        }
        self.attachments.remove(i);
        true
    }

    pub fn is_ready(&self) -> bool {
        !self.text.trim().is_empty()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn attachments(&self) -> &[String] {
        &self.attachments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_truncates_to_cap() {
        let mut p = PromptSharedFull::new();
        p.set(&"x".repeat(MAX_TEXT_CHARS + 10));
        assert_eq!(p.text().chars().count(), MAX_TEXT_CHARS);
    }

    #[test]
    fn attach_caps_at_16() {
        let mut p = PromptSharedFull::new();
        for i in 0..MAX_ATTACHMENTS {
            assert!(p.attach(&format!("f-{i}")));
        }
        assert!(!p.attach("overflow"));
        assert_eq!(p.attachments().len(), MAX_ATTACHMENTS);
    }

    #[test]
    fn attach_truncates_each_to_512() {
        let mut p = PromptSharedFull::new();
        assert!(p.attach(&"y".repeat(MAX_ATTACHMENT_CHARS + 5)));
        assert_eq!(p.attachments()[0].chars().count(), MAX_ATTACHMENT_CHARS);
    }

    #[test]
    fn detach_bounds() {
        let mut p = PromptSharedFull::new();
        assert!(!p.detach(0));
        p.attach("a");
        p.attach("b");
        assert!(p.detach(0));
        assert_eq!(p.attachments(), &["b".to_string()]);
        assert!(!p.detach(5));
    }

    #[test]
    fn ready_blank_false() {
        let mut p = PromptSharedFull::new();
        assert!(!p.is_ready());
        p.set("   ");
        assert!(!p.is_ready());
    }

    #[test]
    fn ready_true_and_default_empty() {
        let mut p = PromptSharedFull::new();
        assert!(p.text().is_empty());
        assert!(p.attachments().is_empty());
        p.set("hello");
        assert!(p.is_ready());
    }
}
