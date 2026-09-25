#![forbid(unsafe_code)]
//! Prompt frecency (mirrors frecency.tsx scoring: freq decayed by recency).
pub const MAX_FRECENCY: usize = 64;
#[derive(Debug, Clone, Default)]
pub struct Frecency {
    pub hits: Vec<(String, u32)>,
}
impl Frecency {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn record(&mut self, key: &str) {
        let k = key.trim();
        if k.is_empty() {
            return;
        }
        if let Some(i) = self.hits.iter().position(|(s, _)| s == k) {
            let (_, c) = self.hits.remove(i);
            self.hits.insert(0, (k.to_string(), c.saturating_add(1)));
        } else {
            self.hits.insert(0, (k.to_string(), 1));
        }
        self.hits.truncate(MAX_FRECENCY);
    }
    #[must_use]
    pub fn top(&self, n: usize) -> Vec<String> {
        let mut v = self.hits.clone();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v.into_iter().take(n).map(|(s, _)| s).collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_top() {
        assert!(Frecency::new().top(3).is_empty());
    }
    #[test]
    fn count_and_rank() {
        let mut f = Frecency::new();
        f.record("a");
        f.record("b");
        f.record("a");
        assert_eq!(f.top(2), vec!["a".to_string(), "b".to_string()]);
    }
    #[test]
    fn blank_ignored() {
        let mut f = Frecency::new();
        f.record("   ");
        assert!(f.hits.is_empty());
    }
    #[test]
    fn cap_64_evicts_oldest() {
        let mut f = Frecency::new();
        for i in 0..MAX_FRECENCY + 5 {
            f.record(&format!("k{i:03}"));
        }
        assert_eq!(f.hits.len(), MAX_FRECENCY);
        assert!(!f.hits.iter().any(|(s, _)| s == "k000"));
    }
    #[test]
    fn top_truncates_n() {
        let mut f = Frecency::new();
        f.record("a");
        f.record("b");
        assert_eq!(f.top(1).len(), 1);
    }
}
