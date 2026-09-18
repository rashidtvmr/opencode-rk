//! Agent session tracking and management.
//!
//! Provides [`AgentSession`] for representing an agent's session and
//! [`SessionManager`] for operating a collection of in-memory sessions with
//! activity tracking and idle cleanup.

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;

/// Identifier type for sessions.
pub type SessionId = String;

/// Identifier type for agents.
pub type AgentId = String;

/// Errors that can occur when operating the session manager.
#[derive(Debug, Error)]
pub enum SessionError {
    /// The requested session does not exist.
    #[error("session not found: {0}")]
    NotFound(String),
    /// The session already exists with the given id.
    #[error("session already exists: {0}")]
    AlreadyExists(String),
}

/// Lifecycle state of an agent session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session is active and connected.
    Active,
    /// Session is idle (disconnected but retained).
    Idle,
    /// Session has terminated and is eligible for removal.
    Terminated,
}

/// A single agent session tracked by the manager.
#[derive(Debug, Clone)]
pub struct AgentSession {
    /// Unique session identifier.
    pub id: SessionId,
    /// Identifier of the agent owning this session.
    pub agent_id: AgentId,
    /// When the session was created.
    pub started_at: DateTime<Utc>,
    /// When the session last had activity.
    pub last_activity: DateTime<Utc>,
    /// Messages accumulated in this session.
    pub messages: Vec<String>,
    /// Current lifecycle state.
    pub state: SessionState,
}

impl AgentSession {
    /// Create a new session in the `Active` state.
    pub fn new(id: SessionId, agent_id: AgentId) -> Self {
        let now = Utc::now();
        Self {
            id,
            agent_id,
            started_at: now,
            last_activity: now,
            messages: Vec::new(),
            state: SessionState::Active,
        }
    }

    /// Mark the session as active and bump `last_activity`.
    pub fn touch(&mut self) {
        self.last_activity = Utc::now();
        self.state = SessionState::Active;
    }

    /// Move the session to idle state.
    pub fn to_idle(&mut self) {
        self.last_activity = Utc::now();
        self.state = SessionState::Idle;
    }

    /// Terminate the session.
    pub fn terminate(&mut self) {
        self.state = SessionState::Terminated;
    }

    /// Append a message and update activity.
    pub fn add_message(&mut self, message: String) {
        self.messages.push(message);
        self.last_activity = Utc::now();
    }

    /// Check whether the session has been idle longer than `max_idle`.
    pub fn is_idle_expired(&self, max_idle: Duration) -> bool {
        let elapsed = Utc::now() - self.last_activity;
        elapsed >= chrono::Duration::from_std(max_idle).unwrap_or(chrono::Duration::zero())
    }
}

/// In-memory manager for [`AgentSession`] instances.
#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: HashMap<SessionId, AgentSession>,
}

impl SessionManager {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Create and store a new session, returning a clone of it.
    ///
    /// Returns [`SessionError::AlreadyExists`] if the id is already in use.
    pub fn create(&mut self, id: SessionId, agent_id: AgentId) -> Result<AgentSession, SessionError> {
        if self.sessions.contains_key(&id) {
            return Err(SessionError::AlreadyExists(id));
        }
        let session = AgentSession::new(id.clone(), agent_id);
        self.sessions.insert(id.clone(), session.clone());
        Ok(session)
    }

    /// Retrieve a clone of the session with the given id, if present.
    pub fn get(&self, id: &SessionId) -> Option<AgentSession> {
        self.sessions.get(id).cloned()
    }

    /// Update activity timestamp for a session and set state to `Active`.
    ///
    /// Returns [`SessionError::NotFound`] if the session does not exist.
    pub fn activity(&mut self, id: &SessionId) -> Result<(), SessionError> {
        match self.sessions.get_mut(id) {
            Some(session) => {
                session.touch();
                Ok(())
            }
            None => Err(SessionError::NotFound(id.clone())),
        }
    }

    /// Terminate the session with the given id, setting its state to `Terminated`.
    ///
    /// Returns [`SessionError::NotFound`] if the session does not exist.
    pub fn terminate(&mut self, id: &SessionId) -> Result<(), SessionError> {
        match self.sessions.get_mut(id) {
            Some(session) => {
                session.terminate();
                Ok(())
            }
            None => Err(SessionError::NotFound(id.clone())),
        }
    }

    /// Remove sessions that are `Terminated` or idle longer than `max_idle`.
    ///
    /// Returns the number of sessions removed.
    pub fn cleanup_idle(&mut self, max_idle: Duration) -> usize {
        let ids_to_remove: Vec<SessionId> = self
            .sessions
            .iter()
            .filter(|(_, session)| {
                session.state == SessionState::Terminated || session.is_idle_expired(max_idle)
            })
            .map(|(id, _)| id.clone())
            .collect();

        let removed = ids_to_remove.len();
        for id in ids_to_remove {
            self.sessions.remove(&id);
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn create_and_get() {
        let mut mgr = SessionManager::new();
        let session = mgr.create("sess-1".to_string(), "agent-a".to_string()).unwrap();
        assert_eq!(session.id, "sess-1");
        assert_eq!(session.agent_id, "agent-a");
        assert_eq!(session.state, SessionState::Active);

        let fetched = mgr.get(&"sess-1".to_string()).unwrap();
        assert_eq!(fetched.id, "sess-1");
        assert_eq!(fetched.state, SessionState::Active);

        assert!(mgr.get(&"nonexistent".to_string()).is_none());

        // Duplicate creation should fail
        assert!(matches!(
            mgr.create("sess-1".to_string(), "agent-a".to_string()),
            Err(SessionError::AlreadyExists(_))
        ));
    }

    #[test]
    fn activity_updates() {
        let mut mgr = SessionManager::new();
        mgr.create("sess-1".to_string(), "agent-a".to_string()).unwrap();

        let before = mgr.get(&"sess-1".to_string()).unwrap().last_activity;
        thread::sleep(Duration::from_millis(20));

        mgr.activity(&"sess-1".to_string()).unwrap();
        let after = mgr.get(&"sess-1".to_string()).unwrap().last_activity;
        assert!(after > before);
        assert_eq!(mgr.get(&"sess-1".to_string()).unwrap().state, SessionState::Active);

        // Activity on nonexistent session should error
        assert!(matches!(
            mgr.activity(&"nope".to_string()),
            Err(SessionError::NotFound(_))
        ));
    }

    #[test]
    fn terminate_changes_state() {
        let mut mgr = SessionManager::new();
        mgr.create("sess-1".to_string(), "agent-a".to_string()).unwrap();

        assert_eq!(mgr.get(&"sess-1".to_string()).unwrap().state, SessionState::Active);

        mgr.terminate(&"sess-1".to_string()).unwrap();
        assert_eq!(mgr.get(&"sess-1".to_string()).unwrap().state, SessionState::Terminated);

        // Terminate nonexistent session should error
        assert!(matches!(
            mgr.terminate(&"nope".to_string()),
            Err(SessionError::NotFound(_))
        ));
    }

    #[test]
    fn cleanup_idle() {
        let mut mgr = SessionManager::new();
        mgr.create("sess-1".to_string(), "agent-a".to_string()).unwrap();

        // Terminate the session so cleanup removes it
        mgr.terminate(&"sess-1".to_string()).unwrap();
        let removed = mgr.cleanup_idle(Duration::from_secs(60));
        assert_eq!(removed, 1);
        assert!(mgr.get(&"sess-1".to_string()).is_none());

        // Nothing to clean when empty
        let removed = mgr.cleanup_idle(Duration::from_secs(60));
        assert_eq!(removed, 0);
    }

    #[test]
    fn multiple_sessions() {
        let mut mgr = SessionManager::new();
        mgr.create("sess-1".to_string(), "agent-a".to_string()).unwrap();
        mgr.create("sess-2".to_string(), "agent-b".to_string()).unwrap();
        mgr.create("sess-3".to_string(), "agent-c".to_string()).unwrap();

        assert_eq!(mgr.get(&"sess-1".to_string()).unwrap().agent_id, "agent-a");
        assert_eq!(mgr.get(&"sess-2".to_string()).unwrap().agent_id, "agent-b");
        assert_eq!(mgr.get(&"sess-3".to_string()).unwrap().agent_id, "agent-c");

        mgr.activity(&"sess-2".to_string()).unwrap();
        mgr.terminate(&"sess-1".to_string()).unwrap();

        let s2 = mgr.get(&"sess-2".to_string()).unwrap();
        assert_eq!(s2.state, SessionState::Active);
        assert_eq!(s2.agent_id, "agent-b");

        let s1 = mgr.get(&"sess-1".to_string()).unwrap();
        assert_eq!(s1.state, SessionState::Terminated);

        // Cleanup should remove the terminated session
        let removed = mgr.cleanup_idle(Duration::from_secs(60));
        assert_eq!(removed, 1);
        assert!(mgr.get(&"sess-1".to_string()).is_none());
        assert!(mgr.get(&"sess-2".to_string()).is_some());
        assert!(mgr.get(&"sess-3".to_string()).is_some());
    }
}
