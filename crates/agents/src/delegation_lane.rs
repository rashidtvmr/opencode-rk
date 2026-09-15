//! Foreground/background delegation with explicit ownership (AUTO-004).
//!
//! Controller-owned, process-local delegation records. Delegations run as
//! in-process state only: no OS process per agent, no background thread, no
//! log reads for status. Every live delegation has exactly one owner token;
//! `cancel` reclaims the live slot and drops retained output within the
//! configured bound.
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::time::Duration;

use thiserror::Error;

pub const DEFAULT_MAX_LIVE: usize = 16;
pub const DEFAULT_MAX_SUMMARY_BYTES: usize = 4096;
pub const DEFAULT_CANCEL_TIMEOUT_MS: u64 = 1000;

const TRUNCATION_MARKER: &str = "...[truncated]";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct DelegationId(u64);

impl DelegationId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    const fn key(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct OwnerToken(u64);

impl OwnerToken {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Foreground,
    Background,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Submitted,
    Running,
    Detached,
    Complete,
    Cancelled,
    Failed,
}

impl State {
    const fn is_live(self) -> bool {
        matches!(self, Self::Submitted | Self::Running | Self::Detached)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Handle {
    pub id: DelegationId,
    pub owner: OwnerToken,
    pub mode: Mode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedSummary(String);

impl BoundedSummary {
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Status {
    pub id: DelegationId,
    pub owner: OwnerToken,
    pub mode: Mode,
    pub state: State,
    pub summary: BoundedSummary,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DelegError {
    #[error("unknown delegation")]
    Unknown,
    #[error("caller is not the delegation owner")]
    NotOwner,
    #[error("delegation already reached a terminal state")]
    Terminal,
    #[error("delegation admission is at capacity")]
    AtCapacity,
}

struct Entry {
    owner: OwnerToken,
    mode: Mode,
    state: State,
    summary: BoundedSummary,
    live_task: bool,
}

pub struct DelegationController {
    max_live: usize,
    max_summary_bytes: usize,
    cancel_timeout: Duration,
    next_id: u64,
    entries: HashMap<u64, Entry>,
}

impl DelegationController {
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(
            DEFAULT_MAX_LIVE,
            DEFAULT_MAX_SUMMARY_BYTES,
            Duration::from_millis(DEFAULT_CANCEL_TIMEOUT_MS),
        )
    }

    #[must_use]
    pub fn new(
        max_live: usize,
        max_summary_bytes: usize,
        cancel_timeout: Duration,
    ) -> Self {
        Self {
            max_live,
            max_summary_bytes,
            cancel_timeout,
            next_id: 1,
            entries: HashMap::new(),
        }
    }

    fn truncate(&self, text: &str) -> BoundedSummary {
        if text.len() <= self.max_summary_bytes {
            return BoundedSummary(text.to_owned());
        }
        let keep = self
            .max_summary_bytes
            .saturating_sub(TRUNCATION_MARKER.len());
        let mut end = keep.min(text.len());
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        let mut out = String::with_capacity(self.max_summary_bytes);
        out.push_str(&text[..end]);
        out.push_str(TRUNCATION_MARKER);
        BoundedSummary(out)
    }

    fn entry_status(id: DelegationId, entry: &Entry) -> Status {
        Status {
            id,
            owner: entry.owner,
            mode: entry.mode,
            state: entry.state,
            summary: entry.summary.clone(),
        }
    }

    pub fn submit(
        &mut self,
        owner: OwnerToken,
        mode: Mode,
        work: impl Into<String>,
    ) -> Result<Handle, DelegError> {
        let live = self.entries.values().filter(|e| e.state.is_live()).count();
        if live >= self.max_live {
            return Err(DelegError::AtCapacity);
        }
        let id = DelegationId::new(self.next_id);
        self.next_id += 1;
        let summary = self.truncate(&work.into());
        self.entries.insert(
            id.key(),
            Entry {
                owner,
                mode,
                state: State::Running,
                summary,
                live_task: true,
            },
        );
        Ok(Handle { id, owner, mode })
    }

    pub fn detach(
        &mut self,
        id: DelegationId,
        owner: OwnerToken,
    ) -> Result<Status, DelegError> {
        let entry = self.entries.get_mut(&id.key()).ok_or(DelegError::Unknown)?;
        if entry.owner != owner {
            return Err(DelegError::NotOwner);
        }
        match entry.state {
            State::Running => {
                entry.state = State::Detached;
                Ok(Self::entry_status(id, entry))
            }
            State::Detached => Ok(Self::entry_status(id, entry)),
            State::Submitted => {
                entry.state = State::Detached;
                Ok(Self::entry_status(id, entry))
            }
            State::Complete | State::Cancelled | State::Failed => {
                Err(DelegError::Terminal)
            }
        }
    }

    pub fn status(&self, id: DelegationId) -> Result<Status, DelegError> {
        self.entries
            .get(&id.key())
            .map(|entry| Self::entry_status(id, entry))
            .ok_or(DelegError::Unknown)
    }

    pub fn cancel(
        &mut self,
        id: DelegationId,
        owner: OwnerToken,
    ) -> Result<Status, DelegError> {
        let entry = self.entries.get_mut(&id.key()).ok_or(DelegError::Unknown)?;
        if entry.owner != owner {
            return Err(DelegError::NotOwner);
        }
        match entry.state {
            State::Cancelled => Ok(Self::entry_status(id, entry)),
            State::Complete | State::Failed => Err(DelegError::Terminal),
            State::Submitted | State::Running | State::Detached => {
                entry.state = State::Cancelled;
                entry.live_task = false;
                entry.summary = BoundedSummary("cancelled".to_owned());
                Ok(Self::entry_status(id, entry))
            }
        }
    }

    pub fn complete(
        &mut self,
        id: DelegationId,
        owner: OwnerToken,
        summary: impl Into<String>,
    ) -> Result<Status, DelegError> {
        let bounded = self.truncate(&summary.into());
        let entry = self.entries.get_mut(&id.key()).ok_or(DelegError::Unknown)?;
        if entry.owner != owner {
            return Err(DelegError::NotOwner);
        }
        if !entry.state.is_live() {
            return Err(DelegError::Terminal);
        }
        entry.state = State::Complete;
        entry.live_task = false;
        entry.summary = bounded;
        Ok(Self::entry_status(id, entry))
    }

    pub fn fail(
        &mut self,
        id: DelegationId,
        summary: impl Into<String>,
    ) -> Result<Status, DelegError> {
        let bounded = self.truncate(&summary.into());
        let entry = self.entries.get_mut(&id.key()).ok_or(DelegError::Unknown)?;
        if !entry.state.is_live() {
            return Err(DelegError::Terminal);
        }
        entry.state = State::Failed;
        entry.live_task = false;
        entry.summary = bounded;
        Ok(Self::entry_status(id, entry))
    }

    #[must_use]
    pub fn task_joined(&self, id: DelegationId) -> bool {
        self.entries.get(&id.key()).is_none_or(|e| !e.live_task)
    }

    #[must_use]
    pub fn live_count(&self) -> usize {
        self.entries.values().filter(|e| e.state.is_live()).count()
    }

    #[must_use]
    pub fn child_process_count(&self) -> u64 {
        0
    }

    #[must_use]
    pub fn max_summary_bytes(&self) -> usize {
        self.max_summary_bytes
    }

    #[must_use]
    pub fn cancel_timeout(&self) -> Duration {
        self.cancel_timeout
    }

    /// Process-local restart: drop every live slot. Live records become small
    /// `Failed` summaries; terminal records are retained unchanged.
    pub fn restart(&mut self) {
        for entry in self.entries.values_mut() {
            if entry.state.is_live() {
                entry.state = State::Failed;
                entry.live_task = false;
                entry.summary =
                    BoundedSummary("restart: process-local state dropped".to_owned());
            }
        }
    }
}
