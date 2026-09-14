//! Catalog registry module.
#![forbid(unsafe_code)]

use std::collections::HashMap;

/// A catalog entry representing a plugin or skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub path: String,
    pub category: String,
    pub tags: Vec<String>,
}

impl CatalogEntry {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        path: impl Into<String>,
        category: impl Into<String>,
        tags: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            path: path.into(),
            category: category.into(),
            tags,
        }
    }
}

/// Registry for catalog entries.
#[derive(Debug, Default)]
pub struct CatalogRegistry {
    entries: HashMap<String, CatalogEntry>,
}

impl CatalogRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, entry: CatalogEntry) {
        self.entries.insert(entry.id.clone(), entry);
    }

    pub fn get(&self, id: &str) -> Option<&CatalogEntry> {
        self.entries.get(id)
    }

    pub fn list_by_category(&self, cat: &str) -> Vec<&CatalogEntry> {
        self.entries
            .values()
            .filter(|e| e.category == cat)
            .collect()
    }

    pub fn search(&self, query: &str) -> Vec<&CatalogEntry> {
        self.entries
            .values()
            .filter(|e| {
                e.name.to_lowercase().contains(&query.to_lowercase())
                    || e.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query.to_lowercase()))
            })
            .collect()
    }

    pub fn unregister(&mut self, id: &str) -> bool {
        self.entries.remove(id).is_some()
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_get() {
        let mut registry = CatalogRegistry::new();
        let entry = CatalogEntry::new(
            "test-1",
            "Test Plugin",
            "1.0.0",
            "/path/to/plugin",
            "skills",
            vec!["test".to_string()],
        );
        registry.register(entry.clone());
        let fetched = registry.get("test-1");
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "Test Plugin");
    }

    #[test]
    fn list_by_category() {
        let mut registry = CatalogRegistry::new();
        registry.register(CatalogEntry::new(
            "skill-1",
            "Skill One",
            "1.0.0",
            "/path/skill1",
            "skills",
            vec![],
        ));
        registry.register(CatalogEntry::new(
            "plugin-1",
            "Plugin One",
            "1.0.0",
            "/path/plugin1",
            "plugins",
            vec![],
        ));
        let skills = registry.list_by_category("skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "skill-1");
    }

    #[test]
    fn search_finds_matching() {
        let mut registry = CatalogRegistry::new();
        registry.register(CatalogEntry::new(
            "test-1",
            "Python Assistant",
            "1.0.0",
            "/path",
            "ai",
            vec!["python".to_string(), "llm".to_string()],
        ));
        registry.register(CatalogEntry::new(
            "test-2",
            "Rust Helper",
            "1.0.0",
            "/path",
            "ai",
            vec!["rust".to_string()],
        ));
        let results = registry.search("python");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "test-1");
    }

    #[test]
    fn unregister_removes() {
        let mut registry = CatalogRegistry::new();
        registry.register(CatalogEntry::new(
            "to-remove",
            "Remove Me",
            "1.0.0",
            "/path",
            "test",
            vec![],
        ));
        assert!(registry.get("to-remove").is_some());
        let removed = registry.unregister("to-remove");
        assert!(removed);
        assert!(registry.get("to-remove").is_none());
    }

    #[test]
    fn count_correct() {
        let mut registry = CatalogRegistry::new();
        assert_eq!(registry.count(), 0);
        registry.register(CatalogEntry::new("a", "a", "1", "/a", "x", vec![]));
        registry.register(CatalogEntry::new("b", "b", "1", "/b", "y", vec![]));
        assert_eq!(registry.count(), 2);
        registry.register(CatalogEntry::new("c", "c", "1", "/c", "z", vec![]));
        assert_eq!(registry.count(), 3);
    }
}
