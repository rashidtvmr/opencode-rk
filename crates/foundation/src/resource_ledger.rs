//! OPS-001 bounded resource ledger plus disabled-cost attestation.
//!
//! Pure admission accounting (no I/O, no clock, no threads) composed from the
//! BASE-003 precedents (`ByteBudget` reserve-before-admit, RAII release).
//! Defaults mirror `config/resource-targets.json` (`maxQueuedAgents: 256`,
//! `maxInputQueueBytesPerSession: 1048576`,
//! `maxInMemoryPreviewBytesPerTool: 65536`,
//! `maxInMemorySubscriberBytes: 262144`).
//!
//! Per-kind bound mapping (every kind enforces BOTH a count and a byte cap):
//!
//! | kind | count cap | byte cap (ledger total) |
//! |---|---|---|
//! | `AgentSlot` | `max_queued_agents` | `max_queued_agents * max_preview_bytes_per_tool` (one preview-sized control record per queued agent) |
//! | `SessionInput` | `max_queued_agents` | `max_input_bytes_per_session` |
//! | `ToolPreview` | `max_queued_agents` | `max_preview_bytes_per_tool` |
//! | `SubscriberBuffer` | `max_queued_agents` | `max_subscriber_bytes` |
//!
//! `admit` checks the count cap first, then the byte cap. A failed admit
//! charges nothing. `Reservation` releases its slot+bytes on `cancel()` or
//! drop; `cancel()` is idempotent and saturates at zero.
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use thiserror::Error;

/// Published caps. `Default` loads the four `config/resource-targets.json`
/// targets. Any zero field is rejected at construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionCaps {
    pub max_queued_agents: u32,
    pub max_input_bytes_per_session: u64,
    pub max_preview_bytes_per_tool: u64,
    pub max_subscriber_bytes: u64,
}

impl Default for AdmissionCaps {
    fn default() -> Self {
        Self {
            max_queued_agents: 256,
            max_input_bytes_per_session: 1_048_576,
            max_preview_bytes_per_tool: 65_536,
            max_subscriber_bytes: 262_144,
        }
    }
}

impl AdmissionCaps {
    /// Reject any zero field; the ledger is unusable until fixed, never
    /// silently unbounded.
    pub fn validate(&self) -> Result<(), LedgerError> {
        if self.max_queued_agents == 0 {
            return Err(LedgerError::InvalidCap("max_queued_agents"));
        }
        if self.max_input_bytes_per_session == 0 {
            return Err(LedgerError::InvalidCap("max_input_bytes_per_session"));
        }
        if self.max_preview_bytes_per_tool == 0 {
            return Err(LedgerError::InvalidCap("max_preview_bytes_per_tool"));
        }
        if self.max_subscriber_bytes == 0 {
            return Err(LedgerError::InvalidCap("max_subscriber_bytes"));
        }
        Ok(())
    }
}

/// Admission lane. Each lane carries its own slot+byte counters.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum WorkKind {
    AgentSlot,
    SessionInput,
    ToolPreview,
    SubscriberBuffer,
}

impl WorkKind {
    fn index(self) -> usize {
        match self {
            WorkKind::AgentSlot => 0,
            WorkKind::SessionInput => 1,
            WorkKind::ToolPreview => 2,
            WorkKind::SubscriberBuffer => 3,
        }
    }
}

/// Refusal. Counters only, never paths or bodies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OverBudget {
    SlotsExhausted { kind: WorkKind },
    BytesExhausted {
        kind: WorkKind,
        requested: u64,
        available: u64,
    },
}

/// Construction / spill failures. Counters and names only, never bodies.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum LedgerError {
    #[error("ledger cap {0} must be non-zero")]
    InvalidCap(&'static str),
    #[error("over budget: {0:?}")]
    Over(OverBudget),
    #[error("preview fits within budget, no spill needed")]
    NoSpillNeeded,
    #[error("spill write failed: {0}")]
    Io(String),
}

impl From<OverBudget> for LedgerError {
    fn from(v: OverBudget) -> Self {
        LedgerError::Over(v)
    }
}

/// Zero-cost proof for one optional subsystem left off.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisabledCostAttestation {
    pub tasks: u32,
    pub threads: u32,
    pub stores: u32,
    pub watchers: u32,
    pub bytes: u64,
}

impl DisabledCostAttestation {
    pub const ZERO: Self = Self {
        tasks: 0,
        threads: 0,
        stores: 0,
        watchers: 0,
        bytes: 0,
    };

    #[must_use]
    pub fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }
}

/// What one optional subsystem (plugin host, JS host, file watcher, LSP
/// service, optional web) would start given its `Features` flag. `enabled`
/// mirrors the flag: off means the subsystem must report zero cost and
/// perform zero I/O.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubsystemProbe {
    name: &'static str,
    enabled: bool,
    cost: DisabledCostAttestation,
    io_ops: u64,
}

impl SubsystemProbe {
    /// Flag off: zero cost, zero I/O by construction.
    #[must_use]
    pub fn disabled(name: &'static str) -> Self {
        Self {
            name,
            enabled: false,
            cost: DisabledCostAttestation::ZERO,
            io_ops: 0,
        }
    }

    /// Flag on with the exact resources the subsystem would start.
    #[must_use]
    pub fn enabled(
        name: &'static str,
        tasks: u32,
        threads: u32,
        stores: u32,
        watchers: u32,
        bytes: u64,
    ) -> Self {
        let cost = DisabledCostAttestation {
            tasks,
            threads,
            stores,
            watchers,
            bytes,
        };
        let io_ops = tasks as u64 + threads as u64 + stores as u64 + watchers as u64;
        Self {
            name,
            enabled: true,
            cost,
            io_ops,
        }
    }

    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Flag off always reports zero; flag on reports the declared cost.
    /// A disabled probe reporting nonzero cost is a leak the attestation
    /// functions refuse to zero out.
    #[must_use]
    pub fn report(&self) -> DisabledCostAttestation {
        if self.enabled {
            self.cost
        } else {
            DisabledCostAttestation::ZERO
        }
    }

    /// Fixture-FS handles opened by this probe. Disabled probes open none.
    #[must_use]
    pub fn io_ops(&self) -> u64 {
        if self.enabled {
            self.io_ops
        } else {
            0
        }
    }
}

/// Attestation failure. Never a zeroed lie.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum AttestError {
    #[error("cost leak in disabled subsystem {subsystem}")]
    CostLeak { subsystem: &'static str },
    #[error("isolation violated: expected exactly one enabled subsystem")]
    Isolation,
}

/// All-off input yields all-zero output. Any enabled probe, or any disabled
/// probe whose report is nonzero, is `CostLeak`, never a zeroed summary.
pub fn attest_disabled(probes: &[SubsystemProbe]) -> Result<DisabledCostAttestation, AttestError> {
    for p in probes {
        if p.enabled || p.report() != DisabledCostAttestation::ZERO {
            return Err(AttestError::CostLeak { subsystem: p.name });
        }
    }
    Ok(DisabledCostAttestation::ZERO)
}

/// Exactly one subsystem enabled: returns only its report and proves every
/// other probe still reports zero. Zero or several enabled is `Isolation`;
/// a nonzero report from a disabled probe is `CostLeak`.
pub fn attest_single_enabled(
    probes: &[SubsystemProbe],
    name: &str,
) -> Result<DisabledCostAttestation, AttestError> {
    let mut found = None;
    for p in probes {
        if p.enabled {
            if p.name != name || found.is_some() {
                return Err(AttestError::Isolation);
            }
            found = Some(p.cost);
        } else if p.report() != DisabledCostAttestation::ZERO || p.io_ops() != 0 {
            return Err(AttestError::CostLeak { subsystem: p.name });
        }
    }
    found.ok_or(AttestError::Isolation)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct KindUsage {
    slots: u64,
    bytes: u64,
}

#[derive(Clone, Debug)]
struct Inner {
    used: [KindUsage; 4],
}

/// Pure admission ledger: no I/O, no clock, no spawned task. The caller owns
/// the ledger lifetime; `Reservation`s borrow it via `Arc` and release on
/// drop. Deterministic: same caps plus same admit/release sequence yields
/// identical state.
#[derive(Clone, Debug)]
pub struct BudgetLedger {
    caps: AdmissionCaps,
    state: Arc<Mutex<Inner>>,
    spills: Arc<AtomicU64>,
}

impl PartialEq for BudgetLedger {
    fn eq(&self, other: &Self) -> bool {
        self.caps == other.caps && self.snapshot() == other.snapshot()
    }
}

impl Eq for BudgetLedger {}

impl BudgetLedger {
    pub fn new(caps: AdmissionCaps) -> Result<Self, LedgerError> {
        caps.validate()?;
        Ok(Self {
            caps,
            state: Arc::new(Mutex::new(Inner {
                used: [KindUsage::default(); 4],
            })),
            spills: Arc::new(AtomicU64::new(0)),
        })
    }

    fn count_cap(&self, kind: WorkKind) -> u64 {
        let _ = kind;
        self.caps.max_queued_agents as u64
    }

    fn byte_cap(&self, kind: WorkKind) -> u64 {
        match kind {
            WorkKind::AgentSlot => {
                (self.caps.max_queued_agents as u64)
                    .saturating_mul(self.caps.max_preview_bytes_per_tool)
            }
            WorkKind::SessionInput => self.caps.max_input_bytes_per_session,
            WorkKind::ToolPreview => self.caps.max_preview_bytes_per_tool,
            WorkKind::SubscriberBuffer => self.caps.max_subscriber_bytes,
        }
    }

    /// Reserve one slot plus `bytes` on `kind`. Reservations are recorded
    /// before work is admitted; a refusal charges nothing.
    pub fn admit(&self, kind: WorkKind, bytes: u64) -> Result<Reservation, OverBudget> {
        let mut state = self.state.lock().expect("ledger lock");
        let i = kind.index();
        if state.used[i].slots >= self.count_cap(kind) {
            return Err(OverBudget::SlotsExhausted { kind });
        }
        let available = self.byte_cap(kind).saturating_sub(state.used[i].bytes);
        if bytes > available {
            return Err(OverBudget::BytesExhausted {
                kind,
                requested: bytes,
                available,
            });
        }
        state.used[i].slots += 1;
        state.used[i].bytes += bytes;
        Ok(Reservation {
            state: Arc::clone(&self.state),
            kind,
            bytes,
            released: false,
        })
    }

    #[must_use]
    pub fn available_slots(&self, kind: WorkKind) -> u64 {
        let state = self.state.lock().expect("ledger lock");
        self.count_cap(kind).saturating_sub(state.used[kind.index()].slots)
    }

    #[must_use]
    pub fn available_bytes(&self, kind: WorkKind) -> u64 {
        let state = self.state.lock().expect("ledger lock");
        self.byte_cap(kind)
            .saturating_sub(state.used[kind.index()].bytes)
    }

    /// `(used_slots, used_bytes)` for one lane.
    #[must_use]
    pub fn usage(&self, kind: WorkKind) -> (u64, u64) {
        let state = self.state.lock().expect("ledger lock");
        let u = state.used[kind.index()];
        (u.slots, u.bytes)
    }

    /// Full per-lane state in fixed lane order
    /// (`AgentSlot`, `SessionInput`, `ToolPreview`, `SubscriberBuffer`).
    /// A failed admit leaves this bit-identical.
    #[must_use]
    pub fn snapshot(&self) -> Vec<(u64, u64)> {
        let state = self.state.lock().expect("ledger lock");
        state.used.iter().map(|u| (u.slots, u.bytes)).collect()
    }

    /// Over-budget tool preview: spill the original bytes per the explicit
    /// retention policy instead of silently deleting them. The spill file
    /// lands inside `dir` only; the log line records counters and the file
    /// name, never body bytes. Payloads within budget need no spill and are
    /// refused with `NoSpillNeeded` (call `admit` instead).
    pub fn spill_tool_preview(
        &self,
        bytes: &[u8],
        policy: RetentionPolicy,
        dir: &std::path::Path,
        log: &mut String,
    ) -> Result<Spilled, LedgerError> {
        let len = bytes.len() as u64;
        if len <= self.caps.max_preview_bytes_per_tool {
            return Err(LedgerError::NoSpillNeeded);
        }
        let RetentionPolicy::SpillToDisk = policy;
        std::fs::create_dir_all(dir).map_err(|e| LedgerError::Io(e.to_string()))?;
        let n = self.spills.fetch_add(1, Ordering::SeqCst);
        let path = dir.join(format!("spill-{n}.bin"));
        if !path.starts_with(dir) {
            return Err(LedgerError::Io("spill path escaped fixture dir".to_string()));
        }
        std::fs::write(&path, bytes).map_err(|e| LedgerError::Io(e.to_string()))?;
        log.push_str(&format!(
            "spilled preview: bytes={len} policy=spill-to-disk file=spill-{n}.bin\n"
        ));
        Ok(Spilled {
            bytes: len,
            policy,
            path,
        })
    }
}

/// RAII charge: releases its slot+bytes on `cancel()` or drop.
#[derive(Debug)]
pub struct Reservation {
    state: Arc<Mutex<Inner>>,
    kind: WorkKind,
    bytes: u64,
    released: bool,
}

impl Reservation {
    fn release_locked(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        let mut state = self.state.lock().expect("ledger lock");
        let u = &mut state.used[self.kind.index()];
        u.slots = u.slots.saturating_sub(1);
        u.bytes = u.bytes.saturating_sub(self.bytes);
    }

    /// Release early. Idempotent: second call is a no-op and usage
    /// saturates at zero, never underflows.
    pub fn cancel(&mut self) {
        self.release_locked();
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        self.release_locked();
    }
}

/// Explicit retention policy for over-budget content. Silent deletion is
/// forbidden: content leaves memory only with a `Spilled` record pointing
/// at recoverable bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetentionPolicy {
    SpillToDisk,
}

/// Spill record: how many original bytes, under which policy, recoverable
/// from `path` (always inside the caller-provided fixture dir).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Spilled {
    pub bytes: u64,
    pub policy: RetentionPolicy,
    pub path: PathBuf,
}
