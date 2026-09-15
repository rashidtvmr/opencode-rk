//! Enterprise ledger entry validation: bounded key appends.
//!
//! Distinct from the server `remote_ledger`: pure validation only, no I/O.

/// Maximum keys accepted by [`append_key`].
pub const MAX_LEDGER_KEYS: usize = 256;

/// Single validated ledger entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntry {
    /// Entry key (non-empty).
    pub key: String,
    /// Sequence number of the entry.
    pub seq: u64,
}

/// Ledger entry validation failure.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LedgerEntryError {
    /// Caller passed an empty key.
    #[error("ledger entry key must not be empty")]
    EmptyKey,
    /// Caller exceeded [`MAX_LEDGER_KEYS`].
    #[error("too many ledger keys: max {max}, actual {actual}")]
    TooMany { max: usize, actual: usize },
}

/// Append `key` to `keys` with validation.
///
/// Rules: empty -> [`LedgerEntryError::EmptyKey`]; `keys.len() >=
/// MAX_LEDGER_KEYS` -> [`LedgerEntryError::TooMany`]; duplicate key is
/// idempotent (ok, no push).
pub fn append_key(keys: &mut Vec<String>, key: &str) -> Result<(), LedgerEntryError> {
    if key.is_empty() {
        return Err(LedgerEntryError::EmptyKey);
    }
    if keys.iter().any(|k| k == key) {
        return Ok(());
    }
    if keys.len() >= MAX_LEDGER_KEYS {
        return Err(LedgerEntryError::TooMany {
            max: MAX_LEDGER_KEYS,
            actual: keys.len(),
        });
    }
    keys.push(key.to_string());
    Ok(())
}
