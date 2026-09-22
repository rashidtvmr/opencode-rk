//! Persistent in-memory session store with dirty tracking and async flush support.
//!
//! This module provides `PersistentSessionStore` which maintains an in-memory cache
//! of sessions with change tracking for efficient batch persistence operations.
//! The store supports deferred writes via `flush` and garbage collection via `compact`.

use std::collections::HashSet;

use opencode_rk_contracts::{SessionId, SessionSummary, Timestamp};
use thiserror::Error;

/// Error types for persistent session store operations.
#[derive(Debug, Error)]
pub enum PersistentSessionStoreError {
    #[error("flush failed: {0}")]
    FlushError(String),
    #[error("load failed: {0}")]
    LoadError(String),
}

/// In-memory session store with dirty tracking.
///
/// Stores session summaries in memory with change tracking for efficient
/// batch persistence operations.
#[derive(Debug, Clone)]
pub struct PersistentSessionStore {
    sessions: Vec<SessionSummary>,
    dirty: HashSet<SessionId>,
    flush_interval_ms: u64,
}

impl Default for PersistentSessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl PersistentSessionStore {
    /// Creates a new empty session store with default flush interval.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            dirty: HashSet::new(),
            flush_interval_ms: 5000,
        }
    }

    /// Creates a new session store with a custom flush interval.
    #[must_use]
    pub fn with_flush_interval(ms: u64) -> Self {
        Self {
            sessions: Vec::new(),
            dirty: HashSet::new(),
            flush_interval_ms: ms,
        }
    }

    /// Saves a session to the store, marking it as dirty.
    ///
    /// If a session with the same ID already exists, it will be updated.
    pub fn save(&mut self, session: SessionSummary) {
        let id = session.id;
        if let Some(pos) = self.sessions.iter().position(|s| s.id == id) {
            self.sessions[pos] = session;
        } else {
            self.sessions.push(session);
        }
        self.dirty.insert(id);
    }

    /// Loads a session by ID from the store.
    pub fn load(&self, id: SessionId) -> Option<SessionSummary> {
        self.sessions.iter().find(|s| s.id == id).cloned()
    }

    /// Flushes dirty sessions to persistent storage.
    ///
    /// Currently clears the dirty set. Future implementations may
    /// persist changes to an underlying storage backend.
    pub fn flush(&mut self) -> Result<(), PersistentSessionStoreError> {
        self.dirty.clear();
        Ok(())
    }

    /// Compacts the store by removing orphaned (still-dirty) sessions.
    ///
    /// An orphaned session is one saved but never flushed: its id remains in
    /// the dirty set. Flushed sessions (ids absent from the dirty set) are
    /// retained. Removed ids are also dropped from the dirty set.
    pub fn compact(&mut self) -> Result<(), PersistentSessionStoreError> {
        let dirty = &self.dirty;
        self.sessions.retain(|s| !dirty.contains(&s.id));
        let live: std::collections::HashSet<SessionId> =
            self.sessions.iter().map(|s| s.id).collect();
        self.dirty.retain(|id| live.contains(id));
        Ok(())
    }

    /// Returns statistics about the store.
    ///
    /// Returns a tuple of (total_sessions, dirty_count).
    #[must_use]
    pub fn stats(&self) -> (usize, usize) {
        (self.sessions.len(), self.dirty.len())
    }

    /// Returns the configured flush interval in milliseconds.
    ///
    /// The store is in-memory only: the interval is advisory metadata for a
    /// future caller-owned flush scheduler. No background flush is spawned.
    #[must_use]
    pub fn flush_interval_ms(&self) -> u64 {
        self.flush_interval_ms
    }

    /// Returns true when the given session id has unflushed changes.
    #[must_use]
    pub fn is_dirty(&self, id: SessionId) -> bool {
        self.dirty.contains(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencode_rk_contracts::SessionState;

    fn make_session(title: &str) -> SessionSummary {
        SessionSummary {
            id: SessionId::new(),
            title: title.to_owned(),
            state: SessionState::Active,
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            archived_at: None,
        }
    }

    #[test]
    fn save_and_load() {
        let mut store = PersistentSessionStore::new();
        let session = make_session("test-session");
        store.save(session);
        let loaded = store.load(session.id);
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.title, session.title);
    }

    #[test]
    fn flush_clears_dirty() {
        let mut store = PersistentSessionStore::new();
        let session = make_session("test-session");
        store.save(session);
        assert_eq!(store.dirty.len(), 1);
        store.flush().unwrap();
        assert_eq!(store.dirty.len(), 0);
    }

    #[test]
    fn compact_removes_orphans() {
        let mut store = PersistentSessionStore::new();
        let active = make_session("active-session");
        let mut archived = make_session("archived-session");
        archived.state = SessionState::Archived;

        store.save(active.clone());
        store.save(archived.clone());
        assert_eq!(store.sessions.len(), 2);

        store.compact().unwrap();
        assert_eq!(store.sessions.len(), 1);
        assert!(store.sessions[0].id == active.id);
    }

    #[test]
    fn load_missing() {
        let store = PersistentSessionStore::new();
        let missing = SessionId::new();
        let loaded = store.load(missing);
        assert!(loaded.is_none());
    }

    #[test]
    fn stats_track() {
        let mut store = PersistentSessionStore::new();
        let session1 = make_session("session-1");
        let session2 = make_session("session-2");
        store.save(session1);
        store.save(session2);

        let (total, dirty) = store.stats();
        assert_eq!(total, 2);
        assert_eq!(dirty, 2);

        store.flush().unwrap();
        let (total, dirty) = store.stats();
        assert_eq!(total, 2);
        assert_eq!(dirty, 0);
    }
}
