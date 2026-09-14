//! Session statistics collection and tracking.
//!
//! Provides [`SessionStatsCollector`] for accumulating session metadata and
//! computing aggregate statistics across all sessions.

use std::collections::HashMap;

use opencode_rk_contracts::{SessionId, SessionState, SessionSummary, Timestamp};

/// Aggregate statistics for all sessions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionStats {
    /// Total number of sessions (active + archived).
    pub total_sessions: usize,
    /// Number of active sessions.
    pub active_sessions: usize,
    /// Number of archived sessions.
    pub archived_sessions: usize,
    /// Total message count across all sessions.
    pub total_messages: u64,
    /// Average messages per session (0.0 if no sessions).
    pub avg_messages_per_session: f64,
}

/// Collector for session statistics.
///
/// Accumulates session data and computes aggregate statistics on demand.
#[derive(Clone, Debug, Default)]
pub struct SessionStatsCollector {
    sessions: Vec<SessionSummary>,
    by_state: HashMap<String, usize>,
    total_messages: u64,
}

impl SessionStatsCollector {
    /// Create a new empty collector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            by_state: HashMap::new(),
            total_messages: 0,
        }
    }

    /// Collect stats from a session manager.
    ///
    /// Clears any previously collected data and populates fresh from the manager.
    pub fn collect(&mut self, manager: &crate::SessionManager) {
        self.sessions = manager.list_sessions(500).unwrap_or_default();
        self.by_state.clear();
        self.total_messages = 0;
        for session in &self.sessions {
            let state_key = format!("{:?}", session.state);
            *self.by_state.entry(state_key).or_insert(0) += 1;
        }
    }

    /// Update the collector with a single session.
    pub fn add(&mut self, session: SessionSummary) {
        let state_key = format!("{:?}", session.state);
        *self.by_state.entry(state_key).or_insert(0) += 1;
        self.sessions.push(session);
    }

    /// Take an immutable snapshot of current statistics.
    pub fn snapshot(&self) -> SessionStats {
        let total_sessions = self.sessions.len();
        let active_sessions = *self.by_state.get("Active").unwrap_or(&0);
        let archived_sessions = *self.by_state.get("Archived").unwrap_or(&0);
        let total_messages = self.total_messages;
        let avg_messages_per_session = if total_sessions > 0 {
            total_messages as f64 / total_sessions as f64
        } else {
            0.0
        };

        SessionStats {
            total_sessions,
            active_sessions,
            archived_sessions,
            total_messages,
            avg_messages_per_session,
        }
    }

    /// Reset all collected data.
    pub fn reset(&mut self) {
        self.sessions.clear();
        self.by_state.clear();
        self.total_messages = 0;
    }

    /// Get number of sessions in collector.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Check if collector is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Set the total message count for average calculation.
    pub fn set_total_messages(&mut self, count: u64) {
        self.total_messages = count;
    }
}

impl Default for SessionStatsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(id: SessionId, state: SessionState) -> SessionSummary {
        SessionSummary {
            id,
            title: format!("Session-{:?}", state),
            state,
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            archived_at: if state == SessionState::Archived {
                Some(Timestamp::now())
            } else {
                None
            },
        }
    }

    #[test]
    fn collect_counts() {
        let mut collector = SessionStatsCollector::new();
        let id1 = SessionId::new();
        let id2 = SessionId::new();
        let id3 = SessionId::new();

        collector.add(make_session(id1, SessionState::Active));
        collector.add(make_session(id2, SessionState::Active));
        collector.add(make_session(id3, SessionState::Archived));

        let stats = collector.snapshot();
        assert_eq!(stats.total_sessions, 3);
    }

    #[test]
    fn active_vs_archived() {
        let mut collector = SessionStatsCollector::new();
        let id1 = SessionId::new();
        let id2 = SessionId::new();
        let id3 = SessionId::new();
        let id4 = SessionId::new();

        collector.add(make_session(id1, SessionState::Active));
        collector.add(make_session(id2, SessionState::Active));
        collector.add(make_session(id3, SessionState::Archived));
        collector.add(make_session(id4, SessionState::Archived));

        let stats = collector.snapshot();
        assert_eq!(stats.active_sessions, 2);
        assert_eq!(stats.archived_sessions, 2);
    }

    #[test]
    fn avg_calculated() {
        let mut collector = SessionStatsCollector::new();
        let id1 = SessionId::new();
        let id2 = SessionId::new();

        collector.add(make_session(id1, SessionState::Active));
        collector.add(make_session(id2, SessionState::Active));
        collector.set_total_messages(30);

        let stats = collector.snapshot();
        assert!((stats.avg_messages_per_session - 15.0).abs() < 0.001);
    }

    #[test]
    fn snapshot_immutable() {
        let mut collector = SessionStatsCollector::new();
        let id1 = SessionId::new();

        collector.add(make_session(id1, SessionState::Active));
        collector.set_total_messages(10);

        let stats1 = collector.snapshot();

        // Modify collector
        let id2 = SessionId::new();
        collector.add(make_session(id2, SessionState::Active));

        // Snapshot should be independent of further collector changes
        assert_eq!(stats1.total_sessions, 1);
    }

    #[test]
    fn reset_clears() {
        let mut collector = SessionStatsCollector::new();
        let id1 = SessionId::new();
        let id2 = SessionId::new();

        collector.add(make_session(id1, SessionState::Active));
        collector.add(make_session(id2, SessionState::Archived));
        collector.set_total_messages(30);

        assert_eq!(collector.len(), 2);
        assert!(!collector.is_empty());

        collector.reset();

        assert_eq!(collector.len(), 0);
        assert!(collector.is_empty());
        let stats = collector.snapshot();
        assert_eq!(stats.total_sessions, 0);
    }
}
