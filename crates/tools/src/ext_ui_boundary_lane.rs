//! Inert UI-declaration boundary lane (EXT-012).
//!
//! Records which plugin scope contributed which bounded UI declaration
//! (command entry, panel slot, status item), replays declarations in
//! registration order, and proves no presentation executes:
//! [`UiBoundary::request_render`] always returns [`UiError::Deferred`],
//! gate closed or open. No I/O, no threads, no imports, no renderer.

use thiserror::Error;

/// Maximum retained UI declarations.
pub const MAX_UI_DECLS: usize = 64;
/// Maximum `label` length in bytes (labels are ASCII-only, so bytes == chars).
pub const MAX_LABEL_LEN: usize = 64;
/// Maximum `detail` length in bytes (details are ASCII-only, so bytes == chars).
pub const MAX_DETAIL_LEN: usize = 256;

/// Which bounded surface a declaration contributes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiSurface {
    Command,
    Panel,
    StatusItem,
}

/// One contributed UI declaration: surface, label, and detail text only.
/// Holds no tool I/O, no credentials, no renderer handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiDecl {
    pub surface: UiSurface,
    pub label: String,
    pub detail: String,
}

/// Boundary failures. All are inert: the registry is unchanged on error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UiError {
    #[error("invalid label")]
    InvalidLabel,
    #[error("invalid detail")]
    InvalidDetail,
    #[error("duplicate UI declaration")]
    Duplicate,
    #[error("UI registry full")]
    Overflow,
    #[error("presentation deferred: no renderer in this slice")]
    Deferred,
}

/// Explicit activation gate. Closed by default; opening starts nothing and
/// never renders or imports anything. Tests open it only to assert the
/// boundary still holds.
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
    /// Closed gate: nothing may present (nothing can present in this slice).
    pub fn closed() -> Self {
        Self { closed: true }
    }

    /// Explicitly open the gate. Allocates nothing, starts nothing;
    /// [`UiBoundary::request_render`] still defers while open.
    pub fn open(&mut self) {
        self.closed = false;
    }

    /// Whether the gate is closed.
    pub fn is_closed(&self) -> bool {
        self.closed
    }
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

fn valid_detail(s: &str) -> bool {
    if s.len() > MAX_DETAIL_LEN {
        return false;
    }
    // Printable ASCII only (0x20..=0x7E); rejects control chars, DEL, and
    // non-ASCII. Empty detail is allowed.
    s.bytes().all(|b| matches!(b, 0x20..=0x7E))
}

/// Pure in-memory registry of UI declarations. Caller owns the lifetime;
/// all methods are synchronous, spawn nothing, touch no I/O.
#[derive(Debug, Default)]
pub struct UiBoundary {
    gate: ActivationGate,
    entries: Vec<(u64, u64, UiDecl)>,
    next_id: u64,
}

impl UiBoundary {
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
    /// unchanged; presentation stays deferred.
    pub fn open_gate(&mut self) {
        self.gate.open();
    }

    /// Record a declaration from `scope`. Validates label/detail, rejects
    /// duplicates (same scope+surface+label) and overflow, assigns the next
    /// monotonic id from 1. Works identically gate open or closed
    /// (recording is not presentation). Failed calls consume no id.
    pub fn declare(&mut self, scope: u64, decl: UiDecl) -> Result<u64, UiError> {
        if !valid_label(&decl.label) {
            return Err(UiError::InvalidLabel);
        }
        if !valid_detail(&decl.detail) {
            return Err(UiError::InvalidDetail);
        }
        if self.entries.iter().any(|(_, s, d)| {
            *s == scope && d.surface == decl.surface && d.label == decl.label
        }) {
            return Err(UiError::Duplicate);
        }
        if self.entries.len() >= MAX_UI_DECLS {
            return Err(UiError::Overflow);
        }
        let id = self.next_id;
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

    /// All declarations sorted by id ascending (registration order,
    /// deterministic).
    pub fn list(&self) -> Vec<(u64, u64, UiDecl)> {
        let mut out = self.entries.clone();
        out.sort_by_key(|(id, _, _)| *id);
        out
    }

    /// Pure metadata projection for the future renderer; performs zero
    /// rendering. Unknown id returns `None`.
    pub fn describe(&self, id: u64) -> Option<(u64, UiDecl)> {
        self.entries
            .iter()
            .find(|(eid, _, _)| *eid == id)
            .map(|(_, scope, decl)| (*scope, decl.clone()))
    }

    /// Always [`UiError::Deferred`], gate closed or open, id known or
    /// unknown. No presentation runs, gate state unchanged.
    pub fn request_render(&self, _id: u64) -> Result<(), UiError> {
        Err(UiError::Deferred)
    }
}
