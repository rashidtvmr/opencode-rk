//! Core session types and constants for the sessions crate.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Maximum number of sessions a single account may hold.
pub const MAX_SESSIONS: u32 = 10_000;

/// Maximum number of messages retained per session before eviction.
pub const MAX_MESSAGES: u32 = 100_000;

/// Default page size for paginated session/message queries.
pub const PAGE_SIZE: u16 = 100;

/// A 128-bit session identifier stored as a raw byte array.
pub type SessionId = [u8; 16];

/// A 128-bit message identifier stored as a raw byte array.
pub type MessageId = [u8; 16];

/// Human-readable session title (owned String).
pub type SessionTitle = String;

/// Lifecycle state a session can occupy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SessionState {
    Pending,
    Active,
    Paused,
    Archived,
    Deleted,
}

/// Record of a session that was moved to the archive.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArchivedSession {
    pub session_id: SessionId,
    pub archived_at: DateTime<Utc>,
    pub old_state: SessionState,
    pub reason: String,
}

/// Immutable identity and lifecycle metadata for a session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub id: SessionId,
    pub title: SessionTitle,
    pub position: u32,
    pub parent_id: Option<SessionId>,
    pub fork_depth: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
    pub state: SessionState,
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn session_state_serializes() {
        let json = serde_json::to_string(&SessionState::Active).unwrap();
        assert_eq!(json, r#""Active""#);
        let json = serde_json::to_string(&SessionState::Paused).unwrap();
        assert_eq!(json, r#""Paused""#);
        let json = serde_json::to_string(&SessionState::Pending).unwrap();
        assert_eq!(json, r#""Pending""#);
        let json = serde_json::to_string(&SessionState::Archived).unwrap();
        assert_eq!(json, r#""Archived""#);
        let json = serde_json::to_string(&SessionState::Deleted).unwrap();
        assert_eq!(json, r#""Deleted""#);
    }

    #[test]
    fn archived_session_fields() {
        let uuid = Uuid::new_u128_pair_le(Uuid::new_v4().as_u128(), Uuid::nil().as_u128());
        let sid = uuid.into();
        let arch = ArchivedSession {
            session_id: sid,
            archived_at: DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
                .unwrap()
                .with_timezone(Utc),
            old_state: SessionState::Active,
            reason: "user requested archive".to_string(),
        };
        assert_eq!(arch.session_id, sid);
        assert_eq!(arch.old_state, SessionState::Active);
        assert_eq!(arch.reason, "user requested archive");
    }

    #[test]
    fn metadata_accessors() {
        let uuid = Uuid::new_v4();
        let sid = uuid.into();
        let now = Utc::now();
        let meta = SessionMetadata {
            id: sid,
            title: "Test Session".to_string(),
            position: 42,
            parent_id: None,
            fork_depth: 0,
            created_at: now,
            updated_at: now,
            archived_at: None,
            state: SessionState::Active,
        };
        assert_eq!(meta.id, sid);
        assert_eq!(meta.title, "Test Session");
        assert_eq!(meta.position, 42);
        assert_eq!(meta.parent_id, None);
        assert_eq!(meta.fork_depth, 0);
        assert_eq!(meta.state, SessionState::Active);
        assert_eq!(meta.archived_at, None);
    }

    #[test]
    fn constants_valid() {
        assert_eq!(MAX_SESSIONS, 10_000);
        assert_eq!(MAX_MESSAGES, 100_000);
        assert_eq!(PAGE_SIZE, 100);
    }

    #[test]
    fn session_id_from_uuid() {
        let uuid = Uuid::new_v4();
        let sid: SessionId = uuid.into();
        let recovered: Uuid = sid.into();
        assert_eq!(recovered, uuid);
    }
}

impl From<Uuid> for SessionId {
    fn from(uuid: Uuid) -> Self {
        *uuid.as_bytes()
    }
}

impl From<SessionId> for Uuid {
    fn from(bytes: SessionId) -> Self {
        Uuid::from_bytes(bytes)
    }
}
