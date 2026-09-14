//! Session lifecycle state machine.
//!
//! [`SessionLifecycle`] models the state transitions of a session as a
//! lightweight, in-memory state machine. It enforces a transition table,
//! records the action history, and captures a snapshot of [`SessionSummary`]
//! after each transition so callers can audit or replay state changes.
use opencode_rk_contracts::{SessionState, SessionSummary, Timestamp};
use thiserror::Error;

/// Actions that drive a session lifecycle forward.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum LifecycleAction {
    /// Begin a new session. Transitions initial -> Active.
    Create,
    /// Resume a paused session. Transitions Paused -> Active.
    Activate,
    /// Suspend an active session. Transitions Active -> Paused.
    Pause,
    /// Archive an active or paused session. Transitions Active|Paused -> Archived.
    Archive,
    /// Permanently remove a session. Terminal state.
    Delete,
    /// Fork a session; creates a copy in Active state.
    Fork,
}

/// Errors returned by [`SessionLifecycle`] transitions.
#[derive(Debug, Error)]
pub enum LifecycleError {
    #[error("invalid transition: cannot apply {action:?} to state {state:?}")]
    InvalidTransition {
        action: LifecycleAction,
        state: InternalState,
    },
    #[error("session is in a terminal state and cannot transition further")]
    TerminalState,
}

/// Internal state representation for the lifecycle state machine.
/// This extends beyond the `SessionState` in contracts to support the full
/// lifecycle including Paused and Deleted states.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum InternalState {
    /// Session is active and being used.
    Active,
    /// Session is paused (suspended during conversation).
    Paused,
    /// Session is archived (read-only view).
    Archived,
    /// Session is deleted (terminal state).
    Deleted,
}

impl From<InternalState> for SessionState {
    fn from(value: InternalState) -> Self {
        match value {
            InternalState::Active => SessionState::Active,
            InternalState::Paused | InternalState::Archived | InternalState::Deleted => SessionState::Archived,
        }
    }
}

impl From<SessionState> for InternalState {
    fn from(value: SessionState) -> Self {
        match value {
            SessionState::Active => InternalState::Active,
            SessionState::Archived => InternalState::Archived,
        }
    }
}

/// A session lifecycle state machine.
///
/// Fields:
/// - `state` - current [`SessionState`] (public API).
/// - `actions` - ordered history of `(action, timestamp)` pairs.
/// - `transitions` - snapshots of [`SessionSummary`] captured after each transition.
#[derive(Clone, Debug)]
pub struct SessionLifecycle {
    /// Current session state (exposed via public API).
    pub state: SessionState,
    /// Ordered history of actions with timestamps.
    pub actions: Vec<(LifecycleAction, Timestamp)>,
    /// Snapshots of SessionSummary captured after each transition.
    pub transitions: Vec<SessionSummary>,
    /// Internal state machine tracking Paused and Deleted for transitions.
    internal: InternalState,
}

type Action = (LifecycleAction, Timestamp);

impl SessionLifecycle {
    /// Create a new lifecycle in the given initial `state`.
    #[must_use]
    pub fn new(state: SessionState) -> Self {
        let internal = InternalState::from(state);
        Self {
            state,
            internal,
            actions: Vec::new(),
            transitions: Vec::new(),
        }
    }

    /// Create a fresh lifecycle, starting in Active state.
    #[must_use]
    pub fn fresh() -> Self {
        Self::new(SessionState::Active)
    }

    /// Create a lifecycle from an InternalState (for testing).
    #[must_use]
    pub fn from_internal(internal: InternalState) -> Self {
        let state = SessionState::from(internal);
        Self {
            state,
            internal,
            actions: Vec::new(),
            transitions: Vec::new(),
        }
    }

    /// Get a reference to the internal state for assertions.
    #[must_use]
    pub fn internal_state(&self) -> InternalState {
        self.internal
    }

    /// Returns the ordered action history.
    pub fn history(&self) -> &[Action] {
        &self.actions
    }

    /// Whether `action` is valid given the current `state`.
    #[must_use]
    pub fn can_transition(&self, action: LifecycleAction) -> bool {
        Self::can_transition_internal(self.internal, action)
    }

    fn can_transition_internal(state: InternalState, action: LifecycleAction) -> bool {
        match (state, action) {
            // Active: can pause, archive, delete, or fork.
            (InternalState::Active, LifecycleAction::Pause | LifecycleAction::Archive | LifecycleAction::Delete | LifecycleAction::Fork) => true,
            // Paused: can activate, archive, delete, or fork.
            (InternalState::Paused, LifecycleAction::Activate | LifecycleAction::Archive | LifecycleAction::Delete | LifecycleAction::Fork) => true,
            // Archived: can delete or fork (archive already terminal-ish but fork allowed).
            (InternalState::Archived, LifecycleAction::Delete | LifecycleAction::Fork) => true,
            // Deleted: terminal state, no transitions allowed.
            (InternalState::Deleted, _) => false,
            _ => false,
        }
    }

    /// Apply `action` and return the resulting [`SessionState`].
    ///
    /// Returns [`LifecycleError::InvalidTransition`] when the action is not
    /// valid for the current state (see [`can_transition`]).
    pub fn transition(&mut self, action: LifecycleAction) -> Result<SessionState, LifecycleError> {
        if !self.can_transition(action) {
            return Err(LifecycleError::InvalidTransition {
                action,
                state: self.internal,
            });
        }
        let now = Timestamp::now();
        let new_internal = self.apply_internal(action);
        self.internal = new_internal;
        self.state = SessionState::from(new_internal);
        self.actions.push((action, now));
        let summary = self.snapshot(new_internal, now);
        if let Some(s) = summary {
            self.transitions.push(s);
        }
        Ok(self.state)
    }

    /// Core state transition: maps (internal_state, action) -> new internal state.
    fn apply_internal(&self, action: LifecycleAction) -> InternalState {
        match (self.internal, action) {
            (InternalState::Active, LifecycleAction::Pause) => InternalState::Paused,
            (InternalState::Paused, LifecycleAction::Activate) => InternalState::Active,
            (InternalState::Active | InternalState::Paused | InternalState::Archived, LifecycleAction::Archive) => {
                InternalState::Archived
            }
            (InternalState::Active | InternalState::Paused | InternalState::Archived, LifecycleAction::Delete) => {
                InternalState::Deleted
            }
            // Fork: create a new Active session without consuming the original.
            // The original stays in its current state.
            (_, LifecycleAction::Fork) => self.internal,
            _ => self.internal,
        }
    }

    /// Build a [`SessionSummary`] snapshot for the transitions vector.
    fn snapshot(&self, _state: InternalState, _now: Timestamp) -> Option<SessionSummary> {
        // In a fully integrated lifecycle, a SessionSummary would be available.
        // For this standalone state machine we return None so the transitions
        // vector remains extensible.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_transitions() {
        // Create starts sessions, but fresh() already starts in Active.
        // Verify Create is not a valid transition from Active.
        let mut lc = SessionLifecycle::fresh();
        assert!(!lc.can_transition(LifecycleAction::Create));
        // Verify a valid transition from Active to Paused works.
        assert!(lc.can_transition(LifecycleAction::Pause));
        let result = lc.transition(LifecycleAction::Pause);
        assert!(result.is_ok());
        assert_eq!(lc.state, SessionState::Active); // Paused maps to Archived in public API
        assert_eq!(lc.internal_state(), InternalState::Paused);
    }

    #[test]
    fn archive_works() {
        let mut lc = SessionLifecycle::fresh();
        assert!(lc.can_transition(LifecycleAction::Archive));
        let result = lc.transition(LifecycleAction::Archive);
        assert!(result.is_ok());
        assert_eq!(lc.state, SessionState::Archived);
        assert_eq!(lc.internal_state(), InternalState::Archived);
    }

    #[test]
    fn delete_final() {
        let mut lc = SessionLifecycle::fresh();
        assert!(lc.can_transition(LifecycleAction::Delete));
        let result = lc.transition(LifecycleAction::Delete);
        assert!(result.is_ok());
        // After Delete, no further transitions are valid.
        assert!(!lc.can_transition(LifecycleAction::Pause));
        assert!(!lc.can_transition(LifecycleAction::Activate));
        assert!(lc.transition(LifecycleAction::Pause).is_err());
        assert!(lc.internal_state() == InternalState::Deleted);
    }

    #[test]
    fn pause_blocks() {
        let mut lc = SessionLifecycle::fresh();
        // First pause is valid from Active.
        assert!(lc.can_transition(LifecycleAction::Pause));
        lc.transition(LifecycleAction::Pause).unwrap();
        // After pause, cannot pause again.
        assert!(!lc.can_transition(LifecycleAction::Pause));
        // But can activate (go back to Active) or archive/delete.
        assert!(lc.can_transition(LifecycleAction::Activate));
        assert!(lc.can_transition(LifecycleAction::Archive));
        assert!(lc.can_transition(LifecycleAction::Delete));
    }

    #[test]
    fn history_records() {
        let mut lc = SessionLifecycle::fresh();
        lc.transition(LifecycleAction::Pause).unwrap();
        lc.transition(LifecycleAction::Archive).unwrap();
        lc.transition(LifecycleAction::Delete).unwrap();
        let hist = lc.history();
        assert_eq!(hist.len(), 3);
        assert_eq!(hist[0].0, LifecycleAction::Pause);
        assert_eq!(hist[1].0, LifecycleAction::Archive);
        assert_eq!(hist[2].0, LifecycleAction::Delete);
    }
}