//! Parent/child app delegation types (PAR-004).
//!
//! Controller-owned, process-local child records. No OS process per child,
//! no thread, no queue without bound. Every live child has one owner;
//! `cancel`/`crash` reclaim the live slot; ambiguous effects never replay.
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt;

/// Opaque child handle id.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ChildId(u64);

impl ChildId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    const fn key(self) -> u64 {
        self.0
    }
}

/// Persisted parent -> child edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ParentLink {
    pub parent: u64,
    pub child: ChildId,
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

/// Ownership + cancel rule. Foreground parent waits on the child;
/// background child runs detached and is reclaimed by owner cancel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ownership {
    ForegroundWait,
    BackgroundOwned,
}

impl Ownership {
    #[must_use]
    pub const fn parent_waits(self) -> bool {
        matches!(self, Self::ForegroundWait)
    }
}

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

    /// Type-level marker: always independent.
    #[must_use]
    pub const fn is_independent(self) -> bool {
        true
    }
}

/// Local no-blind-retry classifier (no cross-crate dependency).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectKind {
    ReadOnly,
    IdempotentWrite,
    Ambiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryDecision {
    RetryAllowed,
    NeverReplay,
}

#[must_use]
pub const fn classify(effect: EffectKind) -> RetryDecision {
    match effect {
        EffectKind::Ambiguous => RetryDecision::NeverReplay,
        EffectKind::ReadOnly | EffectKind::IdempotentWrite => RetryDecision::RetryAllowed,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildState {
    Running,
    Cancelled,
    Crashed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChildHandle {
    pub child: ChildId,
    pub owner: OwnerToken,
    pub ownership: Ownership,
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CrashOutcome {
    pub child: ChildId,
    pub replay_allowed: bool,
    pub reclaimed: bool,
}

struct Entry {
    parent: u64,
    owner: OwnerToken,
    ownership: Ownership,
    effort: IndependentEffort,
    state: ChildState,
    live: bool,
    last_effect: Option<EffectKind>,
}

pub const DEFAULT_MAX_CHILDREN: usize = 16;

pub struct Delegations {
    max_live: usize,
    next_id: u64,
    entries: HashMap<u64, Entry>,
}

impl Delegations {
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(DEFAULT_MAX_CHILDREN)
    }

    #[must_use]
    pub fn new(max_live: usize) -> Self {
        Self {
            max_live: max_live.max(1),
            next_id: 1,
            entries: HashMap::new(),
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
            child.key(),
            Entry {
                parent,
                owner,
                ownership,
                effort,
                state: ChildState::Running,
                live: true,
                last_effect: None,
            },
        );
        Ok(ChildHandle {
            child,
            owner,
            ownership,
        })
    }

    #[must_use]
    pub fn link(&self, child: ChildId) -> Option<ParentLink> {
        self.entries
            .get(&child.key())
            .map(|e| ParentLink {
                parent: e.parent,
                child,
            })
    }

    #[must_use]
    pub fn state(&self, child: ChildId) -> Option<ChildState> {
        self.entries.get(&child.key()).map(|e| e.state)
    }

    #[must_use]
    pub fn effort(&self, child: ChildId) -> Option<IndependentEffort> {
        self.entries.get(&child.key()).map(|e| e.effort)
    }

    #[must_use]
    pub fn ownership_of(&self, child: ChildId) -> Option<Ownership> {
        self.entries.get(&child.key()).map(|e| e.ownership)
    }

    #[must_use]
    pub fn live_count(&self) -> usize {
        self.entries.values().filter(|e| e.live).count()
    }

    #[must_use]
    pub fn task_joined(&self, child: ChildId) -> bool {
        self.entries.get(&child.key()).is_none_or(|e| !e.live)
    }

    /// Route a steer to the intended running child only. Owner-checked;
    /// unknown id -> Unknown, wrong owner -> NotOwner (no mutation),
    /// terminal child -> Terminal. Records the effect kind for crash rules.
    pub fn steer(
        &mut self,
        child: ChildId,
        owner: OwnerToken,
        effect: EffectKind,
    ) -> Result<(), DelegError> {
        let entry = self.entries.get_mut(&child.key()).ok_or(DelegError::Unknown)?;
        if entry.owner != owner {
            return Err(DelegError::NotOwner);
        }
        if !entry.live {
            return Err(DelegError::Terminal);
        }
        entry.last_effect = Some(effect);
        Ok(())
    }

    /// Owner cancel reclaims the live slot. Double cancel is idempotent;
    /// cancel-after-terminal -> Terminal; wrong owner -> NotOwner, no mutation.
    pub fn cancel(&mut self, child: ChildId, owner: OwnerToken) -> Result<ChildState, DelegError> {
        let entry = self.entries.get_mut(&child.key()).ok_or(DelegError::Unknown)?;
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

    /// Crash reclaims the live slot. Ambiguous effects never replay
    /// (returns replay_allowed=false); recorded non-ambiguous effects may.
    /// Crash on a non-live child -> Terminal.
    pub fn crash(&mut self, child: ChildId, effect: EffectKind) -> Result<CrashOutcome, DelegError> {
        let entry = self.entries.get_mut(&child.key()).ok_or(DelegError::Unknown)?;
        if !entry.live {
            return Err(DelegError::Terminal);
        }
        let kind = entry.last_effect.unwrap_or(effect);
        let replay_allowed = matches!(classify(kind), RetryDecision::RetryAllowed);
        entry.state = ChildState::Crashed;
        entry.live = false;
        Ok(CrashOutcome {
            child,
            replay_allowed,
            reclaimed: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steer_hits_intended_child() {
        let mut d = Delegations::with_defaults();
        let a = d
            .spawn(1, OwnerToken::new(10), Ownership::ForegroundWait, IndependentEffort::new(5))
            .unwrap();
        let b = d
            .spawn(1, OwnerToken::new(20), Ownership::BackgroundOwned, IndependentEffort::new(5))
            .unwrap();
        d.steer(a.child, OwnerToken::new(10), EffectKind::ReadOnly).unwrap();
        assert_eq!(d.state(a.child), Some(ChildState::Running));
        assert_eq!(d.state(b.child), Some(ChildState::Running));
        assert_eq!(d.steer(a.child, OwnerToken::new(20), EffectKind::ReadOnly), Err(DelegError::NotOwner));
        // b untouched by a's steer: still no terminal transition
        assert_eq!(d.state(b.child), Some(ChildState::Running));
    }

    #[test]
    fn background_cancel_reclaims() {
        let mut d = Delegations::with_defaults();
        let h = d
            .spawn(7, OwnerToken::new(42), Ownership::BackgroundOwned, IndependentEffort::new(3))
            .unwrap();
        assert_eq!(d.live_count(), 1);
        assert_eq!(d.cancel(h.child, OwnerToken::new(42)), Ok(ChildState::Cancelled));
        assert!(d.task_joined(h.child));
        assert_eq!(d.live_count(), 0);
        // idempotent second cancel
        assert_eq!(d.cancel(h.child, OwnerToken::new(42)), Ok(ChildState::Cancelled));
    }

    #[test]
    fn crash_never_replays_ambiguous() {
        let mut d = Delegations::with_defaults();
        let h = d
            .spawn(9, OwnerToken::new(1), Ownership::BackgroundOwned, IndependentEffort::new(2))
            .unwrap();
        let out = d.crash(h.child, EffectKind::Ambiguous).unwrap();
        assert!(!out.replay_allowed);
        assert!(out.reclaimed);
        assert!(d.task_joined(h.child));
        assert_eq!(d.state(h.child), Some(ChildState::Crashed));
        // contrast: readonly crash may allow retry
        let h2 = d
            .spawn(9, OwnerToken::new(1), Ownership::ForegroundWait, IndependentEffort::new(2))
            .unwrap();
        let out2 = d.crash(h2.child, EffectKind::ReadOnly).unwrap();
        assert!(out2.replay_allowed);
    }

    #[test]
    fn ownership_and_effort_markers() {
        assert!(Ownership::ForegroundWait.parent_waits());
        assert!(!Ownership::BackgroundOwned.parent_waits());
        assert!(IndependentEffort::new(4).is_independent());
        assert_eq!(classify(EffectKind::Ambiguous), RetryDecision::NeverReplay);
        assert_eq!(classify(EffectKind::ReadOnly), RetryDecision::RetryAllowed);
        assert_eq!(classify(EffectKind::IdempotentWrite), RetryDecision::RetryAllowed);
    }
}
