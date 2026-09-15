//! INT-005: credential refresh decision + single-flight gate.
//!
//! Pure in-memory boundary. [`needs_refresh`] decides whether a stored
//! credential is due for refresh (default 5-minute window). [`RefreshGate`]
//! admits at most one live [`RefreshGuard`] per credential id and at most
//! `max_inflight` guards total; guard completion is the only path that
//! records success or typed failure. No clock read, no I/O, no secret
//! material, no threads. Caller owns `now_ms`, the provider refresh
//! callback, persistence, events, and scheduling.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicU64, Ordering},
};

use thiserror::Error;

/// Default refresh window: 5 minutes, matching pinned `refreshWindowMinutes: 5`.
pub const REFRESH_WINDOW_MS: u64 = 300_000;
/// Default cap on concurrent in-flight refresh guards.
pub const MAX_INFLIGHT: usize = 128;
/// Maximum credential id length in bytes.
pub const MAX_CRED_BYTES: usize = 128;

static GATE_SEQ: AtomicU64 = AtomicU64::new(1);

/// Validated credential id: non-empty, at most 128 bytes.
///
/// Debug/secret policy: id bytes never render; every formatting shows the
/// fixed `cred-***` redaction. Use [`CredId::as_str`] only to key caller-side
/// maps, never to log.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CredId(String);

impl CredId {
    /// Validate and wrap a credential id.
    pub fn new(id: &str) -> Result<Self, RefreshError> {
        if id.is_empty() || id.len() > MAX_CRED_BYTES {
            return Err(RefreshError::InvalidCred);
        }
        Ok(Self(id.to_owned()))
    }

    /// Borrow the raw id for caller-side keying. Never log this.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for CredId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cred-***")
    }
}

/// Stored refresh state for one credential: ids plus two timestamps only.
#[derive(Clone, PartialEq, Eq)]
pub struct RefreshState {
    /// Which credential this state belongs to.
    pub credential_id: CredId,
    /// Expiry of the current credential, ms since epoch.
    pub expires_at_ms: u64,
    /// When the last successful refresh was recorded, ms since epoch.
    pub refreshed_at_ms: u64,
}

impl fmt::Debug for RefreshState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // credential_id renders as the fixed redaction via its own Debug.
        f.debug_struct("RefreshState")
            .field("credential_id", &self.credential_id)
            .field("expires_at_ms", &self.expires_at_ms)
            .field("refreshed_at_ms", &self.refreshed_at_ms)
            .finish()
    }
}

/// Typed refresh failure. Carries no secret bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefreshFail {
    /// Provider refused the refresh (bad request, revoked client, etc.).
    ProviderRejected,
    /// Network unavailable; retryable later by the caller.
    NetworkUnavailable,
    /// Grant itself expired; re-authorization required.
    ExpiredGrant,
}

impl RefreshFail {
    /// Typed error recorded for this failure reason.
    pub fn as_error(self) -> RefreshError {
        match self {
            Self::ProviderRejected => RefreshError::ProviderRejected,
            Self::NetworkUnavailable => RefreshError::NetworkUnavailable,
            Self::ExpiredGrant => RefreshError::ExpiredGrant,
        }
    }
}

/// Typed gate failure. Fixed strings only; never carries ids or secrets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum RefreshError {
    /// Credential id empty or over 128 bytes.
    #[error("invalid credential id")]
    InvalidCred,
    /// A refresh guard is already live for this credential; coalesce.
    #[error("refresh already in flight")]
    AlreadyRefreshing,
    /// Gate holds `max_inflight` live guards; nothing admitted.
    #[error("refresh gate full")]
    GateFull,
    /// No live guard for this credential; state unchanged.
    #[error("no in-flight refresh")]
    NoInflightRefresh,
    /// `new_expiry_ms <= now_ms`; state unchanged.
    #[error("invalid expiry")]
    InvalidExpiry,
    /// Refresh failed: provider rejected the request.
    #[error("provider rejected refresh")]
    ProviderRejected,
    /// Refresh failed: network unavailable.
    #[error("network unavailable")]
    NetworkUnavailable,
    /// Refresh failed: grant expired, re-authorization required.
    #[error("expired grant")]
    ExpiredGrant,
}

/// Pure decision: true iff `now_ms + window_ms >= state.expires_at_ms`.
///
/// Saturating add keeps the far-future `u64::MAX` expiry total without
/// wrapping. Reads no clock.
pub fn needs_refresh(state: &RefreshState, now_ms: u64, window_ms: u64) -> bool {
    now_ms.saturating_add(window_ms) >= state.expires_at_ms
}

struct GateInner {
    id: u64,
    max: usize,
    next_gen: u64,
    inflight: HashMap<String, u64>,
    records: HashMap<String, RefreshState>,
}

/// Single-flight gate: at most one live guard per credential, at most
/// `max_inflight` guards total. Empty gate holds two empty maps (no
/// allocation until first use).
#[derive(Clone)]
pub struct RefreshGate {
    inner: Arc<Mutex<GateInner>>,
}

impl RefreshGate {
    /// Gate admitting at most `max_inflight` concurrent guards.
    pub fn new(max_inflight: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(GateInner {
                id: GATE_SEQ.fetch_add(1, Ordering::Relaxed),
                max: max_inflight,
                next_gen: 0,
                inflight: HashMap::new(),
                records: HashMap::new(),
            })),
        }
    }

    /// Configured capacity bound.
    pub fn max_inflight(&self) -> usize {
        self.lock().max
    }

    /// Currently live guards.
    pub fn inflight(&self) -> usize {
        self.lock().inflight.len()
    }

    /// Last recorded success for a credential, if any.
    pub fn last(&self, cred: &str) -> Option<RefreshState> {
        let id = CredId::new(cred).ok()?;
        self.lock().records.get(id.as_str()).cloned()
    }

    /// Begin a refresh for `cred`. Second concurrent begin on the same id
    /// fails with [`RefreshError::AlreadyRefreshing`]; a full gate fails
    /// with [`RefreshError::GateFull`]. Nothing is admitted on failure.
    pub fn try_begin(&self, cred: &str) -> Result<RefreshGuard, RefreshError> {
        let id = CredId::new(cred)?;
        let mut inner = self.lock();
        if inner.inflight.contains_key(id.as_str()) {
            return Err(RefreshError::AlreadyRefreshing);
        }
        if inner.inflight.len() >= inner.max {
            return Err(RefreshError::GateFull);
        }
        let gen = inner.next_gen;
        inner.next_gen = inner.next_gen.wrapping_add(1);
        inner.inflight.insert(id.as_str().to_owned(), gen);
        Ok(RefreshGuard {
            cred: id.as_str().to_owned(),
            gen,
            gate_id: inner.id,
            slot: Arc::downgrade(&self.inner),
            done: false,
        })
    }

    /// Record success without a guard handle (e.g. duplicate completion).
    /// Unknown credential or wrong gate state yields
    /// [`RefreshError::NoInflightRefresh`]; stale expiry yields
    /// [`RefreshError::InvalidExpiry`]; neither records state.
    pub fn complete_for(
        &self,
        cred: &str,
        new_expiry_ms: u64,
        now_ms: u64,
    ) -> Result<RefreshState, RefreshError> {
        let id = CredId::new(cred)?;
        let mut inner = self.lock();
        if !inner.inflight.contains_key(id.as_str()) {
            return Err(RefreshError::NoInflightRefresh);
        }
        if new_expiry_ms <= now_ms {
            inner.inflight.remove(id.as_str());
            return Err(RefreshError::InvalidExpiry);
        }
        inner.inflight.remove(id.as_str());
        let recorded = RefreshState {
            credential_id: id.clone(),
            expires_at_ms: new_expiry_ms,
            refreshed_at_ms: now_ms,
        };
        inner.records.insert(id.as_str().to_owned(), recorded.clone());
        Ok(recorded)
    }

    /// Record typed failure without a guard handle. Unknown credential
    /// yields [`RefreshError::NoInflightRefresh`]; otherwise the slot is
    /// released and the typed reason is returned.
    pub fn fail_for(&self, cred: &str, reason: RefreshFail) -> RefreshError {
        let Ok(id) = CredId::new(cred) else {
            return RefreshError::InvalidCred;
        };
        let mut inner = self.lock();
        if inner.inflight.remove(id.as_str()).is_none() {
            return RefreshError::NoInflightRefresh;
        }
        reason.as_error()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, GateInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Default for RefreshGate {
    fn default() -> Self {
        Self::new(MAX_INFLIGHT)
    }
}

impl fmt::Debug for RefreshGate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = self.lock();
        let mut redacted: Vec<&str> = inner.inflight.keys().map(|_| "cred-***").collect();
        redacted.sort_unstable();
        f.debug_struct("RefreshGate")
            .field("inflight", &inner.inflight.len())
            .field("max_inflight", &inner.max)
            .field("entries", &redacted)
            .finish()
    }
}

/// Live single-flight refresh slot. Exactly one per credential per gate.
/// Completion (success or typed failure) is the only path that mutates gate
/// state; a guard dropped without completion reclaims its slot without
/// recording anything.
pub struct RefreshGuard {
    cred: String,
    gen: u64,
    gate_id: u64,
    slot: Weak<Mutex<GateInner>>,
    done: bool,
}

impl RefreshGuard {
    /// Record success: stores the new expiry and releases the slot.
    /// `new_expiry_ms <= now_ms` releases the slot, records nothing, and
    /// returns [`RefreshError::InvalidExpiry`]. A guard presented to a
    /// different gate returns [`RefreshError::NoInflightRefresh`].
    pub fn complete(
        mut self,
        gate: &RefreshGate,
        new_expiry_ms: u64,
        now_ms: u64,
    ) -> Result<RefreshState, RefreshError> {
        self.done = true;
        let mut inner = gate.lock();
        if inner.id != self.gate_id {
            return Err(RefreshError::NoInflightRefresh);
        }
        match inner.inflight.get(self.cred.as_str()) {
            Some(gen) if *gen == self.gen => {}
            _ => return Err(RefreshError::NoInflightRefresh),
        }
        if new_expiry_ms <= now_ms {
            inner.inflight.remove(self.cred.as_str());
            return Err(RefreshError::InvalidExpiry);
        }
        inner.inflight.remove(self.cred.as_str());
        let recorded = RefreshState {
            credential_id: CredId(self.cred.clone()),
            expires_at_ms: new_expiry_ms,
            refreshed_at_ms: now_ms,
        };
        inner
            .records
            .insert(self.cred.clone(), recorded.clone());
        Ok(recorded)
    }

    /// Record typed failure and release the slot. A guard presented to a
    /// different gate records nothing and still returns the typed reason.
    pub fn fail(mut self, gate: &RefreshGate, reason: RefreshFail) -> RefreshError {
        self.done = true;
        let mut inner = gate.lock();
        if inner.id == self.gate_id {
            if let Some(gen) = inner.inflight.get(self.cred.as_str()) {
                if *gen == self.gen {
                    inner.inflight.remove(self.cred.as_str());
                }
            }
        }
        reason.as_error()
    }
}

impl Drop for RefreshGuard {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        if let Some(shared) = self.slot.upgrade() {
            if let Ok(mut inner) = shared.lock() {
                let stale = inner.inflight.get(self.cred.as_str()) == Some(&self.gen);
                if stale {
                    inner.inflight.remove(self.cred.as_str());
                }
            }
        }
    }
}

impl fmt::Debug for RefreshGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RefreshGuard")
            .field("credential", &"cred-***")
            .field("done", &self.done)
            .finish()
    }
}
