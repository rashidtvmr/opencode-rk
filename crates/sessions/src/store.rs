//! Persistent session store with SQLite-backed CRUD operations.
//!
//! SessionId is stored as BLOB. SessionRecord is an alias for SessionSummary.
use chrono::{DateTime, Utc};
use opencode_rk_contracts::{SessionId, SessionState, Timestamp};
use rusqlite::{params, Connection, OptionalExtension};
use thiserror::Error;

// Re-use the SessionRecord alias from the crate root (aliases SessionSummary).
use crate::SessionRecord;

const ACTIVE: i64 = 0;
const ARCHIVED: i64 = 1;

/// Error type for session store operations.
#[derive(Debug, Error)]
pub enum SessionStoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("session not found: {0}")]
    NotFound(SessionId),
    #[error("mutex poisoned")]
    Poisoned,
}

/// Persistent session store wrapping an SQLite connection.
pub struct PersistentSessionStore {
    conn: Connection,
}

impl PersistentSessionStore {
    /// Create a new store from an existing SQLite connection.
    #[must_use]
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }

    /// Create a new session record in the database.
    pub fn create(&self, session: &SessionRecord) -> Result<(), SessionStoreError> {
        let now = Utc::now().timestamp_micros();
        let state = match session.state {
            SessionState::Active => ACTIVE,
            SessionState::Archived => ARCHIVED,
        };
        let archived_at = session.archived_at.map(|t| t.as_datetime().timestamp_micros());

        self.conn.execute(
            "INSERT INTO sessions (id, title, state, created_at_us, updated_at_us, archived_at_us) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session.id.as_uuid().as_bytes().as_slice(),
                &session.title,
                state,
                now,
                now,
                archived_at,
            ],
        )?;
        Ok(())
    }

    /// Fetch a session by its ID. Returns None if not found.
    pub fn fetch(&self, id: SessionId) -> Result<Option<SessionRecord>, SessionStoreError> {
        Ok(self.conn.query_row(
            "SELECT title, state, created_at_us, updated_at_us, archived_at_us \
             FROM sessions WHERE id=?1",
            params![id.as_uuid().as_bytes().as_slice()],
            |row: &rusqlite::Row<'_>| decode_session(id, row),
        ).optional()?)
    }

    /// Update an existing session record. Returns error if session not found.
    pub fn update(&self, session: &SessionRecord) -> Result<(), SessionStoreError> {
        let now = Utc::now().timestamp_micros();
        let state = match session.state {
            SessionState::Active => ACTIVE,
            SessionState::Archived => ARCHIVED,
        };
        let archived_at = session.archived_at.map(|t| t.as_datetime().timestamp_micros());

        let changed = self.conn.execute(
            "UPDATE sessions SET title=?1, state=?2, updated_at_us=?3, archived_at_us=?4 \
             WHERE id=?5",
            params![
                &session.title,
                state,
                now,
                archived_at,
                session.id.as_uuid().as_bytes().as_slice(),
            ],
        )?;

        if changed == 0 {
            return Err(SessionStoreError::NotFound(session.id));
        }
        Ok(())
    }

    /// Delete a session by ID. Returns true if deleted, false if not found.
    pub fn delete(&self, id: SessionId) -> Result<bool, SessionStoreError> {
        let changed = self.conn.execute(
            "DELETE FROM sessions WHERE id=?1",
            params![id.as_uuid().as_bytes().as_slice()],
        )?;
        Ok(changed > 0)
    }

    /// Return all sessions from the database.
    pub fn all_sessions(&self) -> Result<Vec<SessionRecord>, SessionStoreError> {
        let mut rows = self.conn.prepare(
            "SELECT id, title, state, created_at_us, updated_at_us, archived_at_us FROM sessions"
        )?;
        let records = rows
            .query_map([], |row: &rusqlite::Row<'_>| decode_session_row(row))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(records)
    }
}

/// Decode a session record from a database row.
fn decode_session(id: SessionId, row: &rusqlite::Row<'_>) -> Result<SessionRecord, rusqlite::Error> {
    let title: String = row.get(0)?;
    let state_val: i64 = row.get(1)?;
    let created_at_us: i64 = row.get(2)?;
    let updated_at_us: i64 = row.get(3)?;
    let archived_at_us: Option<i64> = row.get(4)?;

    let state = decode_state(state_val)?;
    let created_at = decode_ts(created_at_us);
    let updated_at = decode_ts(updated_at_us);
    let archived_at = archived_at_us.map(decode_ts);

    Ok(SessionRecord {
        id,
        title,
        state,
        created_at,
        updated_at,
        archived_at,
    })
}

/// Decode a session record where ID comes from the database.
fn decode_session_row(row: &rusqlite::Row<'_>) -> Result<SessionRecord, rusqlite::Error> {
    let idb: Vec<u8> = row.get(0)?;
    let id = SessionId::from_uuid(uuid::Uuid::from_slice(&idb).map_err(|_| rusqlite::Error::InvalidQuery)?);
    let title: String = row.get(1)?;
    let state_val: i64 = row.get(2)?;
    let created_at_us: i64 = row.get(3)?;
    let updated_at_us: i64 = row.get(4)?;
    let archived_at_us: Option<i64> = row.get(5)?;

    let state = decode_state(state_val)?;
    let created_at = decode_ts(created_at_us);
    let updated_at = decode_ts(updated_at_us);
    let archived_at = archived_at_us.map(decode_ts);

    Ok(SessionRecord {
        id,
        title,
        state,
        created_at,
        updated_at,
        archived_at,
    })
}

fn decode_state(value: i64) -> Result<SessionState, rusqlite::Error> {
    match value {
        ACTIVE => Ok(SessionState::Active),
        ARCHIVED => Ok(SessionState::Archived),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn decode_ts(micros: i64) -> Timestamp {
    Timestamp::from_datetime(DateTime::from_timestamp_micros(micros).expect("valid timestamp"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::create_session_table;
    use opencode_rk_contracts::SessionId;
    use tempfile::tempdir;

    fn setup_store() -> PersistentSessionStore {
        let conn = Connection::open_in_memory().unwrap();
        create_session_table(&conn).unwrap();
        PersistentSessionStore::new(conn)
    }

    fn make_session(id: SessionId, title: &str) -> SessionRecord {
        let now = Timestamp::now();
        SessionRecord {
            id,
            title: title.to_string(),
            state: SessionState::Active,
            created_at: now,
            updated_at: now,
            archived_at: None,
        }
    }

    #[test]
    fn create_and_fetch() {
        let store = setup_store();
        let id = SessionId::new();
        let session = make_session(id, "test-session");

        store.create(&session).unwrap();
        let fetched = store.fetch(id).unwrap().unwrap();

        assert_eq!(fetched.id, id);
        assert_eq!(fetched.title, "test-session");
        assert_eq!(fetched.state, SessionState::Active);
    }

    #[test]
    fn update_persists() {
        let store = setup_store();
        let id = SessionId::new();
        let session = make_session(id, "original");

        store.create(&session).unwrap();

        let mut updated = session.clone();
        updated.title = "updated-title".to_string();
        store.update(&updated).unwrap();

        let fetched = store.fetch(id).unwrap().unwrap();
        assert_eq!(fetched.title, "updated-title");
    }

    #[test]
    fn delete_removes() {
        let store = setup_store();
        let id = SessionId::new();
        let session = make_session(id, "to-delete");

        store.create(&session).unwrap();
        assert!(store.fetch(id).unwrap().is_some());

        let deleted = store.delete(id).unwrap();
        assert!(deleted);

        assert!(store.fetch(id).unwrap().is_none());
    }

    #[test]
    fn all_sessions_returns_all() {
        let store = setup_store();
        let id1 = SessionId::new();
        let id2 = SessionId::new();

        store.create(&make_session(id1, "session-1")).unwrap();
        store.create(&make_session(id2, "session-2")).unwrap();

        let all = store.all_sessions().unwrap();
        assert_eq!(all.len(), 2);

        let ids: Vec<SessionId> = all.iter().map(|s| s.id).collect();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[test]
    fn fetch_missing_returns_none() {
        let store = setup_store();
        let missing_id = SessionId::new();

        let result = store.fetch(missing_id).unwrap();
        assert!(result.is_none());
    }
}