#![forbid(unsafe_code)]
//! Prompt tsx state (mirrors `packages/tui/src/ui/dialog-prompt.tsx:9` value :13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptTsx {
    pub label: String,
    pub value: String,
}

impl PromptTsx {
    #[must_use]
    pub fn new(label: &str) -> Self {
        Self {
            label: label.trim().chars().take(128).collect(),
            value: String::new(),
        }
    }
    pub fn set_value(&mut self, v: &str) {
        self.value = v.chars().take(512).collect();
    }
    #[must_use]
    pub fn value_of(&self) -> &str {
        &self.value
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_trims_and_caps_label() {
        assert_eq!(PromptTsx::new("  hi  ").label, "hi");
        assert_eq!(PromptTsx::new(&"a".repeat(200)).label.len(), 128);
    }
    #[test]
    fn set_value_caps_at_512() {
        let mut p = PromptTsx::new("l");
        p.set_value(&"b".repeat(600));
        assert_eq!(p.value_of().len(), 512);
        assert!(!p.is_empty());
    }
    #[test]
    fn empty_by_default() {
        let p = PromptTsx::new("l");
        assert!(p.is_empty());
        assert_eq!(p.value_of(), "");
    }
}
