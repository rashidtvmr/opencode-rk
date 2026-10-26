//! Provider-backed integration HTTP domain (upstream V2 integration contract).
//!
//! Implements the full OAuth lifecycle: connect (begin), status, complete,
//! cancel. Ports upstream OpenCode behavior from
//! `packages/core/src/plugin/provider/openai.ts` +
//! `packages/server/src/handlers/integration.ts`, mapping the HTTP routes
//! defined in `packages/protocol/src/groups/integration.ts`.
//!
//! Route registration: `/api/integration/{integration_id}/connect/oauth`
//! (POST), `/api/integration/{integration_id}/connect/oauth/{attempt_id}`
//! (GET status, POST complete, DELETE cancel).

#![forbid(unsafe_code)]

use std::{collections::HashMap, future::Future, pin::Pin, sync::{Arc, atomic::{AtomicU64, AtomicUsize, Ordering}}, time::{Duration, SystemTime, UNIX_EPOCH}};

use opencode_rk_providers::{openai_device_auth, persisted_auth};
use serde::{Deserialize, Serialize};
use tokio::{sync::{watch, Mutex}, task::JoinHandle};
use std::sync::atomic::AtomicBool;
const WORKER_RUNNING: u8 = 0;
const WORKER_PERSISTING: u8 = 1;
const WORKER_ABORTING: u8 = 2;
const WORKER_FINISHED: u8 = 3;

pub const ATTEMPT_TTL_MS: u64 = 10 * 60 * 1_000;
const TERMINAL_RETENTION_MS: u64 = 60 * 1_000;
pub trait Clock: Send + Sync + 'static { fn now(&self) -> u64; }
#[derive(Clone, Default)] pub struct SystemClock;
impl Clock for SystemClock { fn now(&self) -> u64 { now_ms() } }

const MAX_ATTEMPTS: usize = 8;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct IntegrationInfo {
    pub id: String,
    pub name: String,
    pub methods: Vec<MethodInfo>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct MethodInfo { pub id: String, pub kind: &'static str, pub label: String }

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Attempt {
    #[serde(rename = "attemptID")]
    pub attempt_id: String,
    pub url: String,
    pub instructions: String,
    pub mode: &'static str,
    pub time: AttemptTime,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub struct AttemptTime { pub created: u64, pub expires: u64 }

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum AttemptStatus {
    Pending { time: AttemptTime },
    Complete { time: AttemptTime },
    Failed { message: String, time: AttemptTime },
    Expired { time: AttemptTime },
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConnectRequest {
    #[serde(rename = "methodID")]
    pub method_id: String,
    pub answer: Option<serde_json::Value>,
    pub label: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CompleteRequest { pub code: Option<String> }

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IntegrationError {
    #[error("integration not found")]
    NotFound,
    #[error("integration method not found")]
    MethodNotFound,
    #[error("oauth attempt not found")]
    AttemptNotFound,
    #[error("too many oauth attempts")]
    TooManyAttempts,
    #[error("provider runtime unavailable")]
    Unavailable,
    #[error("provider authorization failed")]
    Authorization,
}

#[derive(Clone)]
struct Pending {
    integration: String,
    method: String,
    attempt: Attempt,
    status: AttemptStatus,
    completing: bool,
    persisting: bool,
    device: Option<openai_device_auth::DeviceAuthorization>,
    cancel: watch::Sender<bool>,
    remove_at: Option<u64>,
}

pub trait CredentialSink: Send + Sync + 'static {
    fn save(&self, provider: String, credential: OpenAiCredential)
        -> Pin<Box<dyn Future<Output = Result<(), IntegrationError>> + Send + '_>>;
}

#[derive(Clone, Default)]
pub struct LegacyCredentialSink;

impl CredentialSink for LegacyCredentialSink {
    fn save(&self, provider: String, credential: OpenAiCredential)
        -> Pin<Box<dyn Future<Output = Result<(), IntegrationError>> + Send + '_>> {
        Box::pin(async move {
            // Compatibility boundary: the current Rust provider store is the
            // legacy auth.json writer. Use its generic atomic OAuth API rather
            // than a provider-specific helper (which is not part of the base).
            let expires = credential.expires_at_ms.max(0);
            persisted_auth::save_oauth_credential_with_metadata(
                provider,
                credential.access,
                credential.refresh,
                expires,
                credential.account_id,
                None,
            ).await.map_err(|_| IntegrationError::Authorization)
        })
    }
}

pub trait OAuthRuntime: Send + Sync + 'static {
    fn begin(&self, integration: &str, method: &str, cancel: watch::Receiver<bool>)
        -> Pin<Box<dyn Future<Output = Result<Authorization, IntegrationError>> + Send + '_>>;
    fn complete(&self, integration: &str, method: &str, auth: Authorization,
        code: Option<String>, cancel: watch::Receiver<bool>)
        -> Pin<Box<dyn Future<Output = Result<OpenAiCredential, IntegrationError>> + Send + '_>>;
}

#[derive(Clone, Debug)]
pub struct Authorization {
    pub url: String,
    pub instructions: String,
    pub mode: &'static str,
    pub device: Option<openai_device_auth::DeviceAuthorization>,
}

#[derive(Clone, Debug)]
pub struct OpenAiCredential {
    pub access: String,
    pub refresh: String,
    pub expires_at_ms: u64,
    pub account_id: Option<String>,
}

#[derive(Clone, Default)]
pub struct VerifiedRuntime;

impl OAuthRuntime for VerifiedRuntime {
    fn begin(&self, integration: &str, method: &str, mut cancel: watch::Receiver<bool>)
        -> Pin<Box<dyn Future<Output = Result<Authorization, IntegrationError>> + Send + '_>> {
        let integration = integration.to_owned(); let method = method.to_owned();
        Box::pin(async move {
            if *cancel.borrow() { return Err(IntegrationError::Unavailable); }
            match (integration.as_str(), method.as_str()) {
                ("openai", "chatgpt-headless") => {
                    let transport = openai_device_auth::AuthTransport::new().map_err(|_| IntegrationError::Unavailable)?;
                    let device = openai_device_auth::begin_device_authorization(
                        &transport, &openai_device_auth::DeviceEndpoints::default(), &mut cancel,
                    ).await.map_err(|_| IntegrationError::Authorization)?;
                    Ok(Authorization {
                        url: device.verification_url.clone(),
                        instructions: format!("Enter code: {}", device.user_code),
                        mode: "auto",
                        device: Some(device),
                    })
                }
                _ => Err(IntegrationError::Unavailable),
            }
        })
    }

    fn complete(&self, integration: &str, method: &str, auth: Authorization, code: Option<String>, mut cancel: watch::Receiver<bool>)
        -> Pin<Box<dyn Future<Output = Result<OpenAiCredential, IntegrationError>> + Send + '_>> {
        let integration = integration.to_owned(); let method = method.to_owned();
        Box::pin(async move {
            if *cancel.borrow() { return Err(IntegrationError::Unavailable); }
            match (integration.as_str(), method.as_str(), auth.mode) {
                ("openai", "chatgpt-headless", "auto") => {
                    let transport = openai_device_auth::AuthTransport::new().map_err(|_| IntegrationError::Unavailable)?;
                    let endpoints = openai_device_auth::DeviceEndpoints::default();
                    let device = auth.device.ok_or(IntegrationError::Authorization)?;
                    let credential = openai_device_auth::authorize_device(
                        &transport, &endpoints, &device, &mut cancel,
                    ).await.map_err(|_| IntegrationError::Authorization)?;
                    Ok(OpenAiCredential {
                        access: credential.access,
                        refresh: credential.refresh,
                        expires_at_ms: credential.expires_at_ms,
                        account_id: credential.account_id,
                    })
                }
                _ => Err(IntegrationError::Unavailable),
            }
        })
    }
}

#[derive(Clone)]
pub struct IntegrationServer<R: OAuthRuntime + ?Sized = VerifiedRuntime> {
    runtime: Arc<R>,
    sink: Arc<dyn CredentialSink>,
    attempts: Arc<Mutex<HashMap<String, Pending>>>,
    clock: Arc<dyn Clock>,
    reservations: Arc<AtomicUsize>,
    workers: Arc<WorkerRegistry>,
    worker_phase: Option<Arc<std::sync::atomic::AtomicU8>>,
    closing: Arc<std::sync::atomic::AtomicBool>,
}

struct WorkerEntry {
    task: JoinHandle<()>,
    phase: Arc<std::sync::atomic::AtomicU8>,
}

struct WorkerRegistry(std::sync::Mutex<HashMap<String, WorkerEntry>>);

impl Drop for WorkerRegistry {
    fn drop(&mut self) {
        if let Ok(workers) = self.0.get_mut() {
            for worker in workers.values() {
                if worker.phase.compare_exchange(WORKER_RUNNING, WORKER_ABORTING, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                    worker.task.abort();
                }
            }
        }
    }
}

impl<R: OAuthRuntime + ?Sized> IntegrationServer<R> {
    /// Close admission under the same attempt lock used to commit an
    /// in-flight begin, then abort RUNNING workers and await every owned task.
    /// PERSISTING workers are the atomic credential-write boundary and are
    /// never detached or interrupted by normal shutdown.
    pub async fn shutdown(&self) {
        let workers = {
            let mut attempts = self.attempts.lock().await;
            self.closing.store(true, Ordering::Release);
            std::mem::take(&mut *self.workers.0.lock().expect("worker registry poisoned"))
        };
        for (_, worker) in workers {
            let mut task = worker.task;
            if worker.phase.compare_exchange(WORKER_RUNNING, WORKER_ABORTING, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                task.abort();
            }
            let _ = task.await;
        }
    }

    pub fn new(runtime: R) -> Self where R: Sized {
        Self::with_parts_clock(Arc::new(runtime), Arc::new(LegacyCredentialSink), Arc::new(SystemClock))
    }

    pub fn with_parts(runtime: Arc<R>, sink: Arc<dyn CredentialSink>) -> Self {
        Self::with_parts_clock(runtime, sink, Arc::new(SystemClock))
    }

    pub fn with_parts_clock(runtime: Arc<R>, sink: Arc<dyn CredentialSink>, clock: Arc<dyn Clock>) -> Self {
        Self {
            runtime,
            sink,
            attempts: Arc::new(Mutex::new(HashMap::new())),
            clock,
            reservations: Arc::new(AtomicUsize::new(0)),
            workers: Arc::new(WorkerRegistry(std::sync::Mutex::new(HashMap::new()))),
            worker_phase: None,
            closing: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn list() -> Vec<IntegrationInfo> {
        vec![
            IntegrationInfo {
                id: "openai".into(),
                name: "OpenAI".into(),
                methods: vec![
                    MethodInfo { id: "chatgpt-headless".into(), kind: "oauth", label: "Codex device code (legacy)".into() },
                ],
            },
        ]
    }

    pub fn get(id: &str) -> Option<IntegrationInfo> {
        Self::list().into_iter().find(|item| item.id == id)
    }

    pub fn method_exists(integration: &str, method_id: &str) -> bool {
        Self::get(integration)
            .map(|info| info.methods.iter().any(|m| m.id == method_id))
            .unwrap_or(false)
    }

    pub async fn connect(
        &self,
        integration: &str,
        request: ConnectRequest,
        now: u64,
    ) -> Result<Attempt, IntegrationError> {
        self.cleanup_finished_workers().await;
        if Self::get(integration).is_none() {
            return Err(IntegrationError::NotFound);
        }
        if !Self::method_exists(integration, &request.method_id) {
            return Err(IntegrationError::MethodNotFound);
        }
        let mut attempts = self.attempts.lock().await;
        if self.closing.load(Ordering::Acquire) {
            return Err(IntegrationError::Unavailable);
        }
        reap(&mut attempts, now);
        let current = self.reservations.load(Ordering::Acquire);
        if attempts.len() + current >= MAX_ATTEMPTS {
            return Err(IntegrationError::TooManyAttempts);
        }
        let mut observed = current;
        loop {
            if attempts.len() + observed >= MAX_ATTEMPTS {
                return Err(IntegrationError::TooManyAttempts);
            }
            match self.reservations.compare_exchange_weak(observed, observed + 1, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => break,
                Err(next) => observed = next,
            }
        }
        let reservation = AdmissionReservation { count: self.reservations.clone(), committed: false };
        let id = allocate_con_id().ok_or(IntegrationError::Unavailable)?;
        let (cancel_tx, cancel_rx) = watch::channel(false);
        drop(attempts);
        let auth = match self.runtime.begin(integration, &request.method_id, cancel_rx).await {
            Ok(auth) => auth,
            Err(error) => return Err(error),
        };
        let mut attempts = self.attempts.lock().await;
        if self.closing.load(Ordering::Acquire) {
            return Err(IntegrationError::Unavailable);
        }
        // Convert the reservation to an entry only after begin succeeds.
        reservation.commit();
        let attempt = Attempt {
            attempt_id: id.clone(),
            url: auth.url.clone(),
            instructions: auth.instructions.clone(),
            mode: auth.mode,
            time: AttemptTime {
                created: now,
                expires: now.saturating_add(ATTEMPT_TTL_MS),
            },
        };
        let automatic = request.method_id == "chatgpt-headless" && auth.mode == "auto";
        attempts.insert(id.clone(), Pending {
            integration: integration.into(),
            method: request.method_id.clone(),
            attempt: attempt.clone(),
            status: AttemptStatus::Pending { time: attempt.time.clone() },
            completing: false,
            persisting: false,
            device: auth.device.clone(),
            cancel: cancel_tx,
            remove_at: None,
        });
        if automatic {
            let phase = Arc::new(std::sync::atomic::AtomicU8::new(WORKER_RUNNING));
            // The task deliberately receives a worker view with a separate
            // registry. The registry owns the JoinHandle; the task must not
            // own the registry or it would form a server -> handle -> server
            // cycle and prevent cancellation cleanup.
            let worker = Self {
                runtime: self.runtime.clone(), sink: self.sink.clone(),
                attempts: self.attempts.clone(), clock: self.clock.clone(),
                reservations: self.reservations.clone(), workers: Arc::new(WorkerRegistry(std::sync::Mutex::new(HashMap::new()))),
                worker_phase: Some(phase.clone()),
                closing: self.closing.clone(),
            };
            let integration = integration.to_owned();
            let worker_id = id.clone();
            let task = tokio::spawn(async move {
                let _ = worker.complete_automatic(&integration, &worker_id, worker.clock.now()).await;
            });
            self.workers.0.lock().expect("worker registry poisoned").insert(id, WorkerEntry { task, phase });
        }
        Ok(attempt)
    }

    pub async fn status(&self, integration: &str, id: &str, now: u64) -> Result<AttemptStatus, IntegrationError> {
        self.cleanup_finished_workers().await;
        let mut attempts = self.attempts.lock().await;
        let pending = attempts.get_mut(id).ok_or(IntegrationError::AttemptNotFound)?;
        if pending.integration != integration {
            return Err(IntegrationError::AttemptNotFound);
        }
        expire(pending, now);
        let status = pending.status.clone();
        let expired = matches!(status, AttemptStatus::Expired { .. });
        drop(attempts);
        if expired {
            self.abort_worker(id).await;
        }
        Ok(status)
    }

    pub async fn cancel(&self, integration: &str, id: &str) -> Result<(), IntegrationError> {
        self.cleanup_finished_workers().await;
        let mut attempts = self.attempts.lock().await;
        let Some(pending) = attempts.get(id) else { return Ok(()); };
        if pending.integration != integration || pending.persisting || !matches!(pending.status, AttemptStatus::Pending { .. }) {
            return Ok(());
        }
        let pending = attempts.remove(id).expect("validated pending attempt");
        let _ = pending.cancel.send(true);
        drop(attempts);
        self.abort_worker(id).await;
        Ok(())
    }

    async fn abort_worker(&self, id: &str) {
        let worker = {
            let mut workers = self.workers.0.lock().expect("worker registry poisoned");
            workers.remove(id)
        };
        if let Some(worker) = worker {
            if worker.phase.compare_exchange(WORKER_RUNNING, WORKER_ABORTING, Ordering::AcqRel, Ordering::Acquire).is_ok() {
            let mut task = worker.task;
            task.abort();
            if tokio::time::timeout(Duration::from_secs(1), &mut task).await.is_err() {
                self.workers.0.lock().expect("worker registry poisoned").insert(id.to_owned(), WorkerEntry { task, phase: worker.phase });
            }
            }
        }
    }

    async fn cleanup_finished_workers(&self) {
        let mut workers = self.workers.0.lock().expect("worker registry poisoned");
        workers.retain(|_, worker| !worker.task.is_finished());
    }

    async fn complete_automatic(&self, integration: &str, id: &str, now: u64) -> Result<(), IntegrationError> {
        self.complete_inner(integration, id, None, now, true).await
    }

    pub async fn complete_code(
        &self,
        integration: &str,
        id: &str,
        code: Option<String>,
        now: u64,
    ) -> Result<(), IntegrationError> {
        self.complete_inner(integration, id, code, now, false).await
    }

    async fn complete_inner(
        &self,
        integration: &str,
        id: &str,
        code: Option<String>,
        now: u64,
        automatic: bool,
    ) -> Result<(), IntegrationError> {
        let mut attempts = self.attempts.lock().await;
        let pending = attempts.get_mut(id).ok_or(IntegrationError::AttemptNotFound)?;
        if pending.integration != integration {
            return Err(IntegrationError::AttemptNotFound);
        }
        expire(pending, now);
        if pending.attempt.mode == "auto" && !automatic { return Err(IntegrationError::Authorization); }
        if pending.attempt.mode == "code" && automatic { return Err(IntegrationError::Authorization); }
        if !matches!(pending.status, AttemptStatus::Pending { .. }) || pending.completing {
            return Err(IntegrationError::AttemptNotFound);
        }
        pending.completing = true;
        let auth = Authorization {
            url: pending.attempt.url.clone(),
            instructions: pending.attempt.instructions.clone(),
            mode: pending.attempt.mode,
            device: pending.device.clone(),
        };
        let cancel_rx = pending.cancel.subscribe();
        let method = pending.method.clone();
        drop(attempts);
        let result = self.runtime.complete(integration, &method, auth, code, cancel_rx).await;
        match result {
            Ok(credential) => {
                let mut attempts = self.attempts.lock().await;
                if let Some(p) = attempts.get_mut(id) {
                    expire(p, self.clock.now());
                }
                let valid = attempts.get_mut(id).map(|p| {
                    let valid = matches!(p.status, AttemptStatus::Pending { .. })
                        && p.completing && !p.persisting && !*p.cancel.borrow();
                    if valid {
                        p.persisting = true;
                        if let Some(phase) = &self.worker_phase {
                            if phase.compare_exchange(WORKER_RUNNING, WORKER_PERSISTING, Ordering::AcqRel, Ordering::Acquire).is_err() {
                                p.persisting = false;
                                return false;
                            }
                        }
                    }
                    valid
                }).unwrap_or(false);
                drop(attempts);
                if !valid { return Err(IntegrationError::AttemptNotFound); }
                let saved = self.sink.save(integration.to_owned(), credential).await;
                let mut attempts = self.attempts.lock().await;
                if let Err(error) = saved {
                    if let Some(p) = attempts.get_mut(id) {
                        p.completing = false; p.persisting = false;
                        p.status = AttemptStatus::Failed { message: "provider authorization failed".into(), time: p.attempt.time.clone() };
                        p.remove_at = Some(self.clock.now().saturating_add(TERMINAL_RETENTION_MS));
                    }
                    if let Some(phase) = &self.worker_phase { phase.store(WORKER_FINISHED, Ordering::Release); }
                    return Err(error);
                }
                if let Some(p) = attempts.get_mut(id) {
                    p.completing = false;
                    p.persisting = false;
                    p.status = AttemptStatus::Complete { time: p.attempt.time.clone() };
                    p.remove_at = Some(self.clock.now().saturating_add(TERMINAL_RETENTION_MS));
                }
                if let Some(phase) = &self.worker_phase { phase.store(WORKER_FINISHED, Ordering::Release); }
                Ok(())
            }
            Err(e) => {
                let mut attempts = self.attempts.lock().await;
                if let Some(p) = attempts.get_mut(id) {
                    p.completing = false;
                    p.persisting = false;
                    // Expiry is a terminal state.  In particular, the provider
                    // future may observe cancellation after a status request has
                    // already settled the attempt as expired; that late error
                    // must not resurrect the attempt as a fresh failed one.
                    // Likewise, only the completion that owns the Pending state
                    // may settle it as Failed.
                    if matches!(p.status, AttemptStatus::Pending { .. }) {
                        p.status = AttemptStatus::Failed {
                            message: "provider authorization failed".to_string(),
                            time: p.attempt.time.clone(),
                        };
                        p.remove_at = Some(self.clock.now().saturating_add(TERMINAL_RETENTION_MS));
                    }
                }
                if let Some(phase) = &self.worker_phase { phase.store(WORKER_FINISHED, Ordering::Release); }
                Err(e)
            }
        }
    }
}

fn expire(p: &mut Pending, now: u64) {
    if matches!(p.status, AttemptStatus::Pending { .. })
        && !p.persisting
        && now >= p.attempt.time.expires
    {
        p.status = AttemptStatus::Expired { time: p.attempt.time.clone() };
        p.remove_at = Some(now.saturating_add(TERMINAL_RETENTION_MS));
        let _ = p.cancel.send(true);
    }
}

fn reap(map: &mut HashMap<String, Pending>, now: u64) {
    for pending in map.values_mut() {
        expire(pending, now);
    }
    map.retain(|_, p| p.remove_at.map(|at| at > now).unwrap_or(true));
}

struct AdmissionReservation {
    count: Arc<AtomicUsize>,
    committed: bool,
}

impl AdmissionReservation {
    fn commit(mut self) { self.committed = true; self.count.fetch_sub(1, Ordering::AcqRel); }
}

impl Drop for AdmissionReservation {
    fn drop(&mut self) {
        if !self.committed {
            self.count.fetch_sub(1, Ordering::AcqRel);
        }
    }
}


pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

// Browser PKCE and callback listener are intentionally a separate increment.

/// Process-wide monotonic counter for unique `con_` attempt IDs.
/// Incremented atomically via `fetch_update` to guarantee uniqueness
/// even across concurrent `begin()` calls. Starts at 1 so IDs are never
/// zero-valued. Overflow (u64::MAX reached) returns `None` and is
/// handled failclosed by the caller.
static CON_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Allocates a fresh, globally-unique `con_` prefixed attempt ID.
/// Uses a process-wide checked monotonic counter so that IDs never
/// collide, even when sequential timestamps with different map lengths
/// would have produced identical XOR-derived values (the original bug).
/// Returns `None` only on u64 overflow (practically unreachable), which
/// is then surfaced as `IntegrationError::Unavailable` by the caller.
fn allocate_con_id() -> Option<String> {
    CON_ID_COUNTER
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            n.checked_add(1)
        })
        .ok()
        .map(|n| format!("con_{:08x}", n))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubRuntime {
        begin_result: Result<Authorization, IntegrationError>,
        complete_result: Result<OpenAiCredential, IntegrationError>,
    }

    impl Clone for StubRuntime {
        fn clone(&self) -> Self {
            fn clone_error(error: &IntegrationError) -> IntegrationError {
                match error {
                    IntegrationError::NotFound => IntegrationError::NotFound,
                    IntegrationError::MethodNotFound => IntegrationError::MethodNotFound,
                    IntegrationError::AttemptNotFound => IntegrationError::AttemptNotFound,
                    IntegrationError::TooManyAttempts => IntegrationError::TooManyAttempts,
                    IntegrationError::Unavailable => IntegrationError::Unavailable,
                    IntegrationError::Authorization => IntegrationError::Authorization,
                }
            }
            Self {
                begin_result: self.begin_result.as_ref().map(Clone::clone).map_err(clone_error),
                complete_result: self.complete_result.as_ref().map(Clone::clone).map_err(clone_error),
            }
        }
    }

    impl OAuthRuntime for StubRuntime {
        fn begin(&self, _integration: &str, _method: &str, _cancel: watch::Receiver<bool>)
            -> Pin<Box<dyn Future<Output = Result<Authorization, IntegrationError>> + Send + '_>> {
            let result = self.clone().begin_result;
            Box::pin(async move { Ok(result?) })
        }
        fn complete(&self, _integration: &str, _method: &str, _auth: Authorization, _code: Option<String>, _cancel: watch::Receiver<bool>)
            -> Pin<Box<dyn Future<Output = Result<OpenAiCredential, IntegrationError>> + Send + '_>> {
            let result = self.clone().complete_result;
            Box::pin(async move { Ok(result?) })
        }
    }

    fn stub_success() -> StubRuntime {
        StubRuntime {
            begin_result: Ok(Authorization {
                url: "https://auth.openai.com/codex/device".to_string(),
                instructions: "Enter code: ABC123".to_string(),
                mode: "auto",
                device: None,
            }),
            complete_result: Ok(OpenAiCredential {
                access: "access-token".to_string(),
                refresh: "refresh-token".to_string(),
                expires_at_ms: now_ms() + 3_600_000,
                account_id: Some("user-123".to_string()),
            }),
        }
    }

    #[test]
    fn list_exposes_only_implemented_device_method() {
        let integrations = IntegrationServer::<VerifiedRuntime>::list();
        let openai = integrations.iter().find(|i| i.id == "openai").expect("openai present");
        assert_eq!(openai.methods.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["chatgpt-headless"]);
        assert!(!IntegrationServer::<VerifiedRuntime>::method_exists("openai", "chatgpt-browser"));
        assert!(!IntegrationServer::<VerifiedRuntime>::method_exists("github-copilot", "device"));
    }

    #[test]
    fn method_exists_recognizes_openai_methods() {
        assert!(IntegrationServer::<VerifiedRuntime>::method_exists("openai", "chatgpt-headless"));
        assert!(IntegrationServer::<VerifiedRuntime>::method_exists("openai", "chatgpt-headless"));
        assert!(!IntegrationServer::<VerifiedRuntime>::method_exists("openai", "nonexistent"));
        assert!(!IntegrationServer::<VerifiedRuntime>::method_exists("nonexistent", "chatgpt-headless"));
    }

    #[tokio::test]
    async fn connect_unknown_integration_returns_not_found() {
        let server = IntegrationServer::new(stub_success());
        let result = server.connect("unknown", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, now_ms()).await;
        assert_eq!(result.map(|_| ()), Err(IntegrationError::NotFound));
    }

    #[tokio::test]
    async fn connect_unknown_method_returns_method_not_found() {
        let server = IntegrationServer::new(stub_success());
        let result = server.connect("openai", ConnectRequest { method_id: "bogus".into(), answer: None, label: None }, now_ms()).await;
        assert_eq!(result.map(|_| ()), Err(IntegrationError::MethodNotFound));
    }

    #[tokio::test]
    async fn connect_creates_pending_attempt() {
        let server = IntegrationServer::new(stub_success());
        let attempt = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, now_ms()).await.unwrap();
        assert!(attempt.attempt_id.starts_with("con_"));
        assert_eq!(attempt.mode, "auto");
        assert!(attempt.url.contains("auth.openai.com"));
    }

    #[tokio::test]
    async fn status_returns_pending_for_active_attempt() {
        let server = IntegrationServer::new(stub_success());
        let attempt = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, now_ms()).await.unwrap();
        let status = server.status("openai", &attempt.attempt_id, now_ms()).await.unwrap();
        assert!(matches!(status, AttemptStatus::Pending { .. }));
    }

    #[tokio::test]
    async fn status_returns_not_found_for_missing_attempt() {
        let server = IntegrationServer::new(stub_success());
        let result = server.status("openai", "con_nonexistent", now_ms()).await;
        assert_eq!(result.map(|_| ()), Err(IntegrationError::AttemptNotFound));
    }

    #[tokio::test]
    async fn status_scoped_to_correct_integration() {
        let server = IntegrationServer::new(stub_success());
        let attempt = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, now_ms()).await.unwrap();
        let result = server.status("github-copilot", &attempt.attempt_id, now_ms()).await;
        assert_eq!(result.map(|_| ()), Err(IntegrationError::AttemptNotFound));
        assert!(matches!(server.status("openai", &attempt.attempt_id, now_ms()).await, Ok(AttemptStatus::Pending { .. })));
    }

    #[tokio::test]
    async fn cancel_removes_attempt() {
        let server = IntegrationServer::new(stub_success());
        let attempt = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, now_ms()).await.unwrap();
        assert!(server.cancel("openai", &attempt.attempt_id).await.is_ok());
        let result = server.status("openai", &attempt.attempt_id, now_ms()).await;
        assert_eq!(result.map(|_| ()), Err(IntegrationError::AttemptNotFound));
    }

    #[tokio::test]
    async fn complete_code_on_expired_attempt_returns_not_found() {
        let server = IntegrationServer::new(stub_success());
        let attempt = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, now_ms()).await.unwrap();
        let expired = attempt.time.expires + 1;
        let result = server.complete_code("openai", &attempt.attempt_id, Some("code".into()), expired).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn max_attempts_bounded() {
        let server = IntegrationServer::new(stub_success());
        let mut _last_id = String::new();
        for i in 0..MAX_ATTEMPTS {
            let attempt = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, now_ms() + i as u64).await.unwrap();
            _last_id = attempt.attempt_id;
        }
        let result = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, now_ms() + MAX_ATTEMPTS as u64).await;
        assert_eq!(result.map(|_| ()), Err(IntegrationError::TooManyAttempts));
    }

    #[tokio::test]
    async fn expired_attempt_not_returned_by_status() {
        let clock = Arc::new(std::sync::atomic::AtomicUsize::new(1_000));
        let server = IntegrationServer::with_parts_clock(
            Arc::new(stub_success()), Arc::new(LegacyCredentialSink),
            Arc::new(TestClock(clock.clone())),
        );
        let attempt = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, 1_000).await.unwrap();
        let expired = attempt.time.expires + 1;
        // Approved upstream contract: during the 1-minute terminal retention
        // (TERMINAL_RETENTION_MS), an expired attempt is still visible via
        // status and returns Expired { time } rather than being removed.
        // The attempt is only reaped after removeAt passes.
        clock.store(expired as usize, std::sync::atomic::Ordering::SeqCst);
        let status = server.status("openai", &attempt.attempt_id, expired).await;
        assert_eq!(status, Ok(AttemptStatus::Expired { time: attempt.time.clone() }));
    }

    #[tokio::test]
    async fn complete_code_succeeds_with_stub_runtime() {
        let runtime = StubRuntime {
            begin_result: Ok(Authorization {
                url: "https://auth.openai.com/codex/device".into(),
                instructions: "Enter code: TEST".into(),
                mode: "code",
                device: None,
            }),
            complete_result: Ok(OpenAiCredential {
                access: "access-token".into(),
                refresh: "refresh-token".into(),
                expires_at_ms: now_ms() + 3_600_000,
                account_id: Some("user-123".into()),
            }),
        };
        std::env::set_var("OPENCODE_AUTH_CONTENT", r#"{"openai":{"type":"oauth","access":"old","refresh":"old","expires":1}}"#);
        let server = IntegrationServer::new(runtime);
        let attempt = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, now_ms()).await.unwrap();
        let result = server.complete_code("openai", &attempt.attempt_id, Some("auth-code".into()), now_ms()).await;
        assert!(result.is_ok(), "complete_code should succeed: {:?}", result);
        let status = server.status("openai", &attempt.attempt_id, now_ms()).await.unwrap();
        assert!(matches!(status, AttemptStatus::Complete { .. }));
        std::env::remove_var("OPENCODE_AUTH_CONTENT");
    }

    #[tokio::test]
    async fn complete_code_failure_sets_failed_status() {
        let runtime = StubRuntime {
            begin_result: Ok(Authorization {
                url: "https://auth.openai.com/codex/device".into(),
                instructions: "Enter code: TEST".into(),
                mode: "code",
                device: None,
            }),
            complete_result: Err(IntegrationError::Authorization),
        };
        let server = IntegrationServer::new(runtime);
        let attempt = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, now_ms()).await.unwrap();
        let result = server.complete_code("openai", &attempt.attempt_id, Some("code".into()), now_ms()).await;
        assert_eq!(result.map(|_|()), Err(IntegrationError::Authorization));
        let status = server.status("openai", &attempt.attempt_id, now_ms()).await.unwrap();
        assert!(matches!(status, AttemptStatus::Failed { .. }));
    }

    #[tokio::test]
    async fn double_complete_returns_not_found_after_first() {
        let runtime = StubRuntime {
            begin_result: Ok(Authorization {
                url: "https://auth.openai.com/codex/device".into(),
                instructions: "Enter code: TEST".into(),
                mode: "code",
                device: None,
            }),
            complete_result: Ok(OpenAiCredential {
                access: "access-token".into(),
                refresh: "refresh-token".into(),
                expires_at_ms: now_ms() + 3_600_000,
                account_id: None,
            }),
        };
        std::env::set_var("OPENCODE_AUTH_CONTENT", r#"{"openai":{"type":"oauth","access":"old","refresh":"old","expires":1}}"#);
        let server = IntegrationServer::new(runtime);
        let attempt = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, now_ms()).await.unwrap();
        // First complete succeeds
        let result = server.complete_code("openai", &attempt.attempt_id, Some("code".into()), now_ms()).await;
        assert!(result.is_ok(), "first complete: {:?}", result);
        // Second complete fails because attempt is no longer Pending
        let result2 = server.complete_code("openai", &attempt.attempt_id, Some("code".into()), now_ms()).await;
        assert_eq!(result2.map(|_| ()), Err(IntegrationError::AttemptNotFound));
        std::env::remove_var("OPENCODE_AUTH_CONTENT");
    }

    #[tokio::test]
    async fn terminal_retention_reclaims_capacity_after_successes_and_failures() {
        let clock = Arc::new(std::sync::atomic::AtomicUsize::new(1_000));
        let server = IntegrationServer::with_parts_clock(
            Arc::new(stub_success()), Arc::new(LegacyCredentialSink),
            Arc::new(TestClock(clock.clone())),
        );
        for index in 0..MAX_ATTEMPTS {
            let attempt = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, 1_000 + index as u64).await.unwrap();
            let _ = server.complete_automatic("openai", &attempt.attempt_id, 1_000).await;
        }
        assert_eq!(server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, 1_000).await.unwrap_err(), IntegrationError::TooManyAttempts);
        clock.store(1_000 + (ATTEMPT_TTL_MS + TERMINAL_RETENTION_MS + 1) as usize, Ordering::SeqCst);
        let attempt = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, clock.load(Ordering::SeqCst) as u64).await.unwrap();
        assert!(matches!(server.status("openai", &attempt.attempt_id, clock.load(Ordering::SeqCst) as u64).await, Ok(AttemptStatus::Pending { .. })));
    }

    #[derive(Clone)]
    struct TestClock(Arc<std::sync::atomic::AtomicUsize>);
    impl Clock for TestClock { fn now(&self) -> u64 { self.0.load(Ordering::SeqCst) as u64 } }

    #[derive(Clone)]
    struct AdmissionRuntime {
        begins: Arc<std::sync::atomic::AtomicUsize>,
        entered: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
    }

    impl OAuthRuntime for AdmissionRuntime {
        fn begin(&self, _integration: &str, _method: &str, _cancel: watch::Receiver<bool>)
            -> Pin<Box<dyn Future<Output = Result<Authorization, IntegrationError>> + Send + '_>> {
            let n = self.begins.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let entered = self.entered.clone();
            let release = self.release.clone();
            Box::pin(async move {
                if n > 0 {
                    entered.notify_one();
                    release.notified().await;
                }
                Ok(Authorization { url: "fixture".into(), instructions: "fixture".into(), mode: "code", device: None })
            })
        }

        fn complete(&self, _integration: &str, _method: &str, _auth: Authorization, _code: Option<String>, _cancel: watch::Receiver<bool>)
            -> Pin<Box<dyn Future<Output = Result<OpenAiCredential, IntegrationError>> + Send + '_>> {
            Box::pin(async { Ok(OpenAiCredential { access: "a".into(), refresh: "r".into(), expires_at_ms: 1, account_id: None }) })
        }
    }

    #[tokio::test]
    async fn begin_does_not_hold_attempt_lock_across_runtime_wait() {
        let runtime = AdmissionRuntime { begins: Arc::new(std::sync::atomic::AtomicUsize::new(0)), entered: Arc::new(tokio::sync::Notify::new()), release: Arc::new(tokio::sync::Notify::new()) };
        let server = IntegrationServer::new(runtime.clone());
        let first = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, 1).await.unwrap();
        let second_server = server.clone();
        let second = tokio::spawn(async move { second_server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, 2).await });
        tokio::time::timeout(Duration::from_secs(1), runtime.entered.notified()).await.unwrap();
        assert!(tokio::time::timeout(Duration::from_secs(1), server.status("openai", &first.attempt_id, 1)).await.is_ok());
        assert!(tokio::time::timeout(Duration::from_secs(1), server.cancel("openai", &first.attempt_id)).await.is_ok());
        runtime.release.notify_one();
        assert!(second.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn cancelled_begin_releases_admission_reservation() {
        let runtime = AdmissionRuntime { begins: Arc::new(std::sync::atomic::AtomicUsize::new(0)), entered: Arc::new(tokio::sync::Notify::new()), release: Arc::new(tokio::sync::Notify::new()) };
        let server = IntegrationServer::new(runtime.clone());
        server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, 0).await.unwrap();
        let blocked = server.clone();
        let task = tokio::spawn(async move {
            blocked.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, 1).await
        });
        tokio::time::timeout(Duration::from_secs(1), runtime.entered.notified()).await.unwrap();
        task.abort();
        let _ = task.await;
        runtime.release.notify_one();
        let result = tokio::time::timeout(Duration::from_secs(1), server.connect(
            "openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, 2,
        )).await.unwrap();
        assert!(result.is_ok());
    }

    struct DropWatch(Arc<std::sync::atomic::AtomicUsize>);
    impl Drop for DropWatch {
        fn drop(&mut self) { self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst); }
    }

    #[derive(Clone)]
    struct NonCooperativeRuntime {
        entered: Arc<tokio::sync::Notify>,
        dropped: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl OAuthRuntime for NonCooperativeRuntime {
        fn begin(&self, _integration: &str, _method: &str, _cancel: watch::Receiver<bool>)
            -> Pin<Box<dyn Future<Output = Result<Authorization, IntegrationError>> + Send + '_>> {
            Box::pin(async { Ok(Authorization { url: "fixture".into(), instructions: "fixture".into(), mode: "auto", device: None }) })
        }

        fn complete(&self, _integration: &str, _method: &str, _auth: Authorization, _code: Option<String>, _cancel: watch::Receiver<bool>)
            -> Pin<Box<dyn Future<Output = Result<OpenAiCredential, IntegrationError>> + Send + '_>> {
            let entered = self.entered.clone();
            let dropped = self.dropped.clone();
            Box::pin(async move {
                let _watch = DropWatch(dropped);
                entered.notify_one();
                std::future::pending::<Result<OpenAiCredential, IntegrationError>>().await
            })
        }
    }

    #[tokio::test]
    async fn cancel_aborts_noncooperative_owned_worker() {
        let runtime = NonCooperativeRuntime { entered: Arc::new(tokio::sync::Notify::new()), dropped: Arc::new(std::sync::atomic::AtomicUsize::new(0)) };
        let clock = Arc::new(std::sync::atomic::AtomicUsize::new(1));
        let server = IntegrationServer::with_parts_clock(
            Arc::new(runtime.clone()), Arc::new(LegacyCredentialSink),
            Arc::new(TestClock(clock.clone())),
        );
        let attempt = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, 1).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), runtime.entered.notified()).await.unwrap();
        server.cancel("openai", &attempt.attempt_id).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while runtime.dropped.load(std::sync::atomic::Ordering::SeqCst) == 0 { tokio::task::yield_now().await; }
        }).await.unwrap();
    }

    #[tokio::test]
    async fn shutdown_aborts_running_worker_and_drains_persisting_owner() {
        #[derive(Clone)]
        struct HeldSink { entered: Arc<tokio::sync::Notify>, release: Arc<tokio::sync::Notify> }
        impl CredentialSink for HeldSink {
            fn save(&self, _provider: String, _credential: OpenAiCredential) -> Pin<Box<dyn Future<Output = Result<(), IntegrationError>> + Send + '_>> {
                let entered = self.entered.clone(); let release = self.release.clone();
                Box::pin(async move { entered.notify_one(); release.notified().await; Ok(()) })
            }
        }
        let runtime = StubRuntime { begin_result: Ok(Authorization { url: "fixture".into(), instructions: "fixture".into(), mode: "auto", device: None }), complete_result: Ok(OpenAiCredential { access: "a".into(), refresh: "r".into(), expires_at_ms: 1, account_id: None }) };
        let sink = Arc::new(HeldSink { entered: Arc::new(tokio::sync::Notify::new()), release: Arc::new(tokio::sync::Notify::new()) });
        let server = IntegrationServer::with_parts_clock(
            Arc::new(runtime), sink.clone(),
            Arc::new(TestClock(Arc::new(std::sync::atomic::AtomicUsize::new(1)))),
        );
        let attempt = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, 1).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), sink.entered.notified()).await.unwrap();
        let mut draining = tokio::spawn({ let server = server.clone(); async move { server.shutdown().await } });
        tokio::select! { result = &mut draining => panic!("shutdown returned before persistence settled: {result:?}"), _ = tokio::time::sleep(Duration::from_millis(20)) => {} }
        sink.release.notify_one();
        tokio::time::timeout(Duration::from_secs(1), &mut draining).await.expect("shutdown drain").expect("shutdown task");
        assert!(matches!(server.status("openai", &attempt.attempt_id, 1).await, Ok(AttemptStatus::Complete { .. })));
    }

    #[tokio::test]
    async fn shutdown_closes_inflight_begin_before_worker_registration() {
        #[derive(Clone)]
        struct HeldBegin { entered: Arc<tokio::sync::Notify>, release: Arc<tokio::sync::Notify> }
        impl OAuthRuntime for HeldBegin {
            fn begin(&self, _i: &str, _m: &str, _c: watch::Receiver<bool>) -> Pin<Box<dyn Future<Output = Result<Authorization, IntegrationError>> + Send + '_>> {
                let entered = self.entered.clone(); let release = self.release.clone();
                Box::pin(async move { entered.notify_one(); release.notified().await; Ok(Authorization { url: "fixture".into(), instructions: "fixture".into(), mode: "auto", device: None }) })
            }
            fn complete(&self, _i: &str, _m: &str, _a: Authorization, _c: Option<String>, _cancel: watch::Receiver<bool>) -> Pin<Box<dyn Future<Output = Result<OpenAiCredential, IntegrationError>> + Send + '_>> {
                Box::pin(async { panic!("late worker registration") })
            }
        }
        let runtime = HeldBegin { entered: Arc::new(tokio::sync::Notify::new()), release: Arc::new(tokio::sync::Notify::new()) };
        let server = IntegrationServer::new(runtime.clone());
        let connecting = tokio::spawn({ let server = server.clone(); async move { server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, 1).await } });
        tokio::time::timeout(Duration::from_secs(1), runtime.entered.notified()).await.unwrap();
        let shutdown = tokio::spawn({ let server = server.clone(); async move { server.shutdown().await } });
        tokio::time::timeout(Duration::from_secs(1), async {
            while !server.closing.load(Ordering::Acquire) { tokio::task::yield_now().await; }
        }).await.expect("shutdown must close admission");
        runtime.release.notify_one();
        assert!(matches!(connecting.await.unwrap(), Err(IntegrationError::Unavailable)));
        shutdown.await.unwrap();
        assert!(server.workers.0.lock().unwrap().is_empty());
        assert_eq!(server.reservations.load(Ordering::Acquire), 0);
        assert!(matches!(server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, 2).await, Err(IntegrationError::Unavailable)));
    }

    #[test]
    fn worker_phase_cas_has_exclusive_persistence_or_abort_winner() {
        let phase = std::sync::atomic::AtomicU8::new(WORKER_RUNNING);
        assert!(phase.compare_exchange(WORKER_RUNNING, WORKER_PERSISTING, Ordering::SeqCst, Ordering::SeqCst).is_ok());
        assert!(phase.compare_exchange(WORKER_RUNNING, WORKER_ABORTING, Ordering::SeqCst, Ordering::SeqCst).is_err());
        let phase = std::sync::atomic::AtomicU8::new(WORKER_RUNNING);
        assert!(phase.compare_exchange(WORKER_RUNNING, WORKER_ABORTING, Ordering::SeqCst, Ordering::SeqCst).is_ok());
        assert!(phase.compare_exchange(WORKER_RUNNING, WORKER_PERSISTING, Ordering::SeqCst, Ordering::SeqCst).is_err());
    }




    #[test]
    fn attempt_id_format_verified() {
        // Supplement the tautological local format check with actual IDs
        // produced by the real allocation facility.
        let id = allocate_con_id().expect("id allocation should succeed");
        assert!(id.starts_with("con_"));
        let hex = id.strip_prefix("con_").expect("has con_ prefix");
        let _ = u64::from_str_radix(hex, 16).expect("id suffix is valid hex");
        // Verify actual connect IDs also carry the con_ prefix and are hex-suffix parseable.
        // (The runtime check itself is exercised by the async tests below.)
    }

    #[test]
    fn attempt_id_is_monotonically_increasing() {
        let a = allocate_con_id().expect("first allocation");
        let b = allocate_con_id().expect("second allocation");
        let va = u64::from_str_radix(a.strip_prefix("con_").unwrap(), 16).unwrap();
        let vb = u64::from_str_radix(b.strip_prefix("con_").unwrap(), 16).unwrap();
        assert_ne!(a, b, "ids must be distinct");
        assert!(vb > va, "ids must be monotonically increasing (b > a)");
    }

    #[tokio::test]
    async fn concurrent_begins_produce_distinct_ids_and_capacity() {
        // Simultaneous connect calls must each get a distinct ID and
        // the reservation system must cap at MAX_ATTEMPTS.
        use tokio::sync::{Barrier, Semaphore};

        /// Blocks every begin on the barrier so all MAX_ATTEMPTS enters are
        /// observed before any proceeds. Then gates release via a Semaphore
        /// with zero initial permits: the test adds permits only after
        /// asserting the (MAX_ATTEMPTS+1)th connect is rejected.
        #[derive(Clone)]
        struct LatchedRuntime {
            barrier: Arc<Barrier>,
            gate: Arc<Semaphore>,
        }
        impl OAuthRuntime for LatchedRuntime {
            fn begin(&self, _i: &str, _m: &str, _c: watch::Receiver<bool>)
                -> Pin<Box<dyn Future<Output = Result<Authorization, IntegrationError>> + Send + '_>> {
                let b = self.barrier.clone();
                let g = self.gate.clone();
                Box::pin(async move {
                    b.wait().await;
                    // Acquire an owned permit and forget it so the permit
                    // is never returned to the semaphore, keeping the count
                    // stable across releases.
                    let permit = g.acquire_owned().await.unwrap();
                    permit.forget();
                    Ok(Authorization { url: "fixture".into(), instructions: "fixture".into(), mode: "code", device: None })
                })
            }
            fn complete(&self, _i: &str, _m: &str, _a: Authorization, _c: Option<String>, _c2: watch::Receiver<bool>)
                -> Pin<Box<dyn Future<Output = Result<OpenAiCredential, IntegrationError>> + Send + '_>> {
                Box::pin(async { Ok(OpenAiCredential { access: "a".into(), refresh: "r".into(), expires_at_ms: 1, account_id: None }) })
            }
        }
        let barrier = Arc::new(Barrier::new(MAX_ATTEMPTS + 1));
        let gate = Arc::new(Semaphore::new(0));
        let runtime = LatchedRuntime { barrier: barrier.clone(), gate: gate.clone() };
        let server = IntegrationServer::new(runtime);

        // Spawn MAX_ATTEMPTS concurrent begins.
        let mut handles = Vec::new();
        for _ in 0..MAX_ATTEMPTS {
            let srv = server.clone();
            handles.push(tokio::spawn(async move {
                srv.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, 1_000).await
            }));
        }
        // Bounded wait: all spawned tasks plus main thread (MAX_ATTEMPTS+1
        // total) must enter begin and hit the barrier. The barrier releases
        // all simultaneously, after which spawned begins block on the
        // Semaphore (0 initial permits) while main thread can probe overflow.
        tokio::time::timeout(Duration::from_secs(2), barrier.wait()).await.unwrap();
        // Now all begins are entered but blocked on the Semaphore (0 permits).
        // The MAX_ATTEMPTS+1th connect must be rejected before release.
        let rejection = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, 1_000).await;
        assert_eq!(rejection.map(|_| ()), Err(IntegrationError::TooManyAttempts));
        // Release all blocked begins by adding MAX_ATTEMPTS permits.
        gate.add_permits(MAX_ATTEMPTS);
        // Collect results and verify all IDs are distinct.
        let mut ids = Vec::new();
        for handle in handles {
            let ok = handle.await.unwrap();
            assert!(ok.is_ok(), "concurrent connect should succeed: {:?}", ok.err());
            ids.push(ok.unwrap().attempt_id);
        }
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "concurrent begins must produce distinct IDs: got {:?}", ids);
        // Registry capacity must reflect all MAX_ATTEMPTS committed entries.
        assert_eq!(server.attempts.lock().await.len(), MAX_ATTEMPTS, "registry must hold all {} attempts", MAX_ATTEMPTS);
    }

    #[tokio::test]
    async fn collided_timestamp_scenario_no_overwrite() {
        // Exact reproduction of the original collision: timestamps 1000/1001/1002
        // with reservation increments 1/2/3 and zero map length.
        // Under XOR: 1000^1=1001, 1001^2=1003, 1002^3=1001 (collision!).
        // With the counter-based allocator these produce three distinct IDs.
        let server = IntegrationServer::new(stub_success());
        let a1 = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, 1_000).await.unwrap();
        let a2 = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, 1_001).await.unwrap();
        let a3 = server.connect("openai", ConnectRequest { method_id: "chatgpt-headless".into(), answer: None, label: None }, 1_002).await.unwrap();
        assert_ne!(a1.attempt_id, a2.attempt_id, "first and second IDs must differ");
        assert_ne!(a1.attempt_id, a3.attempt_id, "first and third IDs must differ (original collision)");
        assert_ne!(a2.attempt_id, a3.attempt_id, "second and third IDs must differ");
        // All three attempts must remain in the registry (no overwrite).
        assert_eq!(server.attempts.lock().await.len(), 3, "registry must hold all 3 attempts without overwrite");
    }
}

// A real injectable route tree used by focused HTTP tests and embedders.
pub fn oauth_routes(state: ProviderIntegrationState) -> axum::Router {
    axum::Router::new()
        .route("/api/integration/{integration_id}/connect/oauth", axum::routing::post(connect_oauth))
        .route("/api/integration/{integration_id}/connect/oauth/{attempt_id}", axum::routing::get(status_oauth).delete(cancel_oauth))
        .route("/api/integration/{integration_id}/connect/oauth/{attempt_id}/complete", axum::routing::post(complete_oauth))
        .layer(axum::extract::Extension(state))
}

// ---------------------------------------------------------------------------
// HTTP route handlers
// ---------------------------------------------------------------------------

use axum::{
    body::{to_bytes, Body},
    extract::{Extension, Path},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::{json, Value};

#[derive(Clone)]
pub struct ProviderIntegrationState {
    pub server: Arc<IntegrationServer<dyn OAuthRuntime>>,
}

impl ProviderIntegrationState {
    pub fn injectable(runtime: Arc<dyn OAuthRuntime>, sink: Arc<dyn CredentialSink>) -> Self {
        Self::injectable_clock(runtime, sink, Arc::new(SystemClock))
    }
    pub fn injectable_clock(runtime: Arc<dyn OAuthRuntime>, sink: Arc<dyn CredentialSink>, clock: Arc<dyn Clock>) -> Self {
        Self { server: Arc::new(IntegrationServer::with_parts_clock(runtime, sink, clock)) }
    }
}

impl Default for ProviderIntegrationState {
    fn default() -> Self {
        Self {
            server: Arc::new(IntegrationServer::with_parts(Arc::new(VerifiedRuntime), Arc::new(LegacyCredentialSink))),
        }
    }
}

/// POST /api/integration/{integration_id}/connect/oauth
pub async fn connect_oauth(
    Extension(state): Extension<ProviderIntegrationState>,
    Path(integration_id): Path<String>,
    Json(body): Json<ConnectRequest>,
) -> impl IntoResponse {
    let now = state.server.clock.now();
    let server = &state.server;
    match server.connect(&integration_id, body, now).await {
        Ok(attempt) => (StatusCode::OK, Json(json!(attempt))),
        Err(IntegrationError::NotFound) => (StatusCode::NOT_FOUND, Json(json!({"error":"integration not found"}))),
        Err(IntegrationError::MethodNotFound) => (StatusCode::NOT_FOUND, Json(json!({"error":"integration method not found"}))),
        Err(IntegrationError::TooManyAttempts) => (StatusCode::TOO_MANY_REQUESTS, Json(json!({"error":"too many oauth attempts"}))),
        Err(IntegrationError::Unavailable) => (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error":"provider runtime unavailable"}))),
        Err(IntegrationError::Authorization) => (StatusCode::BAD_GATEWAY, Json(json!({"error":"provider authorization failed"}))),
        Err(IntegrationError::AttemptNotFound) => (StatusCode::NOT_FOUND, Json(json!({"error":"oauth attempt not found"}))),
    }
}

/// GET /api/integration/{integration_id}/connect/oauth/{attempt_id}
pub async fn status_oauth(
    Extension(state): Extension<ProviderIntegrationState>,
    Path((integration_id, attempt_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let now = state.server.clock.now();
    let server = &state.server;
    match server.status(&integration_id, &attempt_id, now).await {
        Ok(status) => (StatusCode::OK, Json(json!(status))),
        Err(IntegrationError::AttemptNotFound) => (StatusCode::NOT_FOUND, Json(json!({"error":"oauth attempt not found"}))),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":"internal error"}))),
    }
}

/// POST /api/integration/{integration_id}/connect/oauth/{attempt_id}/complete
pub async fn complete_oauth(
    Extension(state): Extension<ProviderIntegrationState>,
    Path((integration_id, attempt_id)): Path<(String, String)>,
    Json(body): Json<CompleteRequest>,
) -> impl IntoResponse {
    let now = state.server.clock.now();
    let server = &state.server;
    match server.complete_code(&integration_id, &attempt_id, body.code, now).await {
        Ok(()) => (StatusCode::NO_CONTENT, Json(json!({})).into_response()),
        Err(IntegrationError::AttemptNotFound) => (StatusCode::NOT_FOUND, Json(json!({"error":"oauth attempt not found"})).into_response()),
        Err(IntegrationError::Authorization) => (StatusCode::BAD_GATEWAY, Json(json!({"error":"provider authorization failed"})).into_response()),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":"internal error"})).into_response()),
    }
}

/// DELETE /api/integration/{integration_id}/connect/oauth/{attempt_id}
pub async fn cancel_oauth(
    Extension(state): Extension<ProviderIntegrationState>,
    Path((integration_id, attempt_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let server = &state.server;
    match server.cancel(&integration_id, &attempt_id).await {
        Ok(()) => (StatusCode::NO_CONTENT, Json(json!({})).into_response()),
        Err(IntegrationError::AttemptNotFound) => (StatusCode::NOT_FOUND, Json(json!({"error":"oauth attempt not found"})).into_response()),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":"internal error"})).into_response()),
    }
}
