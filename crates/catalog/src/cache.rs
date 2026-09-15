//! Plugin cache with LRU eviction.

use serde_json::Value;
use std::collections::HashMap;
use std::time::SystemTime;

/// Entry stored in the cache with metadata for LRU tracking.
#[derive(Clone, Debug)]
pub struct CacheEntry {
    /// Cache key
    pub key: String,
    /// Cached value (JSON)
    pub value: Value,
    /// Access hit count
    pub hits: u64,
    /// When the entry was created
    pub created_at: SystemTime,
    /// When the entry was last accessed
    pub last_accessed: SystemTime,
}

impl CacheEntry {
    fn new(key: String, value: Value) -> Self {
        let now = SystemTime::now();
        Self {
            key: key.clone(),
            value,
            hits: 0,
            created_at: now,
            last_accessed: now,
        }
    }
}

/// LRU-evicting cache for catalog values.
#[derive(Clone, Debug, Default)]
pub struct CatalogCache {
    /// Stored entries indexed by key
    entries: HashMap<String, CacheEntry>,
    /// Track LRU order (front = most recent, back = least recent)
    evict_order: Vec<String>,
    /// Maximum number of entries
    capacity: usize,
    /// Cache statistics
    hits: u64,
    misses: u64,
}

impl CatalogCache {
    /// Create a new cache with the specified capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            evict_order: Vec::new(),
            capacity,
            hits: 0,
            misses: 0,
        }
    }

    /// Get a value from the cache, returns None if not found.
    /// Updates LRU order and hit count on success.
    #[must_use]
    pub fn get(&mut self, key: &str) -> Option<Value> {
        if let Some(entry) = self.entries.get(key).cloned() {
            let value = entry.value.clone();

            // Update hit stats
            self.hits += 1;

            // Update the entry with new hit count and timestamp
            let updated_entry = CacheEntry {
                key: entry.key.clone(),
                value: entry.value.clone(),
                hits: entry.hits + 1,
                created_at: entry.created_at,
                last_accessed: SystemTime::now(),
            };
            self.entries.insert(key.to_string(), updated_entry);

            // Update LRU order
            self._update_lru(key);

            Some(value)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Set a value in the cache, evicting LRU entry if at capacity.
    pub fn set(&mut self, key: String, value: Value) {
        // If capacity exceeded, evict LRU before inserting
        if self.entries.len() >= self.capacity {
            self._evict_lru();
        }

        // Remove from evict_order if already present (will be re-added on update)
        self.evict_order.retain(|k| k != &key);

        // Insert at front of evict_order (most recently used)
        self.evict_order.insert(0, key.clone());

        let entry = CacheEntry::new(key, value);
        self.entries.insert(entry.key.clone(), entry);
    }

    /// Explicitly evict a key from the cache.
    pub fn evict(&mut self, key: &str) {
        self.entries.remove(key);
        self.evict_order.retain(|k| k != key);
    }

    /// Remove all entries from the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.evict_order.clear();
        self.hits = 0;
        self.misses = 0;
    }

    /// Get cache statistics.
    #[must_use]
    pub fn stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }

    /// Move key to front of LRU order (most recently used)
    fn _update_lru(&mut self, key: &str) {
        // Remove from current position
        self.evict_order.retain(|k| k != key);
        // Insert at front (most recently used)
        self.evict_order.insert(0, key.to_string());
    }

    /// Evict the least recently used entry (back of evict_order).
    fn _evict_lru(&mut self) {
        if let Some(lru_key) = self.evict_order.pop() {
            self.entries.remove(&lru_key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn get_miss_returns_none() {
        let mut cache = CatalogCache::new(10);
        let result = cache.get("nonexistent");
        assert!(result.is_none());
        let (hits, misses) = cache.stats();
        assert_eq!(hits, 0);
        assert_eq!(misses, 1);
    }

    #[test]
    fn set_and_get() {
        let mut cache = CatalogCache::new(10);
        let value = json!({"name": "test-model"});
        cache.set("openai/gpt-4".to_string(), value.clone());

        let result = cache.get("openai/gpt-4");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), value);

        let (hits, misses) = cache.stats();
        assert_eq!(hits, 1);
        assert_eq!(misses, 0);
    }

    #[test]
    fn lru_eviction() {
        let mut cache = CatalogCache::new(2);

        // Fill cache to capacity
        cache.set("key1".to_string(), json!({"id": 1}));
        cache.set("key2".to_string(), json!({"id": 2}));

        // Access key1 to make it most recently used
        let _ = cache.get("key1");

        // Insert new key, should evict key2 (LRU)
        cache.set("key3".to_string(), json!({"id": 3}));

        // key2 should be evicted
        assert!(cache.get("key2").is_none());

        // key1 and key3 should exist
        assert!(cache.get("key1").is_some());
        assert!(cache.get("key3").is_some());
    }

    #[test]
    fn clear_removes_all() {
        let mut cache = CatalogCache::new(10);

        // Set and access to generate stats
        cache.set("key1".to_string(), json!({"id": 1}));
        let _ = cache.get("key1");

        cache.clear();

        // Stats should be reset
        let (hits, misses) = cache.stats();
        assert_eq!(hits, 0);
        assert_eq!(misses, 0);

        // Verify entries are cleared
        assert!(cache.get("key1").is_none());
    }

    #[test]
    fn hits_counted() {
        let mut cache = CatalogCache::new(10);

        cache.set("key1".to_string(), json!({"test": true}));

        // First get - hit
        let _ = cache.get("key1");
        let (hits1, misses1) = cache.stats();
        assert_eq!(hits1, 1);
        assert_eq!(misses1, 0);

        // Second get - another hit
        let _ = cache.get("key1");
        let (hits2, misses2) = cache.stats();
        assert_eq!(hits2, 2);
        assert_eq!(misses2, 0);

        // Miss
        let _ = cache.get("nonexistent");
        let (hits3, misses3) = cache.stats();
        assert_eq!(hits3, 2);
        assert_eq!(misses3, 1);
    }
}
