#![forbid(unsafe_code)]

//! Remote approval review with explicit human authority (NET-009).
//!
//! A paired phone (remote) reviews exact operation details and issues one
//! approve/deny decision. Account login is never authority: every decision is
//! bound to operation digest + device + workspace + requester + expiry +
//! policy version, decided exactly once (race = one winner), single-use
//! (replay invalid), and revocable. Local-only / human-only-direct operations
//! stay ineligible over the remote channel.
//!
//! Commit: 5af7884. Standalone module: std only, no crate deps.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Mutex;

/// Hard cap so pending-approval state cannot grow without bound.
pub const MAX_PENDING_APPROVALS: usize = 1024;
/// Max bytes for any id-like field (device / workspace / requester / request).
pub const MAX_FIELD_LEN: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Approve,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub id: String,
    pub op_digest: [u8; 32],
    pub device_id: String,
    pub workspace_id: String,
    pub requester: String,
    pub expires_at_ms: u64,
    /// Operation must run with local presence; remote cannot approve it.
    pub local_only: bool,
    /// Operation needs a directly-present human; remote cannot approve it.
    pub human_only: bool,
    /// Policy version the request was issued under.
    pub policy_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentedBinding {
    pub op_digest: [u8; 32],
    pub device_id: String,
    pub workspace_id: String,
    pub requester: String,
    pub expires_at_ms: u64,
    pub policy_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalReceipt {
    pub request_id: String,
    pub op_digest: [u8; 32],
    pub device_id: String,
    pub workspace_id: String,
    pub requester: String,
    pub expires_at_ms: u64,
    pub policy_version: u64,
    pub decision: Decision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalError {
    NotFound,
    AlreadyExists,
    AlreadyDecided,
    Expired,
    DigestMismatch,
    DeviceMismatch,
    WorkspaceMismatch,
    RequesterMismatch,
    ExpiryMismatch,
    PolicyMismatch,
    DecisionMismatch,
    Revoked,
    Replay,
    RemoteIneligibleLocalOnly,
    RemoteIneligibleHumanOnly,
    QuotaExceeded,
    InvalidInput(&'static str),
}

impl fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "approval request not found"),
            Self::AlreadyExists => write!(f, "approval request already exists"),
            Self::AlreadyDecided => write!(f, "approval already decided"),
            Self::Expired => write!(f, "approval expired"),
            Self::DigestMismatch => write!(f, "operation digest mismatch"),
            Self::DeviceMismatch => write!(f, "device mismatch"),
            Self::WorkspaceMismatch => write!(f, "workspace mismatch"),
            Self::RequesterMismatch => write!(f, "requester mismatch"),
            Self::ExpiryMismatch => write!(f, "expiry mismatch"),
            Self::PolicyMismatch => write!(f, "policy version mismatch"),
            Self::DecisionMismatch => write!(f, "decision does not match recorded decision"),
            Self::Revoked => write!(f, "approval revoked"),
            Self::Replay => write!(f, "approval receipt already consumed"),
            Self::RemoteIneligibleLocalOnly => {
                write!(f, "local-only operation cannot be approved remotely")
            }
            Self::RemoteIneligibleHumanOnly => {
                write!(f, "human-only operation cannot be approved remotely")
            }
            Self::QuotaExceeded => write!(f, "too many pending approvals"),
            Self::InvalidInput(s) => write!(f, "invalid input: {s}"),
        }
    }
}

impl std::error::Error for ApprovalError {}

#[derive(Debug, Clone)]
struct StoredRequest {
    req: ApprovalRequest,
    decision: Option<Decision>,
}

#[derive(Debug)]
struct Inner {
    requests: HashMap<String, StoredRequest>,
    revoked_devices: HashSet<String>,
    revoked_requests: HashSet<String>,
    consumed: HashSet<String>,
    policy_version: u64,
}

#[derive(Debug)]
pub struct RemoteApprovalStore {
    inner: Mutex<Inner>,
}

impl Default for RemoteApprovalStore {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteApprovalStore {
    pub fn new() -> Self {
        Self::with_policy_version(1)
    }

    pub fn with_policy_version(policy_version: u64) -> Self {
        Self {
            inner: Mutex::new(Inner {
                requests: HashMap::new(),
                revoked_devices: HashSet::new(),
                revoked_requests: HashSet::new(),
                consumed: HashSet::new(),
                policy_version,
            }),
        }
    }

    pub fn policy_version(&self) -> u64 {
        self.inner.lock().map(|g| g.policy_version).unwrap_or(0)
    }

    pub fn pending_count(&self) -> usize {
        self.inner
            .lock()
            .map(|g| g.requests.values().filter(|s| s.decision.is_none()).count())
            .unwrap_or(0)
    }

    pub fn decision_of(&self, request_id: &str) -> Option<Decision> {
        self.inner
            .lock()
            .ok()?
            .requests
            .get(request_id)
            .and_then(|s| s.decision)
    }

    pub fn is_consumed(&self, request_id: &str) -> bool {
        self.inner
            .lock()
            .map(|g| g.consumed.contains(request_id))
            .unwrap_or(false)
    }

    fn check_field(s: &str) -> Result<(), ApprovalError> {
        if s.is_empty() {
            return Err(ApprovalError::InvalidInput("empty field"));
        }
        if s.len() > MAX_FIELD_LEN {
            return Err(ApprovalError::InvalidInput("field too long"));
        }
        Ok(())
    }

    /// Register a new pending request. No decision recorded yet.
    pub fn register(&self, req: ApprovalRequest) -> Result<(), ApprovalError> {
        Self::check_field(&req.id)?;
        Self::check_field(&req.device_id)?;
        Self::check_field(&req.workspace_id)?;
        Self::check_field(&req.requester)?;
        let mut g = self.inner.lock().map_err(|_| ApprovalError::InvalidInput("lock"))?;
        if g.requests.contains_key(&req.id) {
            return Err(ApprovalError::AlreadyExists);
        }
        if g.requests.len() >= MAX_PENDING_APPROVALS {
            return Err(ApprovalError::QuotaExceeded);
        }
        if req.policy_version != g.policy_version {
            return Err(ApprovalError::PolicyMismatch);
        }
        g.requests.insert(req.id.clone(), StoredRequest { req, decision: None });
        Ok(())
    }

    /// Remote decide: binds presented fields to the stored request and records
    /// exactly one decision. Second caller gets `AlreadyDecided` (race = one
    /// winner). Failures record nothing.
    pub fn decide(
        &self,
        request_id: &str,
        presented: &PresentedBinding,
        decision: Decision,
        now_ms: u64,
    ) -> Result<ApprovalReceipt, ApprovalError> {
        let mut g = self.inner.lock().map_err(|_| ApprovalError::InvalidInput("lock"))?;
        let stored = g.requests.get(request_id).ok_or(ApprovalError::NotFound)?;
        if stored.decision.is_some() {
            return Err(ApprovalError::AlreadyDecided);
        }
        let req = stored.req.clone();
        if g.revoked_requests.contains(request_id) || g.revoked_devices.contains(&req.device_id) {
            return Err(ApprovalError::Revoked);
        }
        if req.policy_version != g.policy_version {
            return Err(ApprovalError::Revoked);
        }
        if req.local_only {
            return Err(ApprovalError::RemoteIneligibleLocalOnly);
        }
        if req.human_only {
            return Err(ApprovalError::RemoteIneligibleHumanOnly);
        }
        if now_ms > req.expires_at_ms {
            return Err(ApprovalError::Expired);
        }
        if presented.op_digest != req.op_digest {
            return Err(ApprovalError::DigestMismatch);
        }
        if presented.device_id != req.device_id {
            return Err(ApprovalError::DeviceMismatch);
        }
        if presented.workspace_id != req.workspace_id {
            return Err(ApprovalError::WorkspaceMismatch);
        }
        if presented.requester != req.requester {
            return Err(ApprovalError::RequesterMismatch);
        }
        if presented.expires_at_ms != req.expires_at_ms {
            return Err(ApprovalError::ExpiryMismatch);
        }
        if presented.policy_version != req.policy_version {
            return Err(ApprovalError::PolicyMismatch);
        }
        // Commit: single winner.
        if let Some(s) = g.requests.get_mut(request_id) {
            s.decision = Some(decision);
        }
        Ok(ApprovalReceipt {
            request_id: req.id,
            op_digest: req.op_digest,
            device_id: req.device_id,
            workspace_id: req.workspace_id,
            requester: req.requester,
            expires_at_ms: req.expires_at_ms,
            policy_version: req.policy_version,
            decision,
        })
    }

    /// Consume a receipt to authorize one execution. Verifies the receipt is
    /// still bound to the live operation (`current_op_digest`), unexpired,
    /// unrevoked, matching the recorded decision, and unused. Marks consumed;
    /// any failure performs no execution (caller must not execute on `Err`).
    pub fn consume(
        &self,
        receipt: &ApprovalReceipt,
        current_op_digest: &[u8; 32],
        now_ms: u64,
    ) -> Result<Decision, ApprovalError> {
        let mut g = self.inner.lock().map_err(|_| ApprovalError::InvalidInput("lock"))?;
        if g.revoked_requests.contains(&receipt.request_id)
            || g.revoked_devices.contains(&receipt.device_id)
        {
            return Err(ApprovalError::Revoked);
        }
        if receipt.policy_version != g.policy_version {
            return Err(ApprovalError::Revoked);
        }
        let stored = g.requests.get(&receipt.request_id).ok_or(ApprovalError::NotFound)?;
        let req = stored.req.clone();
        let recorded = stored.decision;
        // Receipt must be faithful to the stored request.
        if receipt.op_digest != req.op_digest {
            return Err(ApprovalError::DigestMismatch);
        }
        if receipt.device_id != req.device_id {
            return Err(ApprovalError::DeviceMismatch);
        }
        if receipt.workspace_id != req.workspace_id {
            return Err(ApprovalError::WorkspaceMismatch);
        }
        if receipt.requester != req.requester {
            return Err(ApprovalError::RequesterMismatch);
        }
        if receipt.expires_at_ms != req.expires_at_ms {
            return Err(ApprovalError::ExpiryMismatch);
        }
        if receipt.policy_version != req.policy_version {
            return Err(ApprovalError::PolicyMismatch);
        }
        // Live operation content must be unchanged since approval.
        if current_op_digest != &receipt.op_digest {
            return Err(ApprovalError::DigestMismatch);
        }
        if now_ms > receipt.expires_at_ms {
            return Err(ApprovalError::Expired);
        }
        if recorded != Some(receipt.decision) {
            return Err(ApprovalError::DecisionMismatch);
        }
        if !g.consumed.insert(receipt.request_id.clone()) {
            return Err(ApprovalError::Replay);
        }
        Ok(receipt.decision)
    }

    pub fn revoke_device(&self, device_id: &str) {
        if let Ok(mut g) = self.inner.lock() {
            g.revoked_devices.insert(device_id.to_string());
        }
    }

    pub fn revoke_request(&self, request_id: &str) {
        if let Ok(mut g) = self.inner.lock() {
            g.revoked_requests.insert(request_id.to_string());
        }
    }

    /// PC policy change: invalidates all pending (unconsumed) authority issued
    /// under older versions. Returns the new version.
    pub fn bump_policy(&self) -> u64 {
        self.inner
            .lock()
            .map(|mut g| {
                g.policy_version = g.policy_version.saturating_add(1);
                g.policy_version
            })
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    fn digest(b: u8) -> [u8; 32] {
        [b; 32]
    }

    fn req(id: &str, policy: u64) -> ApprovalRequest {
        ApprovalRequest {
            id: id.to_string(),
            op_digest: digest(7),
            device_id: "phone-1".into(),
            workspace_id: "ws-1".into(),
            requester: "rashid".into(),
            expires_at_ms: 10_000,
            local_only: false,
            human_only: false,
            policy_version: policy,
        }
    }

    fn binding(r: &ApprovalRequest) -> PresentedBinding {
        PresentedBinding {
            op_digest: r.op_digest,
            device_id: r.device_id.clone(),
            workspace_id: r.workspace_id.clone(),
            requester: r.requester.clone(),
            expires_at_ms: r.expires_at_ms,
            policy_version: r.policy_version,
        }
    }

    // NET-009-T01: valid approval matches digest/device/workspace/requester/expiry.
    #[test]
    fn valid_approval_matches_all_fields() {
        let s = RemoteApprovalStore::new();
        let pv = s.policy_version();
        let r = req("op-1", pv);
        s.register(r.clone()).unwrap();
        let rc = s.decide("op-1", &binding(&r), Decision::Approve, 5_000).unwrap();
        assert_eq!(rc.request_id, "op-1");
        assert_eq!(rc.op_digest, r.op_digest);
        assert_eq!(rc.device_id, r.device_id);
        assert_eq!(rc.workspace_id, r.workspace_id);
        assert_eq!(rc.requester, r.requester);
        assert_eq!(rc.expires_at_ms, r.expires_at_ms);
        assert_eq!(s.consume(&rc, &digest(7), 6_000), Ok(Decision::Approve));

        // Each bound field mismatched => rejected, nothing recorded.
        let base = req("op-2", pv);
        s.register(base.clone()).unwrap();
        let mut bad = binding(&base);
        bad.device_id = "phone-2".into();
        assert_eq!(s.decide("op-2", &bad, Decision::Approve, 5_000), Err(ApprovalError::DeviceMismatch));
        bad = binding(&base);
        bad.op_digest = digest(9);
        assert_eq!(s.decide("op-2", &bad, Decision::Approve, 5_000), Err(ApprovalError::DigestMismatch));
        bad = binding(&base);
        bad.workspace_id = "ws-2".into();
        assert_eq!(s.decide("op-2", &bad, Decision::Approve, 5_000), Err(ApprovalError::WorkspaceMismatch));
        bad = binding(&base);
        bad.requester = "mallory".into();
        assert_eq!(s.decide("op-2", &bad, Decision::Approve, 5_000), Err(ApprovalError::RequesterMismatch));
        bad = binding(&base);
        bad.expires_at_ms = 99;
        assert_eq!(s.decide("op-2", &bad, Decision::Approve, 5_000), Err(ApprovalError::ExpiryMismatch));
        assert_eq!(s.decision_of("op-2"), None, "failed decide must record nothing");
        // Expired request rejected.
        assert_eq!(
            s.decide("op-2", &binding(&base), Decision::Approve, 10_001),
            Err(ApprovalError::Expired)
        );
        assert_eq!(s.decision_of("op-2"), None);
    }

    // NET-009-T02: racing approve/deny yields exactly one decision.
    #[test]
    fn race_yields_one_decision() {
        let s = Arc::new(RemoteApprovalStore::new());
        let pv = s.policy_version();
        let r = req("race-1", pv);
        let b = binding(&r);
        s.register(r).unwrap();
        let gate = Arc::new(Barrier::new(2));
        let (s1, s2, b1, b2, g1, g2) = (Arc::clone(&s), Arc::clone(&s), b.clone(), b, Arc::clone(&gate), Arc::clone(&gate));
        let t1 = std::thread::spawn(move || {
            g1.wait();
            s1.decide("race-1", &b1, Decision::Approve, 1_000)
        });
        let t2 = std::thread::spawn(move || {
            g2.wait();
            s2.decide("race-1", &b2, Decision::Deny, 1_000)
        });
        let r1 = t1.join().unwrap();
        let r2 = t2.join().unwrap();
        let oks = [r1.is_ok(), r2.is_ok()].into_iter().filter(|x| *x).count();
        assert_eq!(oks, 1, "exactly one racer wins, got {r1:?} {r2:?}");
        let errs: Vec<_> = [r1, r2].into_iter().filter_map(|r| r.err()).collect();
        assert_eq!(errs, vec![ApprovalError::AlreadyDecided]);
        // One deterministic stored decision; one execution only.
        let winner = s.decision_of("race-1").expect("winner recorded");
        let receipt = ApprovalReceipt {
            request_id: "race-1".into(),
            op_digest: digest(7),
            device_id: "phone-1".into(),
            workspace_id: "ws-1".into(),
            requester: "rashid".into(),
            expires_at_ms: 10_000,
            policy_version: pv,
            decision: winner,
        };
        assert_eq!(s.consume(&receipt, &digest(7), 2_000), Ok(winner));
        assert_eq!(s.consume(&receipt, &digest(7), 2_000), Err(ApprovalError::Replay));
    }

    // NET-009-T03: replay or changed content invalidates, no side effects.
    #[test]
    fn replay_or_changed_content_invalid() {
        let s = RemoteApprovalStore::new();
        let pv = s.policy_version();
        let r = req("op-3", pv);
        s.register(r.clone()).unwrap();
        let rc = s.decide("op-3", &binding(&r), Decision::Approve, 1_000).unwrap();
        assert_eq!(s.consume(&rc, &digest(7), 2_000), Ok(Decision::Approve));
        // Replay: second execution refused.
        assert_eq!(s.consume(&rc, &digest(7), 2_000), Err(ApprovalError::Replay));

        let r2 = ApprovalRequest { id: "op-4".into(), ..req("op-4", pv) };
        s.register(r2.clone()).unwrap();
        let rc2 = s.decide("op-4", &binding(&r2), Decision::Approve, 1_000).unwrap();
        // Operation content changed after approval => invalid, no execution.
        assert_eq!(s.consume(&rc2, &digest(0xFF), 2_000), Err(ApprovalError::DigestMismatch));
        assert!(!s.is_consumed("op-4"), "failed consume must not mark consumed");
    }

    // NET-009-T04: local-only / human-only enforced against remote clients.
    #[test]
    fn local_only_human_only_enforced_remotely() {
        let s = RemoteApprovalStore::new();
        let pv = s.policy_version();
        let mut local = req("op-local", pv);
        local.local_only = true;
        s.register(local.clone()).unwrap();
        assert_eq!(
            s.decide("op-local", &binding(&local), Decision::Approve, 1_000),
            Err(ApprovalError::RemoteIneligibleLocalOnly)
        );
        assert_eq!(s.decision_of("op-local"), None);

        let mut human = req("op-human", pv);
        human.human_only = true;
        s.register(human.clone()).unwrap();
        assert_eq!(
            s.decide("op-human", &binding(&human), Decision::Approve, 1_000),
            Err(ApprovalError::RemoteIneligibleHumanOnly)
        );
        assert_eq!(s.decision_of("op-human"), None);
    }

    // NET-009-T05: revocation / logout / policy change invalidates pending authority.
    #[test]
    fn revocation_invalidates_pending() {
        let s = RemoteApprovalStore::new();
        let pv = s.policy_version();
        // Revoked before decide.
        let r = req("op-5", pv);
        s.register(r.clone()).unwrap();
        s.revoke_request("op-5");
        assert_eq!(s.decide("op-5", &binding(&r), Decision::Approve, 1_000), Err(ApprovalError::Revoked));
        assert_eq!(s.decision_of("op-5"), None);

        // Device logout (revocation) after decide invalidates receipt.
        let r2 = ApprovalRequest { id: "op-6".into(), ..req("op-6", pv) };
        s.register(r2.clone()).unwrap();
        let rc2 = s.decide("op-6", &binding(&r2), Decision::Approve, 1_000).unwrap();
        s.revoke_device("phone-1");
        assert_eq!(s.consume(&rc2, &digest(7), 2_000), Err(ApprovalError::Revoked));

        // PC policy change invalidates other pending authority.
        let s2 = RemoteApprovalStore::new();
        let pv2 = s2.policy_version();
        let r3 = req("op-7", pv2);
        s2.register(r3.clone()).unwrap();
        s2.bump_policy();
        assert_eq!(s2.decide("op-7", &binding(&r3), Decision::Approve, 1_000), Err(ApprovalError::Revoked));
    }
}
