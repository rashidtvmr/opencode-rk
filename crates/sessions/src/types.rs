//! Core session types and constants for the sessions crate.
//!
//! Primary session types live in `opencode-rk-contracts` (SessionId, SessionState,
//! SessionSummary, SessionRecord alias). This module provides any additional
//! types specific to the sessions crate.

use chrono::{DateTime, Utc};

/// Maximum number of sessions a single account may hold.
pub const MAX_SESSIONS: u32 = 10_000;

/// Maximum number of messages retained per session before eviction.
pub const MAX_MESSAGES: u32 = 100_000;

/// Default page size for paginated session/message queries.
pub const PAGE_SIZE: u16 = 100;

/// Metadata for a session that was archived.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchivedSession {
    pub session_id: crate::SessionId,
    pub archived_at: DateTime<Utc>,
    pub old_state: crate::SessionState,
    pub reason: String,
}

/// Immutable identity and lifecycle metadata for a session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionMetadata {
    pub id: crate::SessionId,
    pub title: String,
    pub position: u32,
    pub parent_id: Option<crate::SessionId>,
    pub fork_depth: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
    pub state: crate::SessionState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_valid() {
        assert_eq!(MAX_SESSIONS, 10_000);
        assert_eq!(MAX_MESSAGES, 100_000);
        assert_eq!(PAGE_SIZE, 100);
    }

    #[test]
    fn archived_session_fields() {
        let now = Utc::now();
        let id = crate::SessionId::new();
        let arch = ArchivedSession {
            session_id: id,
            archived_at: now,
            old_state: crate::SessionState::Active,
            reason: "test".to_string(),
        };
        assert_eq!(arch.session_id, id);
        assert_eq!(arch.old_state, crate::SessionState::Active);
        assert_eq!(arch.reason, "test");
    }

    #[test]
    fn metadata_fields() {
        let now = Utc::now();
        let id = crate::SessionId::new();
        let meta = SessionMetadata {
            id,
            title: "Test".to_string(),
            position: 1,
            parent_id: None,
            fork_depth: 0,
            created_at: now,
            updated_at: now,
            archived_at: None,
            state: crate::SessionState::Active,
        };
        assert_eq!(meta.id, id);
        assert_eq!(meta.title, "Test");
        assert!(meta.archived_at.is_none());
    }
}
