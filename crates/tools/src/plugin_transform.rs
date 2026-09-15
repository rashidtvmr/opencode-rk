//! Plugin config-transform replay/disablement boundary (EXT-011).
//!
//! Bounded, purely in-memory record of caller-supplied `(scope, key, value)`
//! config transforms with deterministic replay projection. A scope can be
//! disabled (its transforms stop projecting while staying recorded) and
//! closed (its transforms are removed, revealing prior scope values for the
//! same key). No service is mutated, no host started, no filesystem or
//! network touched. No threads, no I/O, no wall-clock. Std only.

use std::error::Error;
use std::fmt;

/// Maximum transforms retained by one log.
pub const EXT11_MAX_TRANSFORMS: usize = 512;
/// Maximum key length in bytes (keys are ASCII-restricted, so bytes == chars).
pub const EXT11_MAX_KEY_LEN: usize = 128;
/// Maximum value length in bytes.
pub const EXT11_MAX_VALUE_LEN: usize = 1024;

/// One recorded config transform. `seq` is assigned from 1 in insertion
/// order; `disabled_scope` mirrors the current disabled state of `scope`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transform {
    pub scope: u64,
    pub key: String,
    pub value: String,
    pub seq: u64,
    pub disabled_scope: bool,
}

/// Transform-record failures. Every error leaves the log unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformError {
    InvalidKey,
    InvalidValue,
    Duplicate,
    Overflow,
}

impl fmt::Display for TransformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransformError::InvalidKey => write!(f, "invalid key"),
            TransformError::InvalidValue => write!(f, "invalid value"),
            TransformError::Duplicate => write!(f, "duplicate key in scope"),
            TransformError::Overflow => write!(f, "transform log full"),
        }
    }
}

impl Error for TransformError {}

fn key_valid(key: &str) -> bool {
    let bytes = key.as_bytes();
    if bytes.is_empty() || bytes.len() > EXT11_MAX_KEY_LEN {
        return false;
    }
    if !bytes[0].is_ascii_alphanumeric() {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn value_valid(value: &str) -> bool {
    value.len() <= EXT11_MAX_VALUE_LEN && !value.contains('\0')
}

/// Bounded in-memory transform log. Caller owns the lifetime and the scope
/// ids; all methods are synchronous and spawn nothing.
#[derive(Debug, Default)]
pub struct TransformLog {
    entries: Vec<Transform>,
    disabled: Vec<u64>,
    next: u64,
}

impl TransformLog {
    /// Empty log: allocates only the empty vec, no threads or I/O.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            disabled: Vec::new(),
            next: 1,
        }
    }

    /// Validate and record a transform, returning its monotonic `seq`.
    /// Rejects bad key/value, per-scope duplicate keys, and overflow;
    /// any rejection leaves the log unchanged and consumes no `seq`.
    pub fn add(&mut self, scope: u64, key: &str, value: &str) -> Result<u64, TransformError> {
        if !key_valid(key) {
            return Err(TransformError::InvalidKey);
        }
        if !value_valid(value) {
            return Err(TransformError::InvalidValue);
        }
        if self.entries.iter().any(|t| t.scope == scope && t.key == key) {
            return Err(TransformError::Duplicate);
        }
        if self.entries.len() >= EXT11_MAX_TRANSFORMS {
            return Err(TransformError::Overflow);
        }
        let seq = self.next;
        self.next += 1;
        let suppressed = self.disabled.contains(&scope);
        self.entries.push(Transform {
            scope,
            key: key.to_string(),
            value: value.to_string(),
            seq,
            disabled_scope: suppressed,
        });
        Ok(seq)
    }

    /// Disable (`true`) or re-enable (`false`) a scope: future
    /// [`TransformLog::project`] skips or includes that scope's transforms.
    /// The record is retained. Unknown scope is a harmless no-op.
    pub fn set_scope_disabled(&mut self, scope: u64, disabled: bool) {
        if disabled {
            if !self.disabled.contains(&scope) {
                self.disabled.push(scope);
            }
        } else {
            self.disabled.retain(|s| *s != scope);
        }
        for t in self.entries.iter_mut().filter(|t| t.scope == scope) {
            t.disabled_scope = disabled;
        }
    }

    /// Remove only `scope`'s transforms, returning the count removed.
    /// Unknown or already-closed scope returns 0 and changes nothing.
    pub fn close_scope(&mut self, scope: u64) -> u64 {
        let before = self.entries.len();
        self.entries.retain(|t| t.scope != scope);
        self.disabled.retain(|s| *s != scope);
        (before - self.entries.len()) as u64
    }

    /// Replay non-disabled transforms in `seq` order; later `seq` wins per
    /// key. Output sorted by key ascending. Pure: no state change.
    pub fn project(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for t in &self.entries {
            if t.disabled_scope {
                continue;
            }
            match out.iter_mut().find(|(k, _)| *k == t.key) {
                Some(slot) => slot.1.clone_from(&t.value),
                None => out.push((t.key.clone(), t.value.clone())),
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}
