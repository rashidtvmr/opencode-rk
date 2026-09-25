#![forbid(unsafe_code)]
//! Substring search over string lists (std-only).
//!
//! Companion to `crate::sync_search` (exact binary search): this keeps a
//! capped query plus its last case-insensitive substring hits.

/// Max retained query chars.
pub const MAX_QUERY: usize = 128;
/// Max retained hits.
pub const MAX_HITS: usize = 64;

/// Capped query with its last hits.
pub struct SearchFull {
    query: String,
    hits: Vec<String>,
}

impl SearchFull {
    /// Empty query, no hits.
    pub fn new() -> Self {
        Self {
            query: String::new(),
            hits: Vec::new(),
        }
    }

    /// Replace query, truncating to [`MAX_QUERY`] chars.
    pub fn set_query(&mut self, query: &str) {
        self.query = query.chars().take(MAX_QUERY).collect();
    }

    /// Substring scan of `hay` (case-insensitive), capped at [`MAX_HITS`].
    /// Empty query clears hits.
    pub fn search(&mut self, hay: &[String]) {
        self.hits.clear();
        if self.query.is_empty() {
            return;
        }
        // ponytail: per-item to_lowercase; cache lowercased hay when profiling needs it.
        let needle = self.query.to_lowercase();
        for item in hay {
            if item.to_lowercase().contains(&needle) {
                self.hits.push(item.clone());
                if self.hits.len() >= MAX_HITS {
                    break;
                }
            }
        }
    }

    /// Last hits.
    pub fn hits(&self) -> Vec<String> {
        self.hits.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_query_truncates_to_cap() {
        let mut s = SearchFull::new();
        s.set_query(&"a".repeat(MAX_QUERY + 10));
        assert_eq!(s.query.chars().count(), MAX_QUERY);
    }

    #[test]
    fn search_is_case_insensitive_substring() {
        let mut s = SearchFull::new();
        s.set_query("ELL");
        s.search(&["hello".into(), "world".into(), "SHELL".into()]);
        assert_eq!(s.hits(), vec!["hello".to_string(), "SHELL".to_string()]);
    }

    #[test]
    fn empty_query_clears_hits() {
        let mut s = SearchFull::new();
        s.set_query("a");
        s.search(&["a".into()]);
        assert_eq!(s.hits().len(), 1);
        s.set_query("");
        s.search(&["a".into()]);
        assert!(s.hits().is_empty());
    }

    #[test]
    fn hits_capped_at_64() {
        let mut s = SearchFull::new();
        s.set_query("x");
        let hay = vec!["x".to_string(); MAX_HITS + 10];
        s.search(&hay);
        assert_eq!(s.hits().len(), MAX_HITS);
    }

    #[test]
    fn no_match_is_empty() {
        let mut s = SearchFull::new();
        s.set_query("zzz");
        s.search(&["abc".into()]);
        assert!(s.hits().is_empty());
    }

    #[test]
    fn requery_rescans() {
        let mut s = SearchFull::new();
        let hay = vec!["alpha".to_string(), "beta".to_string()];
        s.set_query("alpha");
        s.search(&hay);
        assert_eq!(s.hits(), vec!["alpha".to_string()]);
        s.set_query("beta");
        s.search(&hay);
        assert_eq!(s.hits(), vec!["beta".to_string()]);
    }
}
