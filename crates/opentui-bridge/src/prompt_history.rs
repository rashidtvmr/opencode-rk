#![forbid(unsafe_code)]
//! Prompt history + frecency (`packages/tui/src/prompt/history.tsx`, `frecency.tsx`).
//! Recency = front order, frequency = `uses`; `suggest` ranks `uses`-desc.
pub const MAX_TEXT: usize = 4096;
pub const MAX_ENTRIES: usize = 200;
pub const MAX_SUGGEST: usize = 8;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistEntry {
    pub text: String,
    pub uses: u32,
}
#[derive(Debug, Clone, Default)]
pub struct PromptHistory {
    entries: Vec<HistEntry>,
    cursor: Option<usize>,
}
fn floor(s: &str, max: usize) -> usize {
    let mut e = max.min(s.len());
    while e > 0 && !s.is_char_boundary(e) {
        e -= 1;
    }
    e
}
impl PromptHistory {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    #[must_use]
    pub fn entries(&self) -> &[HistEntry] {
        &self.entries
    }
    #[must_use]
    pub fn cursor(&self) -> Option<usize> {
        self.cursor
    }
    pub fn record(&mut self, text: &str) {
        let t = text.trim();
        if t.is_empty() {
            return;
        }
        let o = t[..floor(t, MAX_TEXT)].to_string();
        if let Some(p) = self.entries.iter().position(|e| e.text == o) {
            let mut e = self.entries.remove(p);
            e.uses = e.uses.saturating_add(1);
            self.entries.insert(0, e);
        } else {
            self.entries.insert(0, HistEntry { text: o, uses: 1 });
            self.entries.truncate(MAX_ENTRIES);
        }
        self.cursor = None;
    }
    #[must_use]
    pub fn suggest(&self, prefix: &str, limit: usize) -> Vec<String> {
        let mut h: Vec<&HistEntry> = self
            .entries
            .iter()
            .filter(|e| e.text.starts_with(prefix))
            .collect();
        h.sort_by(|a, b| b.uses.cmp(&a.uses));
        h.into_iter()
            .take(limit.min(MAX_SUGGEST))
            .map(|e| e.text.clone())
            .collect()
    }
    pub fn prev(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        let n = match self.cursor {
            None => 0,
            Some(i) if i + 1 < self.entries.len() => i + 1,
            _ => return None,
        };
        self.cursor = Some(n);
        Some(self.entries[n].text.as_str())
    }
    pub fn next(&mut self) -> Option<&str> {
        match self.cursor {
            None => None,
            Some(0) => {
                self.cursor = None;
                None
            }
            Some(i) => {
                self.cursor = Some(i - 1);
                Some(self.entries[i - 1].text.as_str())
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dup_bumps_uses_and_moves_front() {
        let mut h = PromptHistory::new();
        h.record("a");
        h.record("b");
        h.record("a");
        assert_eq!(
            (h.len(), h.entries()[0].text.as_str(), h.entries()[0].uses),
            (2, "a", 2)
        );
        assert_eq!(h.entries()[1].text, "b");
    }
    #[test]
    fn evict_caps_at_200_oldest_first() {
        let mut h = PromptHistory::new();
        for i in 0..MAX_ENTRIES + 10 {
            h.record(&format!("e{i:04}"));
        }
        assert_eq!(h.len(), MAX_ENTRIES);
        assert_eq!(h.entries()[0].text, format!("e{:04}", MAX_ENTRIES + 9));
        assert!(!h.entries().iter().any(|e| e.text == "e0000"));
    }
    #[test]
    fn suggest_orders_by_uses_desc() {
        let mut h = PromptHistory::new();
        for s in ["git push", "git pull", "git push", "git pull", "git pull"] {
            h.record(s);
        }
        assert_eq!(h.suggest("git", 8), ["git pull", "git push"]);
    }
    #[test]
    fn suggest_caps_at_8_and_limit() {
        let mut h = PromptHistory::new();
        for i in 0..10 {
            h.record(&format!("cmd-{i}"));
        }
        assert_eq!(h.suggest("cmd-", 100).len(), MAX_SUGGEST);
        assert_eq!(h.suggest("cmd-", 3).len(), 3);
        assert!(h.suggest("zzz", 8).is_empty());
    }
    #[test]
    fn cursor_bounds_stay_put() {
        let mut h = PromptHistory::new();
        assert_eq!(h.prev(), None);
        assert_eq!(h.next(), None);
        h.record("a");
        h.record("b");
        assert_eq!(h.prev(), Some("b"));
        assert_eq!(h.prev(), Some("a"));
        assert_eq!(h.prev(), None);
        assert_eq!(h.cursor(), Some(1));
        assert_eq!(h.next(), Some("b"));
        assert_eq!(h.next(), None);
        assert_eq!(h.next(), None);
        assert_eq!(h.cursor(), None);
    }
    #[test]
    fn record_truncates_to_4kib_and_ignores_blank() {
        let mut h = PromptHistory::new();
        h.record("   ");
        assert!(h.is_empty());
        h.record(&"x".repeat(MAX_TEXT + 100));
        assert_eq!(
            (h.entries()[0].text.len(), h.entries()[0].uses),
            (MAX_TEXT, 1)
        );
    }
}
