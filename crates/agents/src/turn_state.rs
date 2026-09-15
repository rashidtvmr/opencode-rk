//! Typed single-turn submission lifecycle.

use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TurnId(u64);

impl TurnId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TurnPhase {
    Idle,
    Submitted,
    Running,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TurnSubmission {
    id: TurnId,
    prompt: String,
}

impl TurnSubmission {
    #[must_use]
    pub fn new(id: TurnId, prompt: impl Into<String>) -> Self {
        Self {
            id,
            prompt: prompt.into(),
        }
    }

    #[must_use]
    pub const fn id(&self) -> TurnId {
        self.id
    }

    #[must_use]
    pub fn prompt(&self) -> &str {
        &self.prompt
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TurnStateError {
    #[error("turn already active: {active:?}")]
    AlreadyActive { active: TurnId },
    #[error("wrong turn: active={active:?} requested={requested:?}")]
    WrongTurn { active: TurnId, requested: TurnId },
    #[error("invalid turn transition from {from:?} via {operation}")]
    InvalidTransition {
        from: TurnPhase,
        operation: &'static str,
    },
}

#[derive(Clone, Debug)]
pub struct TurnSubmissionState {
    phase: TurnPhase,
    active: Option<TurnSubmission>,
}

impl Default for TurnSubmissionState {
    fn default() -> Self {
        Self::new()
    }
}

impl TurnSubmissionState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            phase: TurnPhase::Idle,
            active: None,
        }
    }

    #[must_use]
    pub const fn phase(&self) -> TurnPhase {
        self.phase
    }

    #[must_use]
    pub fn active(&self) -> Option<&TurnSubmission> {
        self.active.as_ref()
    }

    pub fn submit(&mut self, turn: TurnSubmission) -> Result<(), TurnStateError> {
        if let Some(active) = self.active.as_ref() {
            return Err(TurnStateError::AlreadyActive {
                active: active.id(),
            });
        }
        self.active = Some(turn);
        self.phase = TurnPhase::Submitted;
        Ok(())
    }

    pub fn start(&mut self, requested: TurnId) -> Result<(), TurnStateError> {
        self.ensure_active(requested)?;
        if self.phase != TurnPhase::Submitted {
            return Err(TurnStateError::InvalidTransition {
                from: self.phase,
                operation: "start",
            });
        }
        self.phase = TurnPhase::Running;
        Ok(())
    }

    pub fn complete(&mut self, requested: TurnId) -> Result<(), TurnStateError> {
        self.ensure_active(requested)?;
        if self.phase != TurnPhase::Running {
            return Err(TurnStateError::InvalidTransition {
                from: self.phase,
                operation: "complete",
            });
        }
        self.active = None;
        self.phase = TurnPhase::Idle;
        Ok(())
    }

    pub fn interrupt(&mut self, requested: TurnId) -> Result<(), TurnStateError> {
        self.ensure_active(requested)?;
        if self.phase != TurnPhase::Running {
            return Err(TurnStateError::InvalidTransition {
                from: self.phase,
                operation: "interrupt",
            });
        }
        self.active = None;
        self.phase = TurnPhase::Idle;
        Ok(())
    }

    fn ensure_active(&self, requested: TurnId) -> Result<(), TurnStateError> {
        let Some(active) = self.active.as_ref() else {
            return Err(TurnStateError::InvalidTransition {
                from: self.phase,
                operation: "active_turn_required",
            });
        };
        if active.id() != requested {
            return Err(TurnStateError::WrongTurn {
                active: active.id(),
                requested,
            });
        }
        Ok(())
    }
}
