//! Skill/command namespacing boundary (EXT-009).
//!
//! Bounded, purely in-memory registry of caller-supplied `(kind, name)`
//! declarations keyed by an explicit caller-owned source order. Later
//! sources win on name collision; Skill and Command namespaces are
//! independent. No filesystem traversal, no remote discovery, no policy
//! gating, no handler execution, no threads, no I/O, no wall-clock.

use thiserror::Error;

/// Maximum number of entries across both kinds.
pub const MAX_NAMES: usize = 512;
/// Maximum name length in bytes (names are ASCII-restricted, so bytes == chars).
pub const MAX_NAME_LEN: usize = 64;

/// Which namespace an entry belongs to. Same string in both kinds coexists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Skill,
    Command,
}

/// One caller-supplied name declaration. `source` is a caller-owned order
/// key; larger values win on collision for the same `(kind, name)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameDecl {
    pub kind: EntryKind,
    pub name: String,
    pub source: u32,
}

/// Namespacing failures. Every error leaves the registry unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum NamespaceError {
    #[error("invalid name")]
    InvalidName,
    #[error("registry full")]
    Overflow,
}

/// Bounded in-memory `(kind, name)` registry. Caller owns the lifetime and
/// the source ids; all methods are synchronous and spawn nothing.
#[derive(Debug, Default)]
pub struct Namespace {
    entries: Vec<NameDecl>,
}

fn name_valid(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_NAME_LEN {
        return false;
    }
    if !bytes[0].is_ascii_alphanumeric() {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || *b == b'.' || *b == b'_' || *b == b'-')
}

impl Namespace {
    /// Empty registry. No I/O, no threads.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register a declaration. New `(kind, name)` inserts and returns
    /// `Ok(true)`. Same `(kind, name)` with `source >= existing.source`
    /// replaces and returns `Ok(false)`; with a smaller source the
    /// existing entry is kept and `Ok(false)` is returned. A new entry
    /// beyond [`MAX_NAMES`] returns `Err(Overflow)` with no mutation.
    /// Invalid names return `Err(InvalidName)` with no mutation.
    pub fn register(&mut self, decl: NameDecl) -> Result<bool, NamespaceError> {
        if !name_valid(&decl.name) {
            return Err(NamespaceError::InvalidName);
        }
        if let Some(pos) = self
            .entries
            .iter()
            .position(|e| e.kind == decl.kind && e.name == decl.name)
        {
            if decl.source >= self.entries[pos].source {
                self.entries[pos] = decl;
            }
            return Ok(false);
        }
        if self.entries.len() >= MAX_NAMES {
            return Err(NamespaceError::Overflow);
        }
        self.entries.push(decl);
        Ok(true)
    }

    /// Remove exactly `(kind, name)`. Unknown entries return `false` and
    /// change nothing.
    pub fn unregister(&mut self, kind: EntryKind, name: &str) -> bool {
        if let Some(pos) = self
            .entries
            .iter()
            .position(|e| e.kind == kind && e.name == name)
        {
            self.entries.swap_remove(pos);
            true
        } else {
            false
        }
    }

    /// Borrow the declaration for `(kind, name)`, if present. No state change.
    pub fn lookup(&self, kind: EntryKind, name: &str) -> Option<&NameDecl> {
        self.entries
            .iter()
            .find(|e| e.kind == kind && e.name == name)
    }

    /// Borrowed refs for one kind, sorted by name ascending. Deterministic.
    pub fn list(&self, kind: EntryKind) -> Vec<&NameDecl> {
        let mut out: Vec<&NameDecl> = self.entries.iter().filter(|e| e.kind == kind).collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }
}
