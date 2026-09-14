//! Content-addressed transcript dedupe.
//!
//! Stores repeated prompts and tool schemas by canonical blake3 hash. Turns
//! reference content ids; unreferenced content is reclaimable via `gc_unreferenced`.
//!
//! Tables:
//!   _content_store(hash TEXT PK, data BLOB)
//!   _content_refs(hash TEXT, turn_id TEXT, UNIQUE(hash, turn_id))
#![forbid(unsafe_code)]

use rusqlite::{params, Connection};
use std::sync::Mutex;

use crate::StorageError;

/// blake3 hex hash of the canonical content bytes.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ContentId(String);

impl ContentId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ContentId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl AsRef<str> for ContentId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Content-addressed store wrapping a sqlite connection.
pub struct ContentStore {
    connection: Mutex<Connection>,
}

impl ContentStore {
    /// Open in-memory store for testing; creates tables.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let connection = Connection::open_in_memory()?;
        Self::configure(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Open a file-backed store at the given path; creates tables.
    pub fn open(path: &str) -> Result<Self, StorageError> {
        let connection = Connection::open(path)?;
        Self::configure(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn configure(connection: &Connection) -> Result<(), StorageError> {
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS _content_store (
                hash TEXT PRIMARY KEY,
                data BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS _content_refs (
                hash TEXT NOT NULL,
                turn_id TEXT NOT NULL,
                UNIQUE(hash, turn_id)
            );",
        )?;
        Ok(())
    }

    /// Store content bytes; returns the content id (blake3 hash).
    /// Inserts are idempotent: repeated calls with the same bytes return
    /// the same id and do not duplicate storage.
    pub fn store(&self, content: &[u8]) -> Result<ContentId, StorageError> {
        let hash = blake3::hash(content).to_hex().to_string();
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        connection.execute(
            "INSERT OR IGNORE INTO _content_store (hash, data) VALUES (?1, ?2)",
            params![hash, content],
        )?;
        Ok(ContentId(hash))
    }

    /// Fetch stored content bytes by id, if present.
    pub fn get(&self, id: &ContentId) -> Result<Option<Vec<u8>>, StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let result: Option<Vec<u8>> = connection
            .query_row(
                "SELECT data FROM _content_store WHERE hash = ?1",
                params![id.as_str()],
                |row| row.get(0),
            )
            .ok();
        Ok(result)
    }

    /// Whether content exists for the given id.
    pub fn exists(&self, id: &ContentId) -> Result<bool, StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(1) FROM _content_store WHERE hash = ?1",
                params![id.as_str()],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(count > 0)
    }

    /// Number of turns referencing this content id.
    pub fn reference_count(&self, id: &ContentId) -> Result<usize, StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM _content_refs WHERE hash = ?1",
                params![id.as_str()],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(count as usize)
    }

    /// Record a reference from a turn id to a content id.
    /// Idempotent: duplicate (hash, turn_id) pairs are ignored.
    pub fn add_reference(&self, content_id: &ContentId, turn_id: &str) -> Result<(), StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        connection.execute(
            "INSERT OR IGNORE INTO _content_refs (hash, turn_id) VALUES (?1, ?2)",
            params![content_id.as_str(), turn_id],
        )?;
        Ok(())
    }

    /// Delete content rows with zero references. Returns count deleted.
    /// Uses a deferred FK-friendly delete: a content row is collectible
    /// only when no _content_refs row points at its hash.
    pub fn gc_unreferenced(&self) -> Result<usize, StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let deleted = connection.execute(
            "DELETE FROM _content_store WHERE hash IN (
                SELECT s.hash FROM _content_store s
                LEFT JOIN _content_refs r ON r.hash = s.hash
                WHERE r.hash IS NULL
            )",
            [],
        )?;
        Ok(deleted as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id_of(content: &[u8]) -> String {
        blake3::hash(content).to_hex().to_string()
    }

    #[test]
    fn store_and_get() {
        let store = ContentStore::open_in_memory().unwrap();
        let content = b"hello transcript content";
        let id = store.store(content).unwrap();
        assert_eq!(id.as_str(), id_of(content));
        let fetched = store.get(&id).unwrap().unwrap();
        assert_eq!(fetched, content);
    }

    #[test]
    fn dedup_same_content() {
        let store = ContentStore::open_in_memory().unwrap();
        let content = b"repeat me repeat me";
        let first = store.store(content).unwrap();
        let second = store.store(content).unwrap();
        assert_eq!(first, second);
        // Only one row in _content_store despite two stores.
        let count: i64 = {
            let conn = store.connection.lock().unwrap();
            conn.query_row(
                "SELECT COUNT(*) FROM _content_store WHERE hash = ?1",
                params![first.as_str()],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert_eq!(count, 1);
    }

    #[test]
    fn reference_counting() {
        let store = ContentStore::open_in_memory().unwrap();
        let content = b"tool schema payload";
        let id = store.store(content).unwrap();
        assert_eq!(store.reference_count(&id).unwrap(), 0);
        store.add_reference(&id, "turn-1").unwrap();
        assert_eq!(store.reference_count(&id).unwrap(), 1);
        // Duplicate (hash, turn_id) ignored via UNIQUE constraint + INSERT OR IGNORE.
        store.add_reference(&id, "turn-1").unwrap();
        assert_eq!(store.reference_count(&id).unwrap(), 1);
        store.add_reference(&id, "turn-2").unwrap();
        assert_eq!(store.reference_count(&id).unwrap(), 2);
    }

    #[test]
    fn gc_removes_orphans() {
        let store = ContentStore::open_in_memory().unwrap();
        let live = store.store(b"referenced content").unwrap();
        let orphan = store.store(b"orphan content").unwrap();
        store.add_reference(&live, "turn-1").unwrap();
        let removed = store.gc_unreferenced().unwrap();
        assert_eq!(removed, 1);
        assert!(store.exists(&live).unwrap());
        assert!(!store.exists(&orphan).unwrap());
    }

    #[test]
    fn hash_deterministic() {
        let store = ContentStore::open_in_memory().unwrap();
        let content = b"deterministic bytes";
        let id1 = store.store(content).unwrap();
        let id2 = store.store(content).unwrap();
        let id3 = store.store(b"deterministic bytes").unwrap();
        assert_eq!(id1, id2);
        assert_eq!(id2, id3);
        // Hash matches the standalone blake3 computation.
        assert_eq!(id1.as_str(), id_of(content));
    }
}
