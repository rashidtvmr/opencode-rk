//! Live, session-scoped ownership for turn cancellation and pending approvals.
#![forbid(unsafe_code)]

use opencode_rk_contracts::SessionId;
use opencode_rk_security::{HumanApproval, OperationIntent, PermissionBroker, PolicyContext};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::{Semaphore, SemaphorePermit, oneshot, watch};

pub const MAX_ACTIVE_TURNS: usize = 2;
/// Dedicated child-lane capacity so a saturated root lane cannot starve
/// child turns (both lanes still share one active-registry bound).
pub const MAX_ACTIVE_CHILD_TURNS: usize = MAX_ACTIVE_TURNS;
const MAX_ALWAYS_GRANTS: usize = 128;
const ALWAYS_GRANT_TTL: std::time::Duration = std::time::Duration::from_secs(30 * 60);

static TURN_PERMITS: Semaphore = Semaphore::const_new(MAX_ACTIVE_TURNS);
static CHILD_TURN_PERMITS: Semaphore = Semaphore::const_new(MAX_ACTIVE_CHILD_TURNS);
static ACTIVE_TURNS: Mutex<[Option<Arc<RunOwner>>; MAX_ACTIVE_TURNS + MAX_ACTIVE_CHILD_TURNS]> =
    Mutex::new([const { None }; MAX_ACTIVE_TURNS + MAX_ACTIVE_CHILD_TURNS]);
static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);
static ALWAYS_GRANTS: Mutex<Vec<AlwaysGrant>> = Mutex::new(Vec::new());

#[derive(Clone, Debug, serde::Serialize)]
pub struct PermissionRequest {
    pub id: String,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub action: String,
    pub resources: Vec<String>,
    pub source: PermissionSource,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PermissionSource {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(rename = "messageID")]
    pub message_id: String,
    #[serde(rename = "callID")]
    pub call_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PermissionReply {
    Once,
    Always,
    Reject,
    Cancelled,
}

pub struct ApprovedPermission {
    pub reply: PermissionReply,
    pub capability: Option<HumanApproval>,
}

struct PendingPermission {
    request: PermissionRequest,
    intent: OperationIntent,
    policy_context: PolicyContext,
    broker: PermissionBroker,
    reply: oneshot::Sender<ApprovedPermission>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AlwaysGrant {
    session_id: SessionId,
    action: String,
    resource: String,
    policy_context: PolicyContext,
    expires_at: std::time::Instant,
}

/// The actual owner for one live run, shared with its HTTP cancellation routes.
pub struct RunOwner {
    pub session_id: SessionId,
    pub run_id: u64,
    cancellation: watch::Sender<bool>,
    explicit_cancel: watch::Sender<bool>,
    finished: watch::Sender<bool>,
    pending: Mutex<Option<PendingPermission>>,
    effect_gate: Mutex<()>,
}

impl RunOwner {
    fn new(session_id: SessionId) -> Arc<Self> {
        let (cancellation, _) = watch::channel(false);
        let (explicit_cancel, _) = watch::channel(false);
        let (finished, _) = watch::channel(false);
        Arc::new(Self {
            session_id,
            run_id: NEXT_RUN_ID.fetch_add(1, Ordering::Relaxed),
            cancellation,
            explicit_cancel,
            finished,
            pending: Mutex::new(None),
            effect_gate: Mutex::new(()),
        })
    }

    pub fn cancellation_receiver(&self) -> watch::Receiver<bool> {
        self.cancellation.subscribe()
    }

    pub(crate) fn explicit_cancellation_receiver(&self) -> watch::Receiver<bool> {
        self.explicit_cancel.subscribe()
    }
    pub(crate) fn is_explicitly_cancelled(&self) -> bool {
        *self.explicit_cancel.borrow()
    }

    pub fn begin_effect(&self) -> Option<std::sync::MutexGuard<'_, ()>> {
        let gate = self.effect_gate.lock().ok()?;
        (!self.is_cancelled()).then_some(gate)
    }

    pub fn is_cancelled(&self) -> bool {
        *self.cancellation.borrow()
    }

    pub fn cancel(&self) {
        // Cancellation can originate in a response-body Drop on the runtime
        // thread. Never block that thread on filesystem work owned by a
        // spawn_blocking task. begin_effect checks this signal under its gate;
        // explicit cancellation acknowledgements await wait_finished below.
        self.cancellation.send_replace(true);
        self.clear_pending();
    }

    pub(crate) fn cancel_explicit(&self) {
        self.explicit_cancel.send_replace(true);
        self.cancel();
    }

    pub async fn wait_finished(&self) {
        let mut finished = self.finished.subscribe();
        loop {
            if *finished.borrow_and_update() {
                return;
            }
            if finished.changed().await.is_err() {
                return;
            }
        }
    }

    pub fn pending(&self) -> Option<PermissionRequest> {
        self.pending
            .lock()
            .ok()?
            .as_ref()
            .map(|pending| pending.request.clone())
    }

    /// Lock-protected presence check without cloning the request payload.
    /// Returns true when a human-pending permission is stored; the request
    /// body (including `resources`, which embeds the serialized
    /// `OperationIntent`) is never read or copied. Poison-safe: a poisoned
    /// mutex reports no pending permission rather than panicking.
    pub fn has_pending(&self) -> bool {
        self.pending.lock().is_ok_and(|guard| guard.is_some())
    }

    pub fn install_pending(
        &self,
        request: PermissionRequest,
        intent: OperationIntent,
        broker: &PermissionBroker,
    ) -> Result<oneshot::Receiver<ApprovedPermission>, Box<PermissionRequest>> {
        let (reply, receiver) = oneshot::channel();
        let mut pending = self.pending.lock().map_err(|_| Box::new(request.clone()))?;
        if pending.is_some() || self.is_cancelled() {
            return Err(Box::new(request));
        }
        let resource=crate::permission_rules::fingerprint(&serde_json::to_string(&intent).unwrap_or_default());
        let context=crate::permission_rules::fingerprint(&format!("{:?}",broker.policy_context()));
        match crate::permission_rules::decide(&request.action,&self.session_id.to_string(),&resource,&context) {
            Ok(Some(allow))=>{
                let capability=if allow {broker.approve_human(&intent,self.session_id.to_string(),self.run_id,request.source.call_id.clone())} else {None};
                if allow&&capability.is_none() {return Err(Box::new(request));}
                let _=reply.send(ApprovedPermission {reply:if allow {PermissionReply::Once} else {PermissionReply::Reject},capability});
                return Ok(receiver);
            }
            Err(_)=>return Err(Box::new(request)),
            Ok(None)=>{}
        }
        *pending = Some(PendingPermission {
            request,
            intent,
            policy_context: broker.policy_context(),
            broker: broker.clone(),
            reply,
        });
        Ok(receiver)
    }

    /// TEST-ONLY direct pending injection. Identical slot semantics to
    /// `install_pending` (exclusive pending slot, refuses when cancelled or
    /// already pending) but skips persisted-rule adjudication so tests never
    /// read or depend on the real user preference file at
    /// `permission_rules::path()`. Compiled out of release builds; no
    /// production caller exists, so broker adjudication and its security
    /// properties are untouched.
    #[cfg(test)]
    pub(crate) fn install_pending_test_only(
        &self,
        request: PermissionRequest,
        intent: OperationIntent,
        broker: &PermissionBroker,
    ) -> Result<oneshot::Receiver<ApprovedPermission>, Box<PermissionRequest>> {
        let (reply, receiver) = oneshot::channel();
        let mut pending = self.pending.lock().map_err(|_| Box::new(request.clone()))?;
        if pending.is_some() || self.is_cancelled() {
            return Err(Box::new(request));
        }
        *pending = Some(PendingPermission {
            request,
            intent,
            policy_context: broker.policy_context(),
            broker: broker.clone(),
            reply,
        });
        Ok(receiver)
    }

    pub fn complete_pending(&self, request_id: &str, decision: PermissionReply) -> bool {
        self.reply_pending(request_id, decision, None)
    }

    pub fn reply_scoped(&self,request_id:&str,allow:bool,scope:&str)->Result<bool,String> {
        if !matches!(scope,"once"|"session"|"always") {return Err("invalid permission scope".into());}
        let mut pending=self.pending.lock().map_err(|_|"permission unavailable")?;
        let Some(entry)=pending.as_ref().filter(|entry|entry.request.id==request_id) else {return Ok(false);};
        if self.is_cancelled()||entry.broker.policy_context()!=entry.policy_context {return Ok(false);}
        let capability=if allow {entry.broker.approve_human(&entry.intent,self.session_id.to_string(),self.run_id,entry.request.source.call_id.clone())} else {None};
        if allow&&capability.is_none() {return Ok(false);}
        if scope!="once" {
            crate::permission_rules::upsert(crate::permission_rules::Rule {
                tool:entry.request.action.clone(),decision:if allow {"allow"} else {"deny"}.into(),scope:scope.into(),
                session_id:if scope=="session" {Some(self.session_id.to_string())} else {None},
                resource:Some(crate::permission_rules::fingerprint(&serde_json::to_string(&entry.intent).map_err(|_|"invalid intent")?)),
                context:Some(crate::permission_rules::fingerprint(&format!("{:?}",entry.policy_context))),
            })?;
        }
        let entry=pending.take().expect("checked pending");
        Ok(entry.reply.send(ApprovedPermission {reply:if allow {PermissionReply::Once} else {PermissionReply::Reject},capability}).is_ok())
    }

    fn reply_pending(
        &self,
        request_id: &str,
        decision: PermissionReply,
        always_grant: Option<(String, String)>,
    ) -> bool {
        let mut grants = if always_grant.is_some() {
            match ALWAYS_GRANTS.lock() {
                Ok(mut grants) => {
                    grants.retain(|grant| grant.expires_at > std::time::Instant::now());
                    Some(grants)
                }
                Err(_) => return false,
            }
        } else {
            None
        };
        let mut pending = match self.pending.lock() {
            Ok(pending) => pending,
            Err(_) => return false,
        };
        if !pending
            .as_ref()
            .is_some_and(|entry| entry.request.id == request_id)
        {
            return false;
        }
        let expected_context = pending
            .as_ref()
            .expect("pending was checked")
            .policy_context
            .clone();
        if let (Some(grant), Some(grants)) = (always_grant.as_ref(), grants.as_ref()) {
            let key = AlwaysGrant {
                session_id: self.session_id,
                action: grant.0.clone(),
                resource: grant.1.clone(),
                policy_context: expected_context,
                expires_at: std::time::Instant::now() + ALWAYS_GRANT_TTL,
            };
            if !grants.contains(&key) && grants.len() >= MAX_ALWAYS_GRANTS {
                return false;
            }
        }
        let entry = pending.take().expect("pending was checked");
        let capability = if matches!(decision, PermissionReply::Once | PermissionReply::Always) {
            let broker = &entry.broker;
            if broker.policy_context() != entry.policy_context {
                let _ = entry.reply.send(ApprovedPermission {
                    reply: PermissionReply::Cancelled,
                    capability: None,
                });
                return false;
            }
            let capability = broker.approve_human(
                &entry.intent,
                self.session_id.to_string(),
                self.run_id,
                entry.request.source.call_id.clone(),
            );
            if capability.is_none() {
                let _ = entry.reply.send(ApprovedPermission {
                    reply: PermissionReply::Cancelled,
                    capability: None,
                });
                return false;
            }
            capability
        } else {
            None
        };
        if matches!(decision, PermissionReply::Once | PermissionReply::Always)
            && capability.is_none()
        {
            return false;
        }
        if entry
            .reply
            .send(ApprovedPermission {
                reply: decision,
                capability,
            })
            .is_err()
        {
            return false;
        }
        if let (Some(grant), Some(grants)) = (always_grant, grants.as_mut()) {
            let key = AlwaysGrant {
                session_id: self.session_id,
                action: grant.0,
                resource: grant.1,
                policy_context: entry.policy_context,
                expires_at: std::time::Instant::now() + ALWAYS_GRANT_TTL,
            };
            if !grants.contains(&key) {
                grants.push(key);
            }
        }
        true
    }

    pub fn reply_once(&self, request_id: &str) -> bool {
        self.reply_pending(request_id, PermissionReply::Once, None)
    }

    pub fn reply_always(&self, request_id: &str, action: String, resource: String) -> bool {
        self.reply_pending(
            request_id,
            PermissionReply::Always,
            Some((action, resource)),
        )
    }

    pub fn reply_reject(&self, request_id: &str) -> bool {
        self.reply_pending(request_id, PermissionReply::Reject, None)
    }

    pub fn clear_pending(&self) {
        if let Ok(mut pending) = self.pending.lock() {
            if let Some(pending) = pending.take() {
                let _ = pending.reply.send(ApprovedPermission {
                    reply: PermissionReply::Cancelled,
                    capability: None,
                });
            }
        }
    }

    pub fn finish(&self) {
        self.cancel();
        self.finished.send_replace(true);
    }
}

pub struct PendingPermissionHandle {
    pub request: PermissionRequest,
    reply: oneshot::Sender<PermissionReply>,
}

impl PendingPermissionHandle {
    pub fn deliver(self, decision: PermissionReply) -> bool {
        self.reply.send(decision).is_ok()
    }
}

pub struct PendingPermissionGuard {
    owner: Arc<RunOwner>,
    request_id: String,
}

impl PendingPermissionGuard {
    pub fn new(owner: Arc<RunOwner>, request_id: String) -> Self {
        Self { owner, request_id }
    }
}

impl Drop for PendingPermissionGuard {
    fn drop(&mut self) {
        if let Ok(mut pending) = self.owner.pending.lock() {
            if pending
                .as_ref()
                .is_some_and(|entry| entry.request.id == self.request_id)
            {
                if let Some(entry) = pending.take() {
                    let _ = entry.reply.send(ApprovedPermission {
                        reply: PermissionReply::Cancelled,
                        capability: None,
                    });
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum AdmissionError {
    Capacity,
    SessionActive,
    RegistryUnavailable,
}

pub struct TurnExecutionPermit {
    _global: SemaphorePermit<'static>,
    owner: Arc<RunOwner>,
}

impl TurnExecutionPermit {
    pub fn acquire(session_id: SessionId) -> Result<Self, AdmissionError> {
        Self::acquire_lane(session_id, &TURN_PERMITS)
    }

    /// Child-lane admission: its own bounded permits, so a saturated root
    /// lane cannot starve child turns. Shares the same session-exclusion
    /// registry as roots, so one session can never run twice across lanes.
    pub fn acquire_child(session_id: SessionId) -> Result<Self, AdmissionError> {
        Self::acquire_lane(session_id, &CHILD_TURN_PERMITS)
    }

    fn acquire_lane(
        session_id: SessionId,
        lane_permits: &'static Semaphore,
    ) -> Result<Self, AdmissionError> {
        let global = lane_permits
            .try_acquire()
            .map_err(|_| AdmissionError::Capacity)?;
        let mut active = ACTIVE_TURNS
            .lock()
            .map_err(|_| AdmissionError::RegistryUnavailable)?;
        if active
            .iter()
            .flatten()
            .any(|owner| owner.session_id == session_id)
        {
            return Err(AdmissionError::SessionActive);
        }
        let slot = active
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or(AdmissionError::Capacity)?;
        let owner = RunOwner::new(session_id);
        *slot = Some(Arc::clone(&owner));
        Ok(Self {
            _global: global,
            owner,
        })
    }

    pub fn owner(&self) -> Arc<RunOwner> {
        Arc::clone(&self.owner)
    }
}

impl Drop for TurnExecutionPermit {
    fn drop(&mut self) {
        self.owner.finish();
        if let Ok(mut active) = ACTIVE_TURNS.lock() {
            for slot in active.iter_mut() {
                if slot
                    .as_ref()
                    .is_some_and(|owner| Arc::ptr_eq(owner, &self.owner))
                {
                    *slot = None;
                }
            }
        }
    }
}

pub fn active(session_id: SessionId) -> Option<Arc<RunOwner>> {
    ACTIVE_TURNS
        .lock()
        .ok()?
        .iter()
        .flatten()
        .find(|owner| owner.session_id == session_id)
        .cloned()
}

/// Atomically projected rail-tab view of one live run owner. Carries only
/// stable, non-secret identity and lifecycle booleans: it never exposes a
/// `PermissionRequest` or its `resources` field (which embeds the serialized
/// `OperationIntent`: file paths, process args, SQL, MCP argument JSON), the
/// pending intent, policy context, broker, or any capability token.
/// `admitted` reflects registry admission only, not an in-flight model turn;
/// `has_pending` is a presence boolean, never the request body.
#[derive(Clone, Debug, serde::Serialize)]
pub struct SessionTab {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(rename = "runID")]
    pub run_id: u64,
    #[serde(rename = "admitted")]
    pub admitted: bool,
    #[serde(rename = "cancelled")]
    pub cancelled: bool,
    #[serde(rename = "hasPendingPermission")]
    pub has_pending: bool,
}

/// Single-lock atomic snapshot of every live run owner (roots and children
/// share `ACTIVE_TURNS`, so both appear) in registry-slot order. Bounded by
/// `MAX_ACTIVE_TURNS + MAX_ACTIVE_CHILD_TURNS` by construction. Holds
/// `ACTIVE_TURNS` across field copies and the boolean `has_pending` probe --
/// the same outer-to-inner order (`ACTIVE_TURNS`, then `RunOwner.pending`)
/// as the existing `active` + `pending` caller in lib.rs, so no lock
/// inversion is introduced. Returns empty on a poisoned registry rather
/// than panicking.
pub fn snapshot() -> Vec<SessionTab> {
    let active = match ACTIVE_TURNS.lock() {
        Ok(active) => active,
        Err(_) => return Vec::new(),
    };
    active
        .iter()
        .flatten()
        .map(|owner| SessionTab {
            session_id: owner.session_id.to_string(),
            run_id: owner.run_id,
            admitted: true,
            cancelled: owner.is_cancelled(),
            has_pending: owner.has_pending(),
        })
        .collect()
}

pub async fn cancel_and_wait(session_id: SessionId) {
    let Some(owner) = active(session_id) else {
        return;
    };
    owner.cancel_explicit();
    owner.wait_finished().await;
}

pub fn has_always_grant(
    session_id: SessionId,
    action: &str,
    resource: &str,
    context: &PolicyContext,
) -> bool {
    ALWAYS_GRANTS.lock().is_ok_and(|grants| {
        grants.iter().any(|grant| {
            grant.session_id == session_id
                && grant.action == action
                && grant.resource == resource
                && &grant.policy_context == context
                && grant.expires_at > std::time::Instant::now()
        })
    })
}

pub fn insert_always_grant(
    session_id: SessionId,
    action: String,
    resource: String,
    context: PolicyContext,
) -> bool {
    let Ok(mut grants) = ALWAYS_GRANTS.lock() else {
        return false;
    };
    grants.retain(|grant| grant.expires_at > std::time::Instant::now());
    let grant = AlwaysGrant {
        session_id,
        action,
        resource,
        policy_context: context,
        expires_at: std::time::Instant::now() + ALWAYS_GRANT_TTL,
    };
    if grants.contains(&grant) {
        return true;
    }
    if grants.len() >= MAX_ALWAYS_GRANTS {
        return false;
    }
    grants.push(grant);
    true
}

pub fn can_store_always(
    session_id: SessionId,
    action: &str,
    resource: &str,
    context: &PolicyContext,
) -> bool {
    ALWAYS_GRANTS.lock().is_ok_and(|grants| {
        grants.iter().any(|grant| {
            grant.session_id == session_id
                && grant.action == action
                && grant.resource == resource
                && &grant.policy_context == context
                && grant.expires_at > std::time::Instant::now()
        }) || grants.len() < MAX_ALWAYS_GRANTS
    })
}

#[cfg(test)]
pub fn store_always(session_id: SessionId, action: String, resource: String) -> bool {
    insert_always_grant(
        session_id,
        action,
        resource,
        PermissionBroker::new(opencode_rk_security::SecurityPolicy::lean_default(
            "/".to_owned(),
        ))
        .policy_context(),
    )
}

#[cfg(test)]
mod admission_lane_tests {
    use super::{
        ACTIVE_TURNS, AdmissionError, MAX_ACTIVE_CHILD_TURNS, MAX_ACTIVE_TURNS,
        TurnExecutionPermit,
    };
    use opencode_rk_contracts::SessionId;

    fn fresh() -> SessionId {
        SessionId::new()
    }

    fn hold_admission_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static ADMISSION_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        // Poison-tolerant: a panicked sibling test must not wedge the rest.
        ADMISSION_TEST_LOCK
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    #[test]
    fn root_lane_exhaustion_does_not_starve_child_lane() {
        // These tests exhaust the process-global permit lanes and shared
        // registry; serialize them so RUST_TEST_THREADS=2 cannot interleave.
        let _admission_test_lock = hold_admission_test_lock();
        let roots: Vec<_> = (0..MAX_ACTIVE_TURNS)
            .map(|_| {
                TurnExecutionPermit::acquire(fresh())
                    .expect("root capacity initially available")
            })
            .collect();
        assert!(
            matches!(
                TurnExecutionPermit::acquire(fresh()),
                Err(AdmissionError::Capacity)
            ),
            "third root turn must exhaust the root lane"
        );
        let child = TurnExecutionPermit::acquire_child(fresh())
            .expect("child lane must still admit while root lane is saturated");
        drop(child);
        drop(roots);
    }

    #[test]
    fn duplicate_session_rejected_across_lanes() {
        let _admission_test_lock = hold_admission_test_lock();
        // An active root blocks the same session on the child lane.
        let root = TurnExecutionPermit::acquire(fresh())
            .expect("root capacity initially available");
        assert!(
            matches!(
                TurnExecutionPermit::acquire_child(root.owner().session_id),
                Err(AdmissionError::SessionActive)
            ),
            "acquire_child must keep same-session exclusion with roots"
        );
        // And duplicates inside the child lane itself are rejected too.
        let child = TurnExecutionPermit::acquire_child(fresh())
            .expect("child capacity initially available");
        assert!(
            matches!(
                TurnExecutionPermit::acquire_child(child.owner().session_id),
                Err(AdmissionError::SessionActive)
            ),
            "duplicate child session must be rejected"
        );
    }

    #[test]
    fn aggregate_registry_bounded_at_root_plus_child_capacity() {
        let _admission_test_lock = hold_admission_test_lock();
        let mut permits: Vec<_> = (0..MAX_ACTIVE_TURNS)
            .map(|_| {
                TurnExecutionPermit::acquire(fresh())
                    .expect("root capacity initially available")
            })
            .collect();
        permits.extend((0..MAX_ACTIVE_CHILD_TURNS).map(|_| {
            TurnExecutionPermit::acquire_child(fresh())
                .expect("child capacity initially available")
        }));
        assert_eq!(
            ACTIVE_TURNS.lock().expect("active registry").iter().flatten().count(),
            MAX_ACTIVE_TURNS + MAX_ACTIVE_CHILD_TURNS,
            "shared registry must admit exactly root + child runs concurrently"
        );
        assert!(
            matches!(
                TurnExecutionPermit::acquire(fresh()),
                Err(AdmissionError::Capacity)
            ),
            "aggregate bound must reject further roots"
        );
        assert!(
            matches!(
                TurnExecutionPermit::acquire_child(fresh()),
                Err(AdmissionError::Capacity)
            ),
            "aggregate bound must reject further children"
        );
    }

    #[test]
    fn normal_finish_does_not_mark_explicit_cancellation() {
        let owner = TurnExecutionPermit::acquire(fresh()).unwrap().owner();
        let mut cancelled = owner.explicit_cancellation_receiver();
        owner.finish();
        assert!(!*cancelled.borrow_and_update());
    }

    #[test]
    fn explicit_cancel_marks_private_cancellation_signal() {
        let owner = TurnExecutionPermit::acquire(fresh()).unwrap().owner();
        let mut cancelled = owner.explicit_cancellation_receiver();
        owner.cancel_explicit();
        assert!(*cancelled.borrow_and_update());
    }

    /// LIVE-SNAPSHOT (v3): the rail snapshot must expose registry admission
    /// and lifecycle booleans only. These tests share the process-global
    /// admission registry with the lane-exhaustion tests above, so they
    /// serialize through `hold_admission_test_lock`. Import resolution:
    /// `hold_admission_test_lock` is `admission_lane_tests`-local (`super`),
    /// while `snapshot`, `PermissionRequest`, `PermissionSource` and the
    /// capacity constants live on the `run_owner` module (`super::super`),
    /// where private imports remain visible to descendant test modules.
    mod snapshot_tests {
        use super::hold_admission_test_lock;
        use super::super::{
            MAX_ACTIVE_CHILD_TURNS, MAX_ACTIVE_TURNS, PermissionRequest, PermissionSource,
            TurnExecutionPermit, snapshot,
        };
        use opencode_rk_contracts::SessionId;
        use opencode_rk_security::{OperationIntent, PermissionBroker, SecurityPolicy};

        fn fresh() -> SessionId {
            SessionId::new()
        }

        // Unique fake secret/path injected into a real OperationIntent (process
        // args + cwd) and, mirroring real call sites, into the request's
        // `resources` as the serialized intent. Neither may ever appear in a
        // serialized rail tab or the live route body.
        const FAKE_SECRET: &str = "rail-snapshot-fake-secret-9f3a7c1e";
        const FAKE_PATH: &str = "/tmp/rail-fake-path-7b2d406";

        fn broker() -> PermissionBroker {
            PermissionBroker::new(SecurityPolicy::lean_default("/"))
        }

        fn secret_request(session_id: SessionId) -> (PermissionRequest, OperationIntent) {
            let intent = OperationIntent::Process {
                program: "cat".to_owned(),
                args: vec!["read".to_owned(), FAKE_SECRET.to_owned()],
                cwd: std::path::PathBuf::from(FAKE_PATH),
            };
            let request = PermissionRequest {
                id: "per_snapshot_fixture".to_owned(),
                session_id: session_id.to_string(),
                action: "process: cat".to_owned(),
                resources: vec![serde_json::to_string(&intent).expect("intent serializes")],
                source: PermissionSource {
                    kind: "tool",
                    message_id: "m-snapshot".to_owned(),
                    call_id: "c-snapshot".to_owned(),
                },
            };
            (request, intent)
        }

        /// The wire shape of one rail tab is exactly these five keys. Any
        /// future sensitive field added to `SessionTab` breaks this key set,
        /// so this is a positive shape contract, not a name-substring scan.
        const TAB_KEYS: [&str; 5] = [
            "sessionID",
            "runID",
            "admitted",
            "cancelled",
            "hasPendingPermission",
        ];

        fn assert_tab_wire_shape(tab: &serde_json::Value) {
            let object = tab.as_object().expect("rail tab serializes as an object");
            let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
            keys.sort_unstable();
            let mut expected = TAB_KEYS;
            expected.sort_unstable();
            assert_eq!(keys, expected, "rail tab wire shape must stay boolean-only");
        }

        #[test]
        fn snapshot_empty_when_no_owners() {
            let _lock = hold_admission_test_lock();
            assert!(snapshot().is_empty(), "no owners means no snapshot rows");
        }

        #[test]
        fn snapshot_emits_two_admitted_rows() {
            let _lock = hold_admission_test_lock();
            let first = TurnExecutionPermit::acquire(fresh()).expect("root capacity");
            let second = TurnExecutionPermit::acquire(fresh()).expect("root capacity");
            let first_id = first.owner().session_id.to_string();
            let second_id = second.owner().session_id.to_string();
            let rows = snapshot();
            assert_eq!(rows.len(), 2, "two admitted sessions must both appear");
            let ids: Vec<String> = rows.iter().map(|tab| tab.session_id.clone()).collect();
            assert!(ids.contains(&first_id));
            assert!(ids.contains(&second_id));
            for tab in &rows {
                assert!(tab.admitted, "admitted owner must be admitted == true");
                assert!(!tab.cancelled, "fresh owner must not be cancelled");
                assert!(!tab.has_pending, "no permission requested yet");
                let wire = serde_json::to_value(tab).expect("tab must serialize");
                assert_tab_wire_shape(&wire);
            }
        }

        #[test]
        fn snapshot_empty_after_all_permits_dropped() {
            let _lock = hold_admission_test_lock();
            {
                let _a = TurnExecutionPermit::acquire(fresh()).expect("root capacity");
                let _b = TurnExecutionPermit::acquire(fresh()).expect("root capacity");
                assert_eq!(snapshot().len(), 2);
            }
            assert!(
                snapshot().is_empty(),
                "dropped permits must clear the registry"
            );
        }

        #[test]
        fn snapshot_records_cancelled_and_evicts_on_finish() {
            let _lock = hold_admission_test_lock();
            // Bind the permit to a named binding. A `TurnExecutionPermit`
            // temporary dropped at the end of the statement finishes AND
            // evicts the owner before any assertion can observe the
            // cancelled-but-still-registered state.
            let permit = TurnExecutionPermit::acquire(fresh()).expect("root capacity");
            let owner = permit.owner();
            let id = owner.session_id.to_string();
            owner.cancel_explicit();
            let tab = snapshot()
                .into_iter()
                .find(|tab| tab.session_id == id)
                .expect("cancelled owner must appear in snapshot while its permit lives");
            assert!(tab.cancelled, "explicit cancel must be visible");
            assert!(
                tab.admitted,
                "explicit cancel alone keeps owner admitted until finish"
            );
            owner.finish();
            // Eviction happens on permit drop, never on cancel or finish.
            drop(permit);
            assert!(
                !snapshot().iter().any(|tab| tab.session_id == id),
                "permit drop must evict the owner from the registry"
            );
        }

        /// Genuine pending-permission leak guard. A real `PermissionRequest`
        /// whose `resources` embed the serialized secret-carrying
        /// `OperationIntent` is installed through the legitimate
        /// `install_pending` path, then the projection must report the
        /// presence boolean as true while the wire form omits every seeded
        /// secret. The negative path (cancel flips the boolean without
        /// evicting the owner) is asserted through the same snapshot.
        #[test]
        fn snapshot_has_pending_without_resource_leak() {
            let _lock = hold_admission_test_lock();
            // Bind the permit exactly as above: a temporary dropped at the
            // end of the statement would cancel (and evict) the owner, and
            // the pending hook must then refuse the injection per contract.
            let permit = TurnExecutionPermit::acquire(fresh()).expect("root capacity");
            let owner = permit.owner();
            let id = owner.session_id.to_string();
            let (request, intent) = secret_request(owner.session_id);
            // TEST-ONLY hook: stores the genuine PermissionRequest + secret
            // intent in the real pending slot without consulting persisted
            // rules, so this test never reads or depends on the real user
            // preference file. Adjudication behavior stays covered by the
            // dedicated permission tests; this test pins the projection
            // contract (boolean + no payload leak).
            let _receiver = owner
                .install_pending_test_only(request, intent, &broker())
                .expect("fresh owner with an empty pending slot must store the request");
            let tab = snapshot()
                .into_iter()
                .find(|tab| tab.session_id == id)
                .expect("admitted owner must appear in the snapshot");
            assert!(
                tab.has_pending,
                "a stored pending request must read hasPendingPermission == true"
            );
            let wire = serde_json::to_value(&tab).expect("tab must serialize");
            assert_tab_wire_shape(&wire);
            let serialized = wire.to_string();
            assert!(!serialized.contains(FAKE_SECRET), "secret must not leak");
            assert!(!serialized.contains(FAKE_PATH), "secret cwd must not leak");
            // Negative path: cancel clears the stored request, flipping the
            // boolean without evicting the owner from the registry.
            owner.cancel();
            assert!(!owner.has_pending(), "cancel must clear pending");
            let tab_after = snapshot()
                .into_iter()
                .find(|tab| tab.session_id == id)
                .expect("owner stays registered until the permit drops");
            assert!(!tab_after.has_pending);
            assert!(tab_after.admitted);
            owner.finish();
        }

        #[test]
        fn snapshot_bounded_by_registry_capacity() {
            let _lock = hold_admission_test_lock();
            let roots: Vec<_> = (0..MAX_ACTIVE_TURNS)
                .map(|_| TurnExecutionPermit::acquire(fresh()).expect("root capacity"))
                .collect();
            let children: Vec<_> = (0..MAX_ACTIVE_CHILD_TURNS)
                .map(|_| TurnExecutionPermit::acquire_child(fresh()).expect("child capacity"))
                .collect();
            assert_eq!(
                snapshot().len(),
                MAX_ACTIVE_TURNS + MAX_ACTIVE_CHILD_TURNS,
                "snapshot covers the full shared registry, never beyond it"
            );
            drop(children);
            drop(roots);
        }
    }
}
