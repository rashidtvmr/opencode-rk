//! In-memory session index providing O(1) lookup by id, title, and state.
//!
//! This module provides [`SessionIndex`], a secondary index over session
//! summaries that accelerates queries by id, title, and state without
//! scanning the full session list.

use std::collections::HashMap;

use opencode_rk_contracts::{SessionId, SessionState, SessionSummary};

/// In-memory indices over a set of [`SessionSummary`] records.
#[derive(Default)]
pub struct SessionIndex {
    /// Maps session id to the index of the session in the internal list.
    by_id: HashMap<SessionId, usize>,
    /// Maps title to all indices of sessions sharing that title.
    by_title: HashMap<String, Vec<usize>>,
    /// Maps session state (as string key) to all indices of sessions in that state.
    /// Uses String because contracts' SessionState does not implement Hash.
    by_state: HashMap<String, Vec<usize>>,
    /// Internal storage; indices in the hash maps reference into this vec.
    sessions: Vec<SessionSummary>,
}

impl SessionIndex {
    /// Creates an empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Convert SessionState to a String key for HashMap indexing.
    fn state_key(state: SessionState) -> String {
        match state {
            SessionState::Active => "active".to_string(),
            SessionState::Archived => "archived".to_string(),
        }
    }

    /// Indexes a session summary, replacing any existing entry with the same id.
    pub fn index(&mut self, summary: &SessionSummary) {
        let id = summary.id;

        if let Some(&old_pos) = self.by_id.get(&id) {
            // Remove the old entry from title and state buckets.
            if let Some(titles) = self.by_title.get_mut(&self.sessions[old_pos].title) {
                titles.retain(|&i| i != old_pos);
                if titles.is_empty() {
                    self.by_title.remove(&self.sessions[old_pos].title);
                }
            }
            let old_state_key = Self::state_key(self.sessions[old_pos].state);
            if let Some(states) = self.by_state.get_mut(&old_state_key) {
                states.retain(|&i| i != old_pos);
                if states.is_empty() {
                    self.by_state.remove(&old_state_key);
                }
            }

            // Overwrite in place.
            self.sessions[old_pos] = summary.clone();
            let new_state_key = Self::state_key(summary.state);
            self.by_title
                .entry(summary.title.clone())
                .or_default()
                .push(old_pos);
            self.by_state
                .entry(new_state_key)
                .or_default()
                .push(old_pos);
            return;
        }

        let pos = self.sessions.len();
        self.sessions.push(summary.clone());
        self.by_id.insert(id, pos);
        self.by_title
            .entry(summary.title.clone())
            .or_default()
            .push(pos);
        let state_key = Self::state_key(summary.state);
        self.by_state
            .entry(state_key)
            .or_default()
            .push(pos);
    }

    /// Returns the session with the given id, if present.
    pub fn get(&self, id: SessionId) -> Option<&SessionSummary> {
        self.by_id
            .get(&id)
            .map(|&pos| &self.sessions[pos])
    }

    /// Returns all sessions whose title matches exactly.
    pub fn find_by_title(&self, title: &str) -> Vec<&SessionSummary> {
        self.by_title
            .get(title)
            .map(|indices| {
                indices
                    .iter()
                    .map(|&i| &self.sessions[i])
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns all sessions in the given state.
    pub fn find_by_state(&self, state: SessionState) -> Vec<&SessionSummary> {
        let key = Self::state_key(state);
        self.by_state
            .get(&key)
            .map(|indices| {
                indices
                    .iter()
                    .map(|&i| &self.sessions[i])
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Rebuilds the index from the current internal session list.
    pub fn rebuild(&mut self) {
        self.by_id.clear();
        self.by_title.clear();
        self.by_state.clear();

        for (pos, session) in self.sessions.iter().enumerate() {
            let state_key = Self::state_key(session.state);
            self.by_id.insert(session.id, pos);
            self.by_title
                .entry(session.title.clone())
                .or_default()
                .push(pos);
            self.by_state
                .entry(state_key)
                .or_default()
                .push(pos);
        }
    }

    /// Removes all entries from the index.
    pub fn clear(&mut self) {
        self.by_id.clear();
        self.by_title.clear();
        self.by_state.clear();
        self.sessions.clear();
    }

    /// Returns the number of entries in each index.
    ///
    /// The tuple is `(by_id_count, by_title_count, by_state_count)`.
    #[must_use]
    pub fn stats(&self) -> (usize, usize, usize) {
        (
            self.by_id.len(),
            self.by_title.len(),
            self.by_state.len(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencode_rk_contracts::Timestamp;

    /// Helper to build a session summary for testing.
    fn make_session(title: &str, state: SessionState) -> SessionSummary {
        SessionSummary {
            id: SessionId::new(),
            title: title.to_owned(),
            state,
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            archived_at: None,
        }
    }

    #[test]
    fn index_and_get() {
        let mut idx = SessionIndex::new();
        let s1 = make_session("alpha", SessionState::Active);
        let s2 = make_session("beta", SessionState::Active);

        idx.index(&s1);
        idx.index(&s2);

        assert_eq!(idx.get(s1.id), Some(&s1));
        assert_eq!(idx.get(s2.id), Some(&s2));
        assert_eq!(idx.get(SessionId::new()), None);
    }

    #[test]
    fn find_by_title() {
        let mut idx = SessionIndex::new();
        let s1 = make_session("shared", SessionState::Active);
        let s2 = make_session("shared", SessionState::Archived);
        let s3 = make_session("other", SessionState::Active);

        idx.index(&s1);
        idx.index(&s2);
        idx.index(&s3);

        let found: Vec<_> = idx.find_by_title("shared");
        assert_eq!(found.len(), 2);
        let ids: Vec<_> = found.iter().map(|s| s.id).collect();
        assert!(ids.contains(&s1.id));
        assert!(ids.contains(&s2.id));

        assert!(idx.find_by_title("missing").is_empty());
    }

    #[test]
    fn find_by_state() {
        let mut idx = SessionIndex::new();
        let active1 = make_session("a", SessionState::Active);
        let active2 = make_session("b", SessionState::Active);
        let archived = make_session("c", SessionState::Archived);

        idx.index(&active1);
        idx.index(&active2);
        idx.index(&archived);

        let actives = idx.find_by_state(SessionState::Active);
        assert_eq!(actives.len(), 2);

        let archives = idx.find_by_state(SessionState::Archived);
        assert_eq!(archives.len(), 1);
    }

    #[test]
    fn rebuild() {
        let mut idx = SessionIndex::new();
        let s1 = make_session("x", SessionState::Active);
        let s2 = make_session("y", SessionState::Archived);

        idx.index(&s1);
        idx.index(&s2);

        // Mutate internal state to simulate corruption or reordering.
        // We rebuild and verify lookups still work.
        idx.sessions.reverse();
        idx.by_id.clear();
        idx.rebuild();

        assert_eq!(idx.get(s1.id), Some(&s1));
        assert_eq!(idx.get(s2.id), Some(&s2));
        assert_eq!(idx.find_by_title("x").len(), 1);
        assert_eq!(idx.find_by_state(SessionState::Active).len(), 1);
        assert_eq!(idx.find_by_state(SessionState::Archived).len(), 1);
    }

    #[test]
    fn stats_correct() {
        let mut idx = SessionIndex::new();
        let _a1 = make_session("a", SessionState::Active);
        let _a2 = make_session("a", SessionState::Active);
        let _b1 = make_session("b", SessionState::Archived);

        idx.index(&_a1);
        idx.index(&_a2);
        idx.index(&_b1);

        // by_id: 3 unique ids
        // by_title: 2 unique titles ("a", "b")
        // by_state: 2 unique states (Active, Archived)
        assert_eq!(idx.stats(), (3, 2, 2));
    }
}
