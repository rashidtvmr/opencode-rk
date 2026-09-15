//! SHARE-005 deterministic share-sync transport policy (fallback lane).
//!
//! Pure: no network, no DB mutation, no clock, no thread/timer, no global
//! state. Caller owns batches and schedules retries arithmetically from
//! `backoff_ms`. Policy inspects status codes and sequence numbers only;
//! share secrets never enter this module (no secret-typed parameters), and
//! `Debug` impls render keys/counts/codes only, never record-payload bytes.
//!
//! Upstream shape: `packages/opencode/src/share/share-next.ts:206-359` at
//! pinned commit `95daf90` (create/delete require success; sync drops the
//! batch on HTTP >= 400). Deliberate deviation: failed flushes are retained
//! by the caller for bounded retry, never dropped.
#![forbid(unsafe_code)]

use std::fmt;

/// Opaque share identifier. Never renders its bytes via `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct ShareId(Vec<u8>);

impl ShareId {
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self(id.as_bytes().to_vec())
    }

    fn as_lossy(&self) -> String {
        String::from_utf8_lossy(&self.0).into_owned()
    }
}

impl fmt::Debug for ShareId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShareId").finish_non_exhaustive()
    }
}

/// One coalesced share record. `Debug` renders key + payload length only,
/// never payload bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct ShareRecord {
    pub key: String,
    pub payload: Vec<u8>,
}

impl fmt::Debug for ShareRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShareRecord")
            .field("key", &self.key)
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

/// Drained coalesced batch. `Debug` renders seq + per-record key/len only.
#[derive(Clone, PartialEq, Eq)]
pub struct SyncBatch {
    pub share_id: ShareId,
    pub base_seq: u64,
    pub records: Vec<ShareRecord>,
}

impl fmt::Debug for SyncBatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncBatch")
            .field("share_id", &self.share_id)
            .field("base_seq", &self.base_seq)
            .field("records", &self.records)
            .finish()
    }
}

/// Sync transport decision. `Retry` carries an arithmetic backoff only; the
/// caller schedules. `Abort` carries a machine-readable reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
    Sent,
    Retry { backoff_ms: u64 },
    Abort { reason: String },
}

/// Delete transport decision. `Refused` is the abort-equivalent terminal:
/// no delete is claimed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted,
    NotFound,
    Retry { backoff_ms: u64 },
    Refused { reason: String },
}

/// Explicit remote route surface. Never defaulted: a missing endpoint is an
/// `Abort`, never an implicit target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endpoint {
    Legacy,
    Org,
}

impl Endpoint {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Org => "org",
        }
    }
}

/// Bounded retry policy: two `u64` seq markers live with the caller, one
/// `u32` attempt counter per decision, no per-record retention here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncPolicy {
    pub max_retries: u32,
    pub base_backoff_ms: u64,
}

impl Default for SyncPolicy {
    fn default() -> Self {
        Self::new(3, 500)
    }
}

impl SyncPolicy {
    #[must_use]
    pub fn new(max_retries: u32, base_backoff_ms: u64) -> Self {
        Self {
            max_retries,
            base_backoff_ms,
        }
    }

    /// 2xx => `Sent`; 429/5xx => bounded `Retry`, then `Abort` once
    /// `attempt >= max_retries`; any other status (including other 4xx and
    /// 1xx/3xx) => `Abort`.
    #[must_use]
    pub fn classify(&self, status: u16, attempt: u32) -> SyncOutcome {
        if (200..=299).contains(&status) {
            return SyncOutcome::Sent;
        }
        if status == 429 || (500..=599).contains(&status) {
            if attempt >= self.max_retries {
                return SyncOutcome::Abort {
                    reason: format!("retry-exhausted-{status}"),
                };
            }
            return SyncOutcome::Retry {
                backoff_ms: backoff(self.base_backoff_ms, attempt),
            };
        }
        SyncOutcome::Abort {
            reason: format!("client-error-{status}"),
        }
    }
}

/// `base * 2^attempt` capped at 30_000. Saturating: no overflow, no panic.
fn backoff(base_ms: u64, attempt: u32) -> u64 {
    const CAP_MS: u64 = 30_000;
    let shift = attempt.min(31);
    base_ms
        .saturating_mul(1u64 << shift)
        .min(CAP_MS)
}

/// At-most-once per sequence number: send only when the batch starts past
/// the last acked seq. A replayed or duplicate seq never resends.
#[must_use]
pub fn should_send(last_acked_seq: u64, base_seq: u64) -> bool {
    base_seq > last_acked_seq
}

/// Explicit endpoint selection. `None` constructs no request and sends zero
/// bytes: `Abort { reason: "endpoint-unspecified" }`.
pub fn request_target(endpoint: Option<Endpoint>, id: &ShareId) -> Result<String, SyncOutcome> {
    match endpoint {
        Some(Endpoint::Legacy) => Ok(format!("/api/share/{}", id.as_lossy())),
        Some(Endpoint::Org) => Ok(format!("/api/shares/{}", id.as_lossy())),
        None => Err(SyncOutcome::Abort {
            reason: "endpoint-unspecified".to_owned(),
        }),
    }
}

/// Delete confirmation: 2xx => `Deleted`; 404 => terminal `NotFound`
/// (idempotent delete); 429/5xx => `Retry` until exhausted, then terminal
/// `Refused`; any other 4xx => terminal `Refused` (no delete claimed).
#[must_use]
pub fn decide_delete(policy: &SyncPolicy, status: u16, attempt: u32) -> DeleteOutcome {
    if (200..=299).contains(&status) {
        return DeleteOutcome::Deleted;
    }
    if status == 404 {
        return DeleteOutcome::NotFound;
    }
    if status == 429 || (500..=599).contains(&status) {
        if attempt >= policy.max_retries {
            return DeleteOutcome::Refused {
                reason: format!("retry-exhausted-{status}"),
            };
        }
        return DeleteOutcome::Retry {
            backoff_ms: backoff(policy.base_backoff_ms, attempt),
        };
    }
    DeleteOutcome::Refused {
        reason: format!("client-error-{status}"),
    }
}

/// Create confirmation: 2xx => `Sent` (caller may persist); anything else =>
/// `Abort` (caller must persist nothing / roll back the SHARE-004 record).
#[must_use]
pub fn decide_create(status: u16) -> SyncOutcome {
    if (200..=299).contains(&status) {
        SyncOutcome::Sent
    } else {
        SyncOutcome::Abort {
            reason: format!("create-failed-{status}"),
        }
    }
}

/// Status/seq/endpoint-only log line. Carries zero secret bytes and zero
/// record-payload bytes by construction: only scalars and the endpoint
/// label are rendered.
#[must_use]
pub fn log_event(status: u16, seq: u64, endpoint: Endpoint) -> String {
    format!(
        "share-sync status={status} seq={seq} endpoint={}",
        endpoint.label()
    )
}
