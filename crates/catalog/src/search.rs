//! Catalog search index module.
#![forbid(unsafe_code)]

use super::registry::CatalogEntry;

/// In-memory search index for catalog entries.
#[derive(Debug, Default, Clone)]
pub struct SearchIndex {
    entries: Vec<CatalogEntry>,
    indexed_count: u64,
}

impl SearchIndex {
    /// Creates a new empty search index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an entry to the index, incrementing the count.
    pub fn index(&mut self, entry: CatalogEntry) {
        self.entries.push(entry);
        self.indexed_count += 1;
    }

    /// Searches for entries matching the query in name or tags (case-insensitive).
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&CatalogEntry> {
        let needle = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.name.to_lowercase().contains(&needle)
                    || e.tags.iter().any(|t| t.to_lowercase().contains(&needle))
            })
            .collect()
    }

    /// Removes all entries and resets the count.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.indexed_count = 0;
    }

    /// Returns statistics: (number of entries, total indexed count).
    #[must_use]
    pub fn stats(&self) -> (usize, u64) {
        (self.entries.len(), self.indexed_count)
    }
}

#[cfg(test)]
mod test_search_index {
    use super::*;

    #[test]
    fn index_and_search() {
        let mut index = SearchIndex::new();
        index.index(CatalogEntry::new(
            "test-1",
            "Python Helper",
            "1.0.0",
            "/path/to/plugin",
            "ai",
            vec!["python".to_string()],
        ));
        let results = index.search("python");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "test-1");
    }

    #[test]
    fn search_finds_by_tag() {
        let mut index = SearchIndex::new();
        index.index(CatalogEntry::new(
            "test-1",
            "Helper Tool",
            "1.0.0",
            "/path",
            "tools",
            vec!["helper".to_string(), "utility".to_string()],
        ));
        let results = index.search("utility");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "test-1");
    }

    #[test]
    fn search_case_insensitive() {
        let mut index = SearchIndex::new();
        index.index(CatalogEntry::new(
            "test-1",
            "Rust Assistant",
            "1.0.0",
            "/path",
            "ai",
            vec!["rust".to_string()],
        ));
        let results = index.search("RUST");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "test-1");
    }

    #[test]
    fn clear_removes_all() {
        let mut index = SearchIndex::new();
        index.index(CatalogEntry::new("id1", "Alpha", "1.0", "/a", "x", vec![]));
        index.index(CatalogEntry::new("id2", "Beta", "1.0", "/b", "y", vec![]));
        index.clear();
        assert!(index.entries.is_empty());
        assert_eq!(index.indexed_count, 0);
        let results = index.search("alpha");
        assert!(results.is_empty());
    }

    #[test]
    fn stats_track_count() {
        let mut index = SearchIndex::new();
        let (entries, count) = index.stats();
        assert_eq!(entries, 0);
        assert_eq!(count, 0);

        index.index(CatalogEntry::new("id1", "One", "1.0", "/a", "x", vec![]));
        let (entries, count) = index.stats();
        assert_eq!(entries, 1);
        assert_eq!(count, 1);

        index.index(CatalogEntry::new("id2", "Two", "1.0", "/b", "y", vec![]));
        let (entries, count) = index.stats();
        assert_eq!(entries, 2);
        assert_eq!(count, 2);
    }
}
