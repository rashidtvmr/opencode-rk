//! Inert hook-declaration boundary (EXT-008).
//!
//! Records which plugin scope contributed which bounded before/after
//! tool-hook declaration, replays declarations in registration order, and
//! proves no handler executes: [`HookBoundary::try_execute`] always returns
//! [`HookError::Deferred`], gate closed or open. No I/O, no threads, no
//! imports, no tool input/output access.

use thiserror::Error;

/// Maximum retained hook declarations.
pub const MAX_HOOK_DECLS: usize = 128;
/// Maximum `tool_filter` length in bytes.
pub const MAX_FILTER_LEN: usize = 64;
/// Maximum `label` length in bytes.
pub const MAX_LABEL_LEN: usize = 64;

/// When a contributed hook would run relative to the tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookPhase {
    Before,
    After,
}

/// One contributed hook declaration: filter selects tools, label names the hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookDecl {
    pub phase: HookPhase,
    pub tool_filter: String,
    pub label: String,
}

/// Monotonic declaration id, assigned from 1 in registration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HookId(pub u64);

/// Boundary failures. All are inert: the registry is unchanged on error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HookError {
    #[error("invalid tool filter")]
    InvalidFilter,
    #[error("invalid label")]
    InvalidLabel,
    #[error("duplicate hook declaration")]
    Duplicate,
    #[error("hook registry full")]
    Overflow,
    #[error("execution deferred: no executor in this slice")]
    Deferred,
}

/// Explicit activation gate. Closed by default; opening starts nothing and
/// never imports or runs anything. Tests open it only to assert the boundary
/// still holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivationGate {
    closed: bool,
}

impl Default for ActivationGate {
    fn default() -> Self {
        Self::closed()
    }
}

impl ActivationGate {
    /// Closed gate: nothing may execute (nothing can execute in this slice).
    pub fn closed() -> Self {
        Self { closed: true }
    }

    /// Explicitly open the gate. Allocates nothing, starts nothing;
    /// [`HookBoundary::try_execute`] still defers while open.
    pub fn open(&mut self) {
        self.closed = false;
    }

    /// Whether the gate is closed.
    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

fn valid_filter(s: &str) -> bool {
    if s.is_empty() || s.len() > MAX_FILTER_LEN {
        return false;
    }
    s.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'*' | b'.' | b'_' | b'-'))
}

fn valid_label(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_LABEL_LEN {
        return false;
    }
    if !bytes[0].is_ascii_alphanumeric() {
        return false;
    }
    bytes
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// Pure in-memory registry of hook declarations. Caller owns the lifetime;
/// all methods are synchronous, spawn nothing, touch no I/O.
#[derive(Debug, Default)]
pub struct HookBoundary {
    gate: ActivationGate,
    entries: Vec<(HookId, u64, HookDecl)>,
    next_id: u64,
}

impl HookBoundary {
    /// Empty boundary with the gate closed. No I/O, no threads.
    pub fn new() -> Self {
        Self {
            gate: ActivationGate::closed(),
            entries: Vec::new(),
            next_id: 1,
        }
    }

    /// Whether the activation gate is currently closed.
    pub fn is_gate_closed(&self) -> bool {
        self.gate.is_closed()
    }

    /// Explicitly open the activation gate. Recording semantics are
    /// unchanged; execution stays deferred.
    pub fn open_gate(&mut self) {
        self.gate.open();
    }

    /// Record a declaration from `scope`. Validates filter/label, rejects
    /// duplicates (same scope+phase+filter+label) and overflow, assigns the
    /// next monotonic id. Works identically gate open or closed.
    pub fn declare(&mut self, scope: u64, decl: HookDecl) -> Result<HookId, HookError> {
        if !valid_filter(&decl.tool_filter) {
            return Err(HookError::InvalidFilter);
        }
        if !valid_label(&decl.label) {
            return Err(HookError::InvalidLabel);
        }
        if self.entries.iter().any(|(_, s, d)| {
            *s == scope
                && d.phase == decl.phase
                && d.tool_filter == decl.tool_filter
                && d.label == decl.label
        }) {
            return Err(HookError::Duplicate);
        }
        if self.entries.len() >= MAX_HOOK_DECLS {
            return Err(HookError::Overflow);
        }
        let id = HookId(self.next_id);
        self.next_id += 1;
        self.entries.push((id, scope, decl));
        Ok(id)
    }

    /// Remove only `scope`'s declarations; return count removed.
    /// Unknown scope returns 0 and changes nothing.
    pub fn revoke_scope(&mut self, scope: u64) -> u64 {
        let before = self.entries.len();
        self.entries.retain(|(_, s, _)| *s != scope);
        (before - self.entries.len()) as u64
    }

    /// All declarations sorted by id ascending (registration order).
    pub fn list(&self) -> Vec<(HookId, u64, HookDecl)> {
        let mut out = self.entries.clone();
        out.sort_by_key(|(id, _, _)| id.0);
        out
    }

    /// Pure projection of declarations in registration order. Performs zero calls.
    pub fn replay(&self) -> Vec<HookDecl> {
        self.list().into_iter().map(|(_, _, d)| d).collect()
    }

    /// Always [`HookError::Deferred`], gate closed or open, id known or
    /// unknown. No handler runs, no input/output touched, gate unchanged.
    pub fn try_execute(&self, _id: HookId) -> Result<(), HookError> {
        Err(HookError::Deferred)
    }
}
