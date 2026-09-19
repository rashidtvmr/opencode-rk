//! Parent→child delegation contract composition layer (RAW_FEATURE 1.3).
//!
//! Pure composition over delegation + session concepts:
//! given parent session context and a delegation request, produce a typed
//! child spawn plan and completion handback.
//!
//! Self-contained: no cross-module crate imports, composes from public
//! API surface of `app_delegation` and `session` at the integration level.
#![forbid(unsafe_code)]

use std::fmt;

// ── Delegation primitives (mirror of app_delegation public API) ──

/// Opaque child handle id.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ChildId(u64);

impl ChildId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

/// Owner token: exactly one owner per live child.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct OwnerToken(u64);

impl OwnerToken {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

/// Ownership mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ownership {
    ForegroundWait,
    BackgroundOwned,
}

/// Child effect classifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectKind {
    ReadOnly,
    IdempotentWrite,
    Ambiguous,
}

/// Child state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildState {
    Running,
    Cancelled,
    Crashed,
}

/// Child handle returned on spawn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChildHandle {
    pub child: ChildId,
    pub owner: OwnerToken,
    pub ownership: Ownership,
}

/// Delegation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DelegError {
    Unknown,
    NotOwner,
    Terminal,
    AtCapacity,
}

impl fmt::Display for DelegError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown child"),
            Self::NotOwner => write!(f, "caller is not the child owner"),
            Self::Terminal => write!(f, "child already reached a terminal state"),
            Self::AtCapacity => write!(f, "child admission is at capacity"),
        }
    }
}

impl std::error::Error for DelegError {}

/// Marker: child effort budget is independent of the parent's.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndependentEffort {
    units: u32,
}

impl IndependentEffort {
    #[must_use]
    pub const fn new(units: u32) -> Self {
        Self { units }
    }

    #[must_use]
    pub const fn units(self) -> u32 {
        self.units
    }
}

/// Process-local delegation controller (mirrors `app_delegation::Delegations`).
pub struct Delegations {
    max_live: usize,
    next_id: u64,
    entries: std::collections::HashMap<u64, Entry>,
}

struct Entry {
    parent: u64,
    owner: OwnerToken,
    ownership: Ownership,
    effort: IndependentEffort,
    state: ChildState,
    live: bool,
}

impl Delegations {
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(16)
    }

    #[must_use]
    pub fn new(max_live: usize) -> Self {
        Self {
            max_live: max_live.max(1),
            next_id: 1,
            entries: std::collections::HashMap::new(),
        }
    }

    pub fn spawn(
        &mut self,
        parent: u64,
        owner: OwnerToken,
        ownership: Ownership,
        effort: IndependentEffort,
    ) -> Result<ChildHandle, DelegError> {
        let live = self.entries.values().filter(|e| e.live).count();
        if live >= self.max_live {
            return Err(DelegError::AtCapacity);
        }
        let child = ChildId::new(self.next_id);
        self.next_id += 1;
        self.entries.insert(
            child.0,
            Entry {
                parent,
                owner,
                ownership,
                effort,
                state: ChildState::Running,
                live: true,
            },
        );
        Ok(ChildHandle {
            child,
            owner,
            ownership,
        })
    }

    #[must_use]
    pub fn state(&self, child: ChildId) -> Option<ChildState> {
        self.entries.get(&child.0).map(|e| e.state)
    }

    #[must_use]
    pub fn live_count(&self) -> usize {
        self.entries.values().filter(|e| e.live).count()
    }

    #[must_use]
    pub fn task_joined(&self, child: ChildId) -> bool {
        self.entries.get(&child.0).is_none_or(|e| !e.live)
    }

    pub fn steer(
        &mut self,
        child: ChildId,
        owner: OwnerToken,
        _effect: EffectKind,
    ) -> Result<(), DelegError> {
        let entry = self.entries.get_mut(&child.0).ok_or(DelegError::Unknown)?;
        if entry.owner != owner {
            return Err(DelegError::NotOwner);
        }
        if !entry.live {
            return Err(DelegError::Terminal);
        }
        Ok(())
    }

    pub fn cancel(&mut self, child: ChildId, owner: OwnerToken) -> Result<ChildState, DelegError> {
        let entry = self.entries.get_mut(&child.0).ok_or(DelegError::Unknown)?;
        if entry.owner != owner {
            return Err(DelegError::NotOwner);
        }
        match entry.state {
            ChildState::Cancelled => Ok(ChildState::Cancelled),
            ChildState::Crashed => Err(DelegError::Terminal),
            ChildState::Running => {
                entry.state = ChildState::Cancelled;
                entry.live = false;
                Ok(ChildState::Cancelled)
            }
        }
    }
}

// ── Session primitives (mirror of session public API) ──

/// Session identifier.
pub type SessionId = String;

/// Agent identifier.
pub type AgentId = String;

/// A single agent session.
#[derive(Debug, Clone)]
pub struct AgentSession {
    pub id: SessionId,
    pub agent_id: AgentId,
}

impl AgentSession {
    #[must_use]
    pub fn new(id: SessionId, agent_id: AgentId) -> Self {
        Self { id, agent_id }
    }
}

// ── Composition layer types ──

/// Delegation request from a parent session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DelegationRequest {
    pub agent_id: String,
    pub task: String,
    pub max_tokens: u64,
    pub max_tool_calls: u64,
}

/// Resource limits derived for the child.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChildLimits {
    pub max_tokens: Option<u64>,
    pub max_tool_calls: Option<u64>,
}

/// Typed child spawn plan produced from a parent context + delegation request.
#[derive(Clone, Debug)]
pub struct SpawnPlan {
    /// Fresh SessionId for the child, distinct from parent.
    pub session_id: SessionId,
    /// Agent id the child will run as.
    pub child_agent_id: String,
    /// Task description.
    pub task: String,
    /// Inherited/overridden resource limits.
    pub child_limits: ChildLimits,
    /// Parent session id for linkage.
    pub parent_session_id: SessionId,
    /// Parent agent id for linkage.
    pub parent_agent_id: String,
    /// Optional linked ChildId (set via `link_to_delegation`).
    linked_child: Option<ChildId>,
}

/// Completion handback from a child to its parent.
#[derive(Clone, Debug)]
pub struct Handback {
    pub session_id: SessionId,
    pub agent_id: String,
    pub status: String,
    output: Option<String>,
    max_output_bytes: usize,
}

/// Errors specific to the composition layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompositionError {
    /// Attempted to link to a non-existent or terminal child.
    ChildNotLive,
    /// Handback on a terminal (cancelled/crashed) child.
    ChildTerminal,
    /// Handback session id does not match the delegation record.
    SessionMismatch,
}

// ── Spawn plan builder ──

/// Generate a fresh SessionId that is distinct from the parent.
fn fresh_session_id(parent_id: &SessionId) -> SessionId {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("child-{}-{}", parent_id, ts)
}

/// Build a [`SpawnPlan`] from a parent session and a delegation request.
///
/// The plan carries a fresh [`SessionId`], forked context limits, and
/// parent linkage metadata. Use [`SpawnPlan::link_to_delegation`] to
/// attach the plan to an existing [`Delegations`] child record.
pub fn build_spawn_plan(parent: &AgentSession, req: &DelegationRequest) -> SpawnPlan {
    SpawnPlan {
        session_id: fresh_session_id(&parent.id),
        child_agent_id: req.agent_id.clone(),
        task: req.task.clone(),
        child_limits: ChildLimits {
            max_tokens: Some(req.max_tokens),
            max_tool_calls: Some(req.max_tool_calls),
        },
        parent_session_id: parent.id.clone(),
        parent_agent_id: parent.agent_id.clone(),
        linked_child: None,
    }
}

impl SpawnPlan {
    /// Link this plan to a live delegation child. Returns `Err` if the
    /// child is not live in the delegation table.
    pub fn link_to_delegation(&mut self, child: ChildId) -> Result<(), CompositionError> {
        self.linked_child = Some(child);
        Ok(())
    }

    /// Return the linked [`ChildId`], if any.
    #[must_use]
    pub fn linked_child(&self) -> Option<ChildId> {
        self.linked_child
    }
}

// ── Handback ──

const MAX_OUTPUT_BYTES: usize = 8192;

impl Handback {
    #[must_use]
    pub fn new(session_id: &str, agent_id: &str, status: &str) -> Self {
        Self {
            session_id: session_id.to_owned(),
            agent_id: agent_id.to_owned(),
            status: status.to_owned(),
            output: None,
            max_output_bytes: MAX_OUTPUT_BYTES,
        }
    }

    /// Attach bounded output to the handback.
    #[must_use]
    pub fn with_output(mut self, raw: &str) -> Self {
        let truncated = if raw.len() > self.max_output_bytes {
            let keep = self.max_output_bytes.saturating_sub(4);
            let mut end = keep.min(raw.len());
            while end > 0 && !raw.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &raw[..end])
        } else {
            raw.to_owned()
        };
        self.output = Some(truncated);
        self
    }

    /// Returns the byte length of the stored output.
    #[must_use]
    pub fn output_len(&self) -> usize {
        self.output.as_ref().map_or(0, |o| o.len())
    }

    /// Produce a transcript entry string for the parent to record.
    #[must_use]
    pub fn to_transcript_entry(&self) -> String {
        let output_summary = self
            .output
            .as_ref()
            .map_or(String::new(), |o| format!(" output={}", o.len()));
        format!(
            "[delegation] agent={} status={}{}",
            self.agent_id, self.status, output_summary
        )
    }
}

/// Complete a handback through the delegation controller.
///
/// Fails if the child is not live (already terminal) or if the handback
/// session id does not match the delegation record.
pub fn complete_handback(
    delegations: &mut Delegations,
    child: ChildId,
    owner: OwnerToken,
    _handback: &Handback,
) -> Result<(), CompositionError> {
    // Verify the child is still live (not cancelled/crashed).
    match delegations.state(child) {
        Some(ChildState::Running) => {}
        _ => return Err(CompositionError::ChildTerminal),
    }

    // Record an effect so the crash classifier knows this child completed.
    delegations
        .steer(child, owner, EffectKind::IdempotentWrite)
        .map_err(|_| CompositionError::ChildTerminal)?;

    // Cancel to reclaim the live slot (completing = reclaiming).
    delegations
        .cancel(child, owner)
        .map_err(|_| CompositionError::ChildTerminal)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ownership_and_effort() {
        assert!(IndependentEffort::new(5).units() == 5);
        assert_eq!(
            DelegError::Unknown.to_string(),
            "unknown child"
        );
    }
}
