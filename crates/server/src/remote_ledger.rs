//! Pure in-memory remote-sync ledger: tracks pushed refs, no network.
#![forbid(unsafe_code)]

use std::fmt;

/// A single pushed ref record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteEntry {
    pub ref_name: String,
    pub hash: String,
    pub pushed_at: u64,
}

/// Ledger failure states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    EmptyRef,
    BadHash,
    TooManyEntries { max: usize, actual: usize },
    UnknownRef { name: String },
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRef => write!(f, "empty ref name"),
            Self::BadHash => write!(f, "hash must be 40 lowercase hex chars"),
            Self::TooManyEntries { max, actual } => {
                write!(f, "too many entries: max {max}, actual {actual}")
            }
            Self::UnknownRef { name } => write!(f, "unknown ref: {name}"),
        }
    }
}

impl std::error::Error for LedgerError {}

/// Maximum entries retained by the ledger.
pub const MAX_LEDGER_ENTRIES: usize = 256;

/// In-memory ledger of pushed refs, in insertion order.
#[derive(Debug, Default)]
pub struct RemoteLedger {
    entries: Vec<RemoteEntry>,
}

fn valid_hash(hash: &str) -> bool {
    hash.len() == 40
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl RemoteLedger {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Record a pushed ref. Same `ref_name` replaces in place (upsert, no dup).
    pub fn record(&mut self, e: RemoteEntry) -> Result<(), LedgerError> {
        if e.ref_name.is_empty() {
            return Err(LedgerError::EmptyRef);
        }
        if !valid_hash(&e.hash) {
            return Err(LedgerError::BadHash);
        }
        if let Some(slot) = self.entries.iter_mut().find(|x| x.ref_name == e.ref_name) {
            *slot = e;
            return Ok(());
        }
        if self.entries.len() >= MAX_LEDGER_ENTRIES {
            return Err(LedgerError::TooManyEntries {
                max: MAX_LEDGER_ENTRIES,
                actual: self.entries.len(),
            });
        }
        self.entries.push(e);
        Ok(())
    }

    /// Look up a pushed ref by name.
    pub fn get(&self, r: &str) -> Result<&RemoteEntry, LedgerError> {
        self.entries
            .iter()
            .find(|x| x.ref_name == r)
            .ok_or_else(|| LedgerError::UnknownRef {
                name: r.to_string(),
            })
    }

    /// All entries in insertion order.
    pub fn list(&self) -> &[RemoteEntry] {
        &self.entries
    }
}
