//! Bounded in-memory AUTO lease-budget table (AUTO-004).
//!
//! Owns no I/O, no threads, no global state. Capacity is hard-capped at
//! [`MAX_LEASES`] so the table cannot grow without bound.

use thiserror::Error;

/// Maximum leases retained in one [`LeaseTable`].
pub const MAX_LEASES: usize = 64;

/// One lease request: normalized (trimmed, lowercased) owner plus TTL.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseAsk {
    pub owner: String,
    pub ttl_secs: u64,
}

/// Rejection reasons for [`normalize_owner`] and [`LeaseTable::add`].
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum LeaseError {
    #[error("owner must not be empty")]
    EmptyOwner,
    #[error("ttl must be non-zero")]
    ZeroTtl,
    #[error("too many leases: max {max}, actual {actual}")]
    TooManyLeases { max: usize, actual: usize },
}

/// Bounded table of lease asks. Owners stored normalized.
#[derive(Clone, Debug, Default)]
pub struct LeaseTable {
    leases: Vec<LeaseAsk>,
}

impl LeaseTable {
    #[must_use]
    pub fn new() -> Self {
        Self { leases: Vec::new() }
    }

    pub fn add(&mut self, ask: LeaseAsk) -> Result<(), LeaseError> {
        let owner = normalize_owner(&ask.owner)?;
        if ask.ttl_secs == 0 {
            return Err(LeaseError::ZeroTtl);
        }
        if self.leases.len() >= MAX_LEASES {
            return Err(LeaseError::TooManyLeases {
                max: MAX_LEASES,
                actual: self.leases.len(),
            });
        }
        self.leases.push(LeaseAsk {
            owner,
            ttl_secs: ask.ttl_secs,
        });
        Ok(())
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.leases.len()
    }

    #[must_use]
    pub fn list(&self) -> &[LeaseAsk] {
        &self.leases
    }
}

/// Trim surrounding whitespace and lowercase. Empty after trim is rejected.
pub fn normalize_owner(s: &str) -> Result<String, LeaseError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(LeaseError::EmptyOwner);
    }
    Ok(trimmed.to_lowercase())
}
