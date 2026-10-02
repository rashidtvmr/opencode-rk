//! Live, session-scoped ownership for turn cancellation and pending approvals.
#![forbid(unsafe_code)]

use opencode_rk_contracts::SessionId;
use opencode_rk_security::{HumanApproval, OperationIntent, PermissionBroker, PolicyContext};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use tokio::sync::{oneshot, watch, Semaphore, SemaphorePermit};

pub const MAX_ACTIVE_TURNS: usize = 2;
const MAX_ALWAYS_GRANTS: usize = 128;
const ALWAYS_GRANT_TTL: std::time::Duration = std::time::Duration::from_secs(30 * 60);

static TURN_PERMITS: Semaphore = Semaphore::const_new(MAX_ACTIVE_TURNS);
static ACTIVE_TURNS: Mutex<[Option<Arc<RunOwner>>; MAX_ACTIVE_TURNS]> =
    Mutex::new([const { None }; MAX_ACTIVE_TURNS]);
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
    finished: watch::Sender<bool>,
    pending: Mutex<Option<PendingPermission>>,
    effect_gate: Mutex<()>,
}

impl RunOwner {
    fn new(session_id: SessionId) -> Arc<Self> {
        let (cancellation, _) = watch::channel(false);
        let (finished, _) = watch::channel(false);
        Arc::new(Self {
            session_id,
            run_id: NEXT_RUN_ID.fetch_add(1, Ordering::Relaxed),
            cancellation,
            finished,
            pending: Mutex::new(None),
            effect_gate: Mutex::new(()),
        })
    }

    pub fn cancellation_receiver(&self) -> watch::Receiver<bool> {
        self.cancellation.subscribe()
    }

    pub fn begin_effect(&self) -> Option<std::sync::MutexGuard<'_, ()>> {
        let gate = self.effect_gate.lock().ok()?;
        (!self.is_cancelled()).then_some(gate)
    }

    pub fn is_cancelled(&self) -> bool {
        *self.cancellation.borrow()
    }

    pub fn cancel(&self) {
        if let Ok(_gate) = self.effect_gate.lock() {
            self.cancellation.send_replace(true);
        } else {
            self.cancellation.send_replace(true);
        }
        self.clear_pending();
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

    pub fn install_pending(
        &self,
        request: PermissionRequest,
        intent: OperationIntent,
        broker: &PermissionBroker,
    ) -> Result<oneshot::Receiver<ApprovedPermission>, PermissionRequest> {
        let (reply, receiver) = oneshot::channel();
        let mut pending = self.pending.lock().map_err(|_| request.clone())?;
        if pending.is_some() || self.is_cancelled() {
            return Err(request);
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

#[derive(Clone, Copy)]
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
        let global = TURN_PERMITS
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

pub async fn cancel_and_wait(session_id: SessionId) {
    let Some(owner) = active(session_id) else {
        return;
    };
    owner.cancel();
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
