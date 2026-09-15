//! OPS-007 bounded small-machine admission guard.
//!
//! Pure O(1) policy over caller-owned counters. No I/O, no threads, no
//! allocation, no retained state. The caller owns the [`Admission`] lifetime;
//! failed checks mutate nothing. Defaults mirror
//! `config/resource-targets.json` (`activeGenerationSlots: 4`,
//! `maxQueuedAgents: 256`, `maxInputQueueBytesPerSession: 1048576`,
//! `maxInMemoryPreviewBytesPerTool: 65536`).
use thiserror::Error;

/// Published caps for one admission decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpsBudget {
    pub max_live_tasks: usize,
    pub max_queued: usize,
    pub max_input_bytes: u64,
    pub max_preview_bytes: u64,
}

impl Default for OpsBudget {
    fn default() -> Self {
        Self {
            max_live_tasks: 4,
            max_queued: 256,
            max_input_bytes: 1_048_576,
            max_preview_bytes: 65_536,
        }
    }
}

/// Caller-owned counters for admitted work. No lifetime held here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Admission {
    pub live_tasks: u32,
    pub queued: u32,
    pub reserved_bytes: u64,
}

/// Refusal or misconfiguration. Counters only, never paths or bodies.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BudgetError {
    #[error("budget limit {0} must be non-zero")]
    InvalidLimit(&'static str),
    #[error("over cap: requested {requested}, available {available}")]
    OverCap { requested: u64, available: u64 },
}

impl OpsBudget {
    /// Reject any zero cap. Deterministic, side-effect free.
    pub fn validate(&self) -> Result<(), BudgetError> {
        if self.max_live_tasks == 0 {
            return Err(BudgetError::InvalidLimit("max_live_tasks"));
        }
        if self.max_queued == 0 {
            return Err(BudgetError::InvalidLimit("max_queued"));
        }
        if self.max_input_bytes == 0 {
            return Err(BudgetError::InvalidLimit("max_input_bytes"));
        }
        if self.max_preview_bytes == 0 {
            return Err(BudgetError::InvalidLimit("max_preview_bytes"));
        }
        Ok(())
    }

    /// Pure check, no mutation. Admits iff `live < max_live_tasks`,
    /// `queued < max_queued`, `bytes <= max_input_bytes`.
    /// Overflow on `live + 1` / `queued + 1` can never admit.
    pub fn admit(&self, live: u32, queued: u32, bytes: u64) -> Result<Admission, BudgetError> {
        self.validate()?;
        let max_live = self.max_live_tasks as u64;
        let max_queued = self.max_queued as u64;
        if (live as u64) >= max_live {
            return Err(BudgetError::OverCap {
                requested: (live as u64).saturating_add(1),
                available: max_live,
            });
        }
        if (queued as u64) >= max_queued {
            return Err(BudgetError::OverCap {
                requested: (queued as u64).saturating_add(1),
                available: max_queued,
            });
        }
        if bytes > self.max_input_bytes {
            return Err(BudgetError::OverCap {
                requested: bytes,
                available: self.max_input_bytes,
            });
        }
        Ok(Admission {
            live_tasks: live,
            queued,
            reserved_bytes: bytes,
        })
    }

    /// Admits iff `bytes <= max_preview_bytes`; returns granted bytes unchanged.
    pub fn reserve_preview(&self, bytes: u64) -> Result<u64, BudgetError> {
        self.validate_preview()?;
        if bytes > self.max_preview_bytes {
            return Err(BudgetError::OverCap {
                requested: bytes,
                available: self.max_preview_bytes,
            });
        }
        Ok(bytes)
    }

    /// Preview path validates its own gate first, then falls back to full
    /// validation so a zeroed unrelated cap still refuses deterministically.
    /// Field order keeps the preview cap authoritative for preview verdicts.
    fn validate_preview(&self) -> Result<(), BudgetError> {
        if self.max_preview_bytes == 0 {
            return Err(BudgetError::InvalidLimit("max_preview_bytes"));
        }
        self.validate()
    }
}
