#![forbid(unsafe_code)]
//! Draft + submit history (ctx for `prompt.tsx` PromptRef + `prompt_composer.rs`).
//! Pure data: bounded draft, front-push history, browse cursor. No I/O.

/// Draft cap in chars (8KiB chars).
pub const MAX_DRAFT_CHARS: usize = 8192;
/// History entries kept (newest front).
pub const MAX_HISTORY: usize = 50;
/// Per-entry cap in chars (4KiB chars).
pub const MAX_ENTRY_CHARS: usize = 4096;

fn take_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Draft with submit history. `hcursor`: `None` = editing, `Some(i)` = browsing.
#[derive(Debug, Clone, Default)]
pub struct PromptCtx {
    draft: String,
    history: Vec<String>,
    hcursor: Option<usize>,
}

impl PromptCtx {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn draft(&self) -> &str {
        &self.draft
    }
    #[must_use]
    pub fn history(&self) -> &[String] {
        &self.history
    }
    #[must_use]
    pub fn hcursor(&self) -> Option<usize> {
        self.hcursor
    }
    /// Set draft, truncated to [`MAX_DRAFT_CHARS`] chars. Exits browse mode.
    pub fn set_draft(&mut self, s: &str) {
        self.draft = take_chars(s, MAX_DRAFT_CHARS);
        self.hcursor = None;
    }
    /// Submit draft: empty -> false; else push front (entry capped at
    /// [`MAX_ENTRY_CHARS`]), evict past [`MAX_HISTORY`], clear draft.
    pub fn submit(&mut self) -> bool {
        if self.draft.is_empty() {
            return false;
        }
        self.history
            .insert(0, take_chars(&self.draft, MAX_ENTRY_CHARS));
        if self.history.len() > MAX_HISTORY {
            self.history.pop();
        }
        self.draft.clear();
        self.hcursor = None;
        true
    }
    /// Older entry (`None` -> newest, `i` -> `i+1`). `None` at end/empty.
    pub fn hist_prev(&mut self) -> Option<&str> {
        if self.history.is_empty() {
            return None;
        }
        let n = match self.hcursor {
            None => 0,
            Some(i) => i + 1,
        };
        if n >= self.history.len() {
            return None;
        }
        self.hcursor = Some(n);
        Some(self.history[n].as_str())
    }
    /// Newer entry; at newest exits browse mode and returns `None`.
    pub fn hist_next(&mut self) -> Option<&str> {
        match self.hcursor {
            None => None,
            Some(0) => {
                self.hcursor = None;
                None
            }
            Some(i) => {
                let p = i - 1;
                self.hcursor = Some(p);
                Some(self.history[p].as_str())
            }
        }
    }
    /// Clear draft, history, and browse cursor.
    pub fn clear(&mut self) {
        self.draft.clear();
        self.history.clear();
        self.hcursor = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn submit_empty_false() {
        let mut p = PromptCtx::new();
        assert!(!p.submit());
        p.set_draft("");
        assert!(!p.submit());
        assert!(p.history().is_empty());
    }
    #[test]
    fn submit_pushes_front_clears_draft() {
        let mut p = PromptCtx::new();
        p.set_draft("a");
        assert!(p.submit());
        p.set_draft("b");
        assert!(p.submit());
        assert_eq!(p.history(), &["b".to_string(), "a".to_string()]);
        assert_eq!(p.draft(), "");
        assert_eq!(p.hcursor(), None);
    }
    #[test]
    fn prev_next_walk() {
        let mut p = PromptCtx::new();
        assert_eq!(p.hist_prev(), None);
        assert_eq!(p.hist_next(), None);
        p.set_draft("a");
        assert!(p.submit());
        p.set_draft("b");
        assert!(p.submit());
        assert_eq!(p.hist_prev(), Some("b"));
        assert_eq!(p.hist_prev(), Some("a"));
        assert_eq!(p.hist_prev(), None);
        assert_eq!(p.hist_next(), Some("b"));
        assert_eq!(p.hist_next(), None);
        assert_eq!(p.hcursor(), None);
    }
    #[test]
    fn evicts_past_50() {
        let mut p = PromptCtx::new();
        for i in 0..MAX_HISTORY + 5 {
            p.set_draft(&format!("e{i}"));
            assert!(p.submit());
        }
        assert_eq!(p.history().len(), MAX_HISTORY);
        assert_eq!(p.history()[0], format!("e{}", MAX_HISTORY + 4));
        assert!(!p.history().iter().any(|e| e == "e0"));
    }
    #[test]
    fn clear_resets_all() {
        let mut p = PromptCtx::new();
        p.set_draft("x".repeat(MAX_DRAFT_CHARS + 10).as_str());
        assert_eq!(p.draft().chars().count(), MAX_DRAFT_CHARS);
        assert!(p.submit());
        p.hist_prev();
        p.clear();
        assert_eq!(p.draft(), "");
        assert!(p.history().is_empty());
        assert_eq!(p.hcursor(), None);
    }
    #[test]
    fn entry_capped_at_4kib() {
        let mut p = PromptCtx::new();
        p.set_draft("y".repeat(MAX_ENTRY_CHARS + 100).as_str());
        assert!(p.submit());
        assert_eq!(p.history()[0].chars().count(), MAX_ENTRY_CHARS);
    }
}
