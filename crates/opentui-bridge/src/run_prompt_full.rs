#![forbid(unsafe_code)]

//! Full run-prompt buffer: bounded draft + submit history.
//! TS truth: `packages/opencode/src/cli/cmd/run/footer.prompt.tsx`
//! (`createPromptState` history nav); normalize via
//! `crate::prompt_full::normalize_paste` (CRLF/CR fold, ANSI strip, 8KiB cap).

use crate::prompt_full::normalize_paste;

/// Draft cap in chars (8KiB chars).
pub const MAX_DRAFT_CHARS: usize = 8192;
/// History entries kept (oldest evicted).
pub const MAX_HISTORY: usize = 50;

/// Draft with submit history. Push order: newest at back; [`recall`] = last.
#[derive(Debug, Clone, Default)]
pub struct PromptFull {
    pub draft: String,
    pub history: Vec<String>,
}

impl PromptFull {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Set draft, truncated to [`MAX_DRAFT_CHARS`] chars.
    pub fn set_draft(&mut self, s: &str) {
        self.draft = s.chars().take(MAX_DRAFT_CHARS).collect();
    }
    /// Normalize `raw`; empty (trimmed) -> `None` (draft kept as normalized).
    /// Else push history (evict oldest past cap), clear draft, return `Some`.
    pub fn submit(&mut self, raw: &str) -> Option<String> {
        let text = normalize_paste(raw);
        if text.trim().is_empty() {
            self.draft = text;
            return None;
        }
        self.history.push(text.clone());
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        }
        self.draft.clear();
        Some(text)
    }
    /// Most recent submitted entry.
    #[must_use]
    pub fn recall(&self) -> Option<&str> {
        self.history.last().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_none_keeps_draft() {
        let mut p = PromptFull::new();
        assert_eq!(p.submit("   "), None);
        assert_eq!(p.recall(), None);
    }
    #[test]
    fn submit_pushes_clears_returns() {
        let mut p = PromptFull::new();
        p.set_draft("typing");
        assert_eq!(p.submit("hi"), Some("hi".to_string()));
        assert_eq!(p.draft, "");
        assert_eq!(p.recall(), Some("hi"));
    }
    #[test]
    fn crlf_normalized_on_submit() {
        let mut p = PromptFull::new();
        assert_eq!(p.submit("a\r\nb"), Some("a\nb".to_string()));
    }
    #[test]
    fn recall_last_after_many() {
        let mut p = PromptFull::new();
        p.submit("a");
        p.submit("b");
        assert_eq!(p.recall(), Some("b"));
        assert_eq!(p.history.len(), 2);
    }
    #[test]
    fn evicts_oldest_past_50() {
        let mut p = PromptFull::new();
        for i in 0..MAX_HISTORY + 5 {
            assert!(p.submit(&format!("e{i}")).is_some());
        }
        assert_eq!(p.history.len(), MAX_HISTORY);
        assert_eq!(p.recall(), Some(format!("e{}", MAX_HISTORY + 4).as_str()));
        assert!(!p.history.iter().any(|e| e == "e0"));
    }
    #[test]
    fn draft_capped_at_8kib() {
        let mut p = PromptFull::new();
        p.set_draft(&"x".repeat(MAX_DRAFT_CHARS + 10));
        assert_eq!(p.draft.chars().count(), MAX_DRAFT_CHARS);
    }
}
