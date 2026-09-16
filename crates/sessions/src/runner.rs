//! Bounded per-session single-flight runner (RUN-001 slice).
//!
//! Caller-owned slot map with RAII guards. No threads, no processes, no I/O,
//! no clock. At most one normal plus one shell guard per session.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Per-run step budget replacing upstream Infinity.
pub const MAX_STEPS: u32 = 64;
/// Maximum session-id length.
pub const MAX_SESSION_ID_LEN: usize = 128;

/// Which execution lane a guard holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunKind {
    Normal,
    Shell,
}

/// Observable per-session runner state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunnerState {
    Idle,
    Running { kind: RunKind },
    Busy,
}

/// Typed contention rejection; carries the session id only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BusyError {
    pub session: String,
}

impl std::fmt::Display for BusyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "session {} is busy", self.session)
    }
}

impl std::error::Error for BusyError {}

/// Runner validation failures (not contention).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunnerError {
    InvalidInput,
    StepBudget,
    Busy { session: String },
}

impl std::fmt::Display for RunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput => f.write_str("invalid session id"),
            Self::StepBudget => f.write_str("step budget exhausted"),
            Self::Busy { .. } => f.write_str("session is busy"),
        }
    }
}

impl std::error::Error for RunnerError {}

impl From<BusyError> for RunnerError {
    fn from(value: BusyError) -> Self {
        Self::Busy {
            session: value.session,
        }
    }
}

#[derive(Debug, Default)]
struct SessionSlots {
    normal: bool,
    shell: bool,
    completed: u64,
    cancelled: u64,
}

#[derive(Debug, Default)]
struct Inner {
    slots: HashMap<String, SessionSlots>,
}

/// Caller-owned bounded runner set.
#[derive(Clone, Debug, Default)]
pub struct RunnerSet {
    inner: Arc<Mutex<Inner>>,
}

impl RunnerSet {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn valid(session: &str) -> bool {
        !session.is_empty() && session.len() <= MAX_SESSION_ID_LEN
    }

    /// Start one lane. Rejections leave the set unchanged.
    pub fn start(&self, session: &str, kind: RunKind) -> Result<RunGuard, RunnerError> {
        if !Self::valid(session) {
            return Err(RunnerError::InvalidInput);
        }
        let mut inner = self.inner.lock().unwrap();
        let slots = inner.slots.entry(session.to_owned()).or_default();
        let occupied = match kind {
            RunKind::Normal => slots.normal,
            RunKind::Shell => slots.shell,
        };
        if occupied {
            return Err(RunnerError::Busy {
                session: session.to_owned(),
            });
        }
        match kind {
            RunKind::Normal => slots.normal = true,
            RunKind::Shell => slots.shell = true,
        }
        Ok(RunGuard {
            set: self.inner.clone(),
            session: session.to_owned(),
            kind,
            steps_left: MAX_STEPS,
            settled: false,
        })
    }

    /// Observable state: Running when any guard is alive.
    #[must_use]
    pub fn status(&self, session: &str) -> RunnerState {
        let inner = self.inner.lock().unwrap();
        match inner.slots.get(session) {
            Some(s) if s.normal || s.shell => RunnerState::Running {
                kind: if s.normal {
                    RunKind::Normal
                } else {
                    RunKind::Shell
                },
            },
            Some(_) if inner.slots.contains_key(session) => RunnerState::Idle,
            _ => RunnerState::Idle,
        }
    }

    /// Live guard count for one session.
    #[must_use]
    pub fn live_count(&self, session: &str) -> usize {
        let inner = self.inner.lock().unwrap();
        match inner.slots.get(session) {
            Some(s) => usize::from(s.normal) + usize::from(s.shell),
            None => 0,
        }
    }

    /// Release every live slot for one session; unknown session is harmless.
    pub fn cancel_all(&self, session: &str) -> usize {
        let mut inner = self.inner.lock().unwrap();
        match inner.slots.get_mut(session) {
            Some(s) => {
                let n = usize::from(s.normal) + usize::from(s.shell);
                s.cancelled += n as u64;
                s.normal = false;
                s.shell = false;
                n
            }
            None => 0,
        }
    }
}

/// RAII execution handle. Drop releases the slot as Cancelled.
#[derive(Debug)]
pub struct RunGuard {
    set: Arc<Mutex<Inner>>,
    session: String,
    kind: RunKind,
    steps_left: u32,
    settled: bool,
}

impl RunGuard {
    /// Steps remaining in this run's budget.
    #[must_use]
    pub fn steps_remaining(&self) -> u32 {
        self.steps_left
    }

    /// Consume one step; errors at zero without looping.
    pub fn take_step(&mut self) -> Result<(), RunnerError> {
        if self.steps_left == 0 {
            return Err(RunnerError::StepBudget);
        }
        self.steps_left -= 1;
        Ok(())
    }

    fn release(&mut self, completed: bool) {
        if self.settled {
            return;
        }
        self.settled = true;
        if let Ok(mut inner) = self.set.lock() {
            if let Some(slots) = inner.slots.get_mut(&self.session) {
                match self.kind {
                    RunKind::Normal => slots.normal = false,
                    RunKind::Shell => slots.shell = false,
                }
                if completed {
                    slots.completed += 1;
                } else {
                    slots.cancelled += 1;
                }
            }
        }
    }

    /// Release the slot as completed.
    pub fn complete(mut self) {
        self.release(true);
    }

    /// Release the slot as cancelled.
    pub fn cancel(mut self) {
        self.release(false);
    }
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        self.release(false);
    }
}
