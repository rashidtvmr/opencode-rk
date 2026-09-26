//! Prompt trait flags (slash / mention / paste).
//!
//! TS truth: `packages/tui/src/prompt/traits.ts` (`computePromptTraits`).
//! The TS side describes editor capture/status; this bridge helper classifies
//! raw prompt text so native callers can pre-route slash commands, mentions,
//! and pasted blobs without linking the TS editor.

#![forbid(unsafe_code)]

/// Char count above which input counts as a paste (1 KiB chars).
pub const PASTE_THRESHOLD_CHARS: usize = 1024;

/// Lightweight flags describing raw prompt text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PromptTraits {
    /// Text starts with `/` (slash command).
    pub slash: bool,
    /// Text contains `@` (mention reference).
    pub mention: bool,
    /// Text exceeds [`PASTE_THRESHOLD_CHARS`] chars (pasted blob).
    pub paste: bool,
}

impl PromptTraits {
    /// Classify raw prompt text.
    pub fn detect(text: &str) -> Self {
        Self {
            slash: text.starts_with('/'),
            mention: text.contains('@'),
            paste: text.chars().count() > PASTE_THRESHOLD_CHARS,
        }
    }

    /// True when any flag is set.
    pub fn has_any(&self) -> bool {
        self.slash || self.mention || self.paste
    }
}

/// Classify raw prompt text (free-function form).
pub fn detect(text: &str) -> PromptTraits {
    PromptTraits::detect(text)
}

/// True when any flag in `traits` is set (free-function form).
pub fn has_any(traits: &PromptTraits) -> bool {
    traits.has_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_detect() {
        let t = PromptTraits::detect("/help me");
        assert!(t.slash);
        assert!(!t.mention);
    }

    #[test]
    fn mention_detect() {
        let t = detect("hey @alice review");
        assert!(t.mention);
        assert!(!t.slash);
    }

    #[test]
    fn paste_threshold() {
        let big = "x".repeat(PASTE_THRESHOLD_CHARS + 1);
        assert!(detect(&big).paste);
        let edge = "x".repeat(PASTE_THRESHOLD_CHARS);
        assert!(!detect(&edge).paste);
    }

    #[test]
    fn empty_none() {
        let t = detect("");
        assert_eq!(t, PromptTraits::default());
        assert!(!t.has_any());
        assert!(!has_any(&t));
    }

    #[test]
    fn combined_flags() {
        let big = format!("/tell @bob {}", "y".repeat(PASTE_THRESHOLD_CHARS + 1));
        let t = detect(&big);
        assert!(t.slash && t.mention && t.paste);
        assert!(t.has_any());
    }

    #[test]
    fn slash_only_single_char() {
        let t = detect("/");
        assert!(t.slash);
        assert!(t.has_any());
    }
}
