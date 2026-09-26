//! Prompt part trait (mirrors `packages/tui/src/prompt/traits.ts` role/kind split).
#![forbid(unsafe_code)]

/// Max chars per text part (4KiB chars).
pub const MAX_TEXT_CHARS: usize = 4096;

/// Minimal prompt part: a kind tag plus its text.
pub trait PromptTrait {
    fn kind(&self) -> &str;
    fn text(&self) -> &str;
}

/// Plain text prompt part, capped at [`MAX_TEXT_CHARS`] chars.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextPart {
    text: String,
}

impl TextPart {
    #[must_use]
    pub fn new(s: &str) -> Self {
        Self {
            text: s.chars().take(MAX_TEXT_CHARS).collect(),
        }
    }
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl PromptTrait for TextPart {
    fn kind(&self) -> &str {
        "text"
    }
    fn text(&self) -> &str {
        &self.text
    }
}

/// Kind tag of any prompt part.
pub fn kind_of(p: &dyn PromptTrait) -> &str {
    p.kind()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let p = TextPart::new("hi");
        assert_eq!(p.kind(), "text");
        assert_eq!(p.text(), "hi");
        assert_eq!(kind_of(&p), "text");
    }

    #[test]
    fn cap_4kib() {
        let p = TextPart::new(&"y".repeat(MAX_TEXT_CHARS + 10));
        assert_eq!(p.text().chars().count(), MAX_TEXT_CHARS);
    }

    #[test]
    fn empty() {
        let p = TextPart::new("");
        assert_eq!(p.text(), "");
        assert_eq!(kind_of(&p), "text");
    }
}
