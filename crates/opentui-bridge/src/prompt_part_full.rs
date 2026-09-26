//! Full prompt part (TS: `prompt/part.ts`).
#![forbid(unsafe_code)]

pub const MAX_KIND_CHARS: usize = 32;
pub const MAX_TEXT_CHARS: usize = 4096;
pub const PREVIEW_CHARS: usize = 64;

fn take(s: &str, cap: usize) -> String {
    s.chars().take(cap).collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptPart {
    pub kind: String,
    pub text: String,
}

impl PromptPart {
    pub fn new(kind: &str, text: &str) -> Self {
        Self {
            kind: take(kind, MAX_KIND_CHARS),
            text: take(text, MAX_TEXT_CHARS),
        }
    }

    pub fn is_text(&self) -> bool {
        self.kind == "text"
    }

    pub fn preview(&self) -> String {
        take(&self.text, PREVIEW_CHARS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_kind_len() {
        let p = PromptPart::new(&"k".repeat(40), "hi");
        assert_eq!(p.kind.chars().count(), MAX_KIND_CHARS);
    }

    #[test]
    fn caps_text_len() {
        let p = PromptPart::new("text", &"x".repeat(5000));
        assert_eq!(p.text.chars().count(), MAX_TEXT_CHARS);
    }

    #[test]
    fn is_text_flag() {
        assert!(PromptPart::new("text", "hi").is_text());
        assert!(!PromptPart::new("image", "hi").is_text());
    }

    #[test]
    fn preview_truncates() {
        let p = PromptPart::new("text", &"y".repeat(100));
        assert_eq!(p.preview().chars().count(), PREVIEW_CHARS);
    }

    #[test]
    fn preview_short_passthrough() {
        let p = PromptPart::new("text", "hello");
        assert_eq!(p.preview(), "hello");
    }
}
