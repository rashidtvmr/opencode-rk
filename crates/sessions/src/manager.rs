//! Session manager for CRUD operations on sessions.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use opencode_rk_contracts::{MessageRecord, SessionId};
use thiserror::Error;

/// Error returned when a session is not found.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SessionManagerError {
    #[error("session not found: {0}")]
    NotFound(SessionId),
}

/// A record representing a single session with its messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionRecord {
    pub id: SessionId,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<MessageRecord>,
}

/// In-memory session manager providing CRUD operations.
pub struct SessionManager {
    sessions: HashMap<SessionId, SessionRecord>,
    next_id: u64,
}

impl SessionManager {
    /// Create a new empty session manager.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            next_id: 1,
        }
    }

    /// Create a new session with the given title and return its ID.
    pub fn create(&mut self, title: &str) -> SessionId {
        let id = SessionId::new();
        let now = Utc::now();
        let record = SessionRecord {
            id,
            title: title.to_string(),
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
        };
        self.sessions.insert(id, record);
        id
    }

    /// Get a reference to a session by ID, if it exists.
    pub fn get(&self, id: SessionId) -> Option<&SessionRecord> {
        self.sessions.get(&id)
    }

    /// Delete a session by ID. Returns true if the session was deleted,
    /// false if it did not exist.
    pub fn delete(&mut self, id: SessionId) -> bool {
        self.sessions.remove(&id).is_some()
    }

    /// List all sessions.
    pub fn list(&self) -> Vec<&SessionRecord> {
        self.sessions.values().collect()
    }

    /// Update the title of a session. Returns true if updated,
    /// false if the session was not found.
    pub fn update_title(&mut self, id: SessionId, title: &str) -> bool {
        if let Some(record) = self.sessions.get_mut(&id) {
            record.title = title.to_string();
            record.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Number of sessions currently managed.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Whether the manager has no sessions.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_get() {
        let mut manager = SessionManager::new();
        let title = "Test Session";
        let id = manager.create(title);
        let record = manager.get(id).expect("session should exist after creation");
        assert_eq!(record.id, id);
        assert_eq!(record.title, title);
        assert!(record.messages.is_empty());
    }

    #[test]
    fn delete_removes() {
        let mut manager = SessionManager::new();
        let id = manager.create("Session to delete");
        assert!(manager.get(id).is_some());
        assert!(manager.delete(id));
        assert!(manager.get(id).is_none());
    }

    #[test]
    fn list_returns_all() {
        let mut manager = SessionManager::new();
        let id1 = manager.create("First");
        let id2 = manager.create("Second");
        let list = manager.list();
        assert_eq!(list.len(), 2);
        let ids: Vec<SessionId> = list.iter().map(|r| r.id).collect();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[test]
    fn update_title_works() {
        let mut manager = SessionManager::new();
        let id = manager.create("Old title");
        assert!(manager.update_title(id, "New title"));
        let record = manager.get(id).expect("session should exist");
        assert_eq!(record.title, "New title");
    }

    #[test]
    fn delete_missing_returns_false() {
        let mut manager = SessionManager::new();
        let id = SessionId::new();
        assert!(!manager.delete(id));
    }
}
