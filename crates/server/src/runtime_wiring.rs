//! DISC-113: one daemon-owned application runtime composition.
//!
//! AUD-010 finding 2 (unwired): `crates/server/src/lib.rs:97-152` builds routes
//! from a loose `AppState { sessions, catalog }` and serves without composing
//! the engine-owned handles (`EngineHandles`, `app_runtime.rs:504-568`), so
//! daemon/headless/SDK/CLI paths can fork private runtimes instead of sharing
//! one daemon-owned engine. This module is the composition half: it bundles
//! one shared engine (provider routing, sessions, tools, policy, persistence,
//! events) with the client registry, a durable-event replay window, a bounded
//! proxy queue, and cooperative cancel/reclaim state behind a single-owner
//! lease. Execution behavior (provider calls, tool dispatch, HTTP routes)
//! lives elsewhere; serve-path wiring of this composition is integrator-owned.
//!
//! Bounds: at most [`MAX_RUNTIME_SESSIONS`] registered sessions, 64 attached
//! clients (`app_runtime::MAX_ENGINE_CLIENTS`), 2 concurrent turns
//! (`app_runtime::MAX_CONCURRENT_TURNS`), 256-slot proxy queue
//! (`workspace_proxy::MAX_PROXY_QUEUE_ITEMS`), 512 retained replay events
//! (`event_cursor::REPLAY_BUFFER_CAP`). All state is `Send + Sync`; every
//! method takes only short in-memory locks and performs no I/O, spawns no
//! threads, touches no clock, logs no secrets, and contains no `unsafe`.
#![forbid(unsafe_code)]

use std::{
    collections::HashMap,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use opencode_rk_contracts::{AgentId, SessionId};
use opencode_rk_providers::registry::ProviderRegistry;
use opencode_rk_sessions::SessionService;
use opencode_rk_tools::registry::ToolRegistry;
use tokio::sync::mpsc;

use crate::{
    app_runtime::{
        EngineError, EngineHandles, EnginePolicy, TurnObservation, TurnPermits,
        assert_same_turn, assert_single_owner, event_bus, MAX_ENGINE_CLIENTS,
        MAX_MODEL_REF_BYTES, OwnerGuard, SingleOwner,
    },
    clients::{ClientError, ClientId, ClientMessage},
    event_bus::{EventBus, ServerEvent},
    event_cursor::{Cursor, CursorError, ReplayBuffer, StoredEvent, classify},
    turn_service::CancelToken,
    workspace_proxy::{ProxyError, ProxyItem, ProxyQueue, ProxyTarget, RouteHint},
};

/// Sessions one runtime tracks. Above [`MAX_RUNTIME_SESSIONS`] the daemon
/// must shard or refuse; silent unbounded growth is a budget violation.
pub const MAX_RUNTIME_SESSIONS: usize = 4096;
/// Proxy queue item slots shared by every routed request on this engine.
pub const MAX_RUNTIME_PROXY_ITEMS: usize = 256;

/// Every composition-level failure. Deny/bound errors mutate nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WiringError {
    /// Client is not attached to this engine.
    UnknownClient,
    /// Client registry is at [`MAX_ENGINE_CLIENTS`].
    TooManyClients,
    /// Routed request carried no session.
    SessionRequired,
    /// Session is not registered on this engine.
    UnknownSession,
    /// Session registry is at [`MAX_RUNTIME_SESSIONS`].
    TooManySessions,
    /// All [`MAX_CONCURRENT_TURNS`] turn permits are held.
    NoTurnPermit,
    /// `provider/model` reference is empty or over [`MAX_MODEL_REF_BYTES`].
    InvalidModelRef,
    /// A second engine was claimed while one is live.
    AlreadyOwned,
    /// Expected exactly one engine; found another count.
    UnexpectedEngineCount(usize),
    /// Two clients observe turns in different sessions.
    SessionMismatch,
    /// Two clients observe different turns in the same session.
    TurnMismatch { expected: AgentId, actual: AgentId },
    /// Replay cursor is malformed or behind retention.
    BadCursor,
    /// Proxy queue refused the item or the routing input was invalid.
    Proxy(ProxyError),
}

impl fmt::Display for WiringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownClient => write!(f, "client is not attached to this engine"),
            Self::TooManyClients => write!(f, "engine client limit reached"),
            Self::SessionRequired => write!(f, "routed request requires a session"),
            Self::UnknownSession => write!(f, "session is not registered on this engine"),
            Self::TooManySessions => write!(f, "engine session limit reached"),
            Self::NoTurnPermit => write!(f, "too many active turns"),
            Self::InvalidModelRef => write!(f, "model reference is empty or too long"),
            Self::AlreadyOwned => write!(f, "application engine already owned"),
            Self::UnexpectedEngineCount(count) => {
                write!(f, "expected one engine owner, found {count}")
            }
            Self::SessionMismatch => write!(f, "clients observe turns in different sessions"),
            Self::TurnMismatch { expected, actual } => {
                write!(f, "duplicate turn runtimes: {expected} != {actual}")
            }
            Self::BadCursor => write!(f, "replay cursor invalid or behind retention"),
            Self::Proxy(error) => write!(f, "proxy rejected request: {error}"),
        }
    }
}

impl std::error::Error for WiringError {}

impl From<ProxyError> for WiringError {
    fn from(error: ProxyError) -> Self {
        Self::Proxy(error)
    }
}

impl From<EngineError> for WiringError {
    fn from(error: EngineError) -> Self {
        match error {
            EngineError::AlreadyOwned => Self::AlreadyOwned,
            EngineError::UnexpectedEngineCount(count) => Self::UnexpectedEngineCount(count),
            EngineError::SessionMismatch => Self::SessionMismatch,
            EngineError::TurnMismatch { expected, actual } => {
                Self::TurnMismatch { expected, actual }
            }
            EngineError::NoTurnPermit => Self::NoTurnPermit,
            EngineError::InvalidModelRef => Self::InvalidModelRef,
            EngineError::QueueFull { .. } | EngineError::PolicyFull { .. } => {
                Self::TooManySessions
            }
            EngineError::InvalidToolName => Self::InvalidModelRef,
        }
    }
}

impl From<ClientError> for WiringError {
    fn from(error: ClientError) -> Self {
        match error {
            ClientError::MaxClients => Self::TooManyClients,
            ClientError::UnknownClient => Self::UnknownClient,
        }
    }
}

impl From<CursorError> for WiringError {
    fn from(error: CursorError) -> Self {
        let _ = error;
        Self::BadCursor
    }
}

/// Process-wide runtime owner. The daemon acquires an [`EngineLease`] once at
/// startup; every other path (headless submit, SDK, CLI, proxy) borrows the
/// shared [`RuntimeWiring`] instead of building a private runtime.
///
/// Own static (not `app_runtime::ENGINE_OWNER`) so this lane's lease tests
/// never race `app_runtime`'s exclusive-claim unit test in the same target.
static RUNTIME_OWNER: SingleOwner = SingleOwner::const_new();

/// Proof of daemon engine ownership. Releases the [`RUNTIME_OWNER`] claim on
/// drop, so a panicked or cancelled daemon cannot leak the claim.
pub struct EngineLease {
    _guard: OwnerGuard<'static>,
}

impl EngineLease {
    /// Claim the process-wide engine. Fails with [`WiringError::AlreadyOwned`]
    /// while a previous lease is live: starting a second engine against the
    /// same store fails closed instead of forking state.
    pub fn acquire() -> Result<Self, WiringError> {
        let guard = RUNTIME_OWNER.try_claim().map_err(WiringError::from)?;
        Ok(Self { _guard: guard })
    }

    /// Whether the process-wide engine is currently leased.
    #[must_use]
    pub fn is_leased() -> bool {
        RUNTIME_OWNER.is_owned()
    }
}

impl fmt::Debug for EngineLease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EngineLease").finish_non_exhaustive()
    }
}

/// Assert exactly one engine owns the store. `0` means no runtime is
/// serving; `> 1` means duplicate private runtimes exist and clients diverge.
pub fn assert_one_engine(active_engines: usize) -> Result<(), WiringError> {
    assert_single_owner(active_engines).map_err(WiringError::from)
}

/// Acceptance probe: an SDK call and a CLI call must observe the same turn ID
/// in the same session. Any divergence is an error, never a second runtime.
pub fn assert_shared_turn(
    first: &TurnObservation,
    second: &TurnObservation,
) -> Result<AgentId, WiringError> {
    assert_same_turn(first, second).map_err(WiringError::from)
}

/// A headless submit admitted onto the shared engine: which attached client,
/// in which registered session, under which turn ID, against which model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmittedTurn {
    /// Attached client that submitted.
    pub client: ClientId,
    /// Registered session the turn belongs to.
    pub session: SessionId,
    /// Stable turn ID every client observes (no duplicate runtimes).
    pub turn: AgentId,
    /// Validated `provider/model` reference.
    pub model: String,
}

/// Cooperative cancel/reclaim state for one runtime. `cancel()` fires the
/// active token (drivers poll [`CancelToken::is_cancelled`] and stop new
/// work); `reclaim()` retires it and issues a fresh token so a subsequent
/// submit cannot inherit a stale cancellation. The generation counter proves
/// a reclaim happened.
#[derive(Debug)]
pub struct CancelState {
    generation: AtomicU64,
    cancelled: AtomicBool,
    token: Mutex<CancelToken>,
}

impl CancelState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            cancelled: AtomicBool::new(false),
            token: Mutex::new(CancelToken::new()),
        }
    }

    /// Fire the active cancel token. Idempotent; mutates nothing else.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Ok(token) = self.token.lock() {
            token.cancel();
        }
    }

    /// Retire the fired token and issue a fresh one. Returns the new
    /// generation (starts at 1; each reclaim bumps by exactly one).
    pub fn reclaim(&self) -> u64 {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.cancelled.store(false, Ordering::SeqCst);
        if let Ok(mut slot) = self.token.lock() {
            *slot = CancelToken::new();
        }
        generation
    }

    /// Current reclaim generation (0 = never reclaimed).
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Whether the active token is fired.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Clone of the active token for drivers to poll.
    #[must_use]
    pub fn token(&self) -> CancelToken {
        self.token
            .lock()
            .map(|token| token.clone())
            .unwrap_or_default()
    }
}

impl Default for CancelState {
    fn default() -> Self {
        Self::new()
    }
}

struct RuntimeInner {
    engine: EngineHandles,
    attached: Mutex<HashMap<ClientId, mpsc::Sender<ClientMessage>>>,
    next_client: AtomicU64,
    sessions: Mutex<Vec<SessionId>>,
    replay: Mutex<ReplayBuffer>,
    proxy_queue: Mutex<ProxyQueue>,
    cancel: CancelState,
    permits: TurnPermits,
    events: EventBus,
}

/// One daemon-owned application runtime: provider routing, sessions, tools,
/// policy, persistence and events behind a single shared composition. Every
/// client path (headless submit, SDK, CLI, workspace proxy) borrows these
/// handles instead of building a private runtime.
pub struct RuntimeWiring {
    inner: Arc<RuntimeInner>,
}

impl Clone for RuntimeWiring {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl RuntimeWiring {
    /// Compose the daemon's shared runtime with an initially empty provider
    /// registry. Provider credentials and clients remain lazy; constructing
    /// the HTTP daemon must not read ambient secrets or start background work.
    #[must_use]
    pub fn for_daemon(sessions: SessionService, tools: ToolRegistry, policy: EnginePolicy) -> Self {
        Self::with_sessions(ProviderRegistry::new(), sessions, tools, policy)
    }

    /// Compose from a live session service: sessions and store share the same
    /// value so every client observes one persistence authority. No I/O, no
    /// spawning: the daemon builds providers/sessions/tools/policy and the
    /// runtime only takes shared ownership plus its bounded registries.
    #[must_use]
    pub fn with_sessions(
        providers: ProviderRegistry,
        sessions: SessionService,
        tools: ToolRegistry,
        policy: EnginePolicy,
    ) -> Self {
        let events = event_bus();
        let store = sessions.clone();
        let engine = EngineHandles::new(providers, sessions, tools, policy, store, events.clone());
        Self::from_parts(engine, events)
    }

    fn from_parts(engine: EngineHandles, events: EventBus) -> Self {
        Self {
            inner: Arc::new(RuntimeInner {
                engine,
                attached: Mutex::new(HashMap::new()),
                next_client: AtomicU64::new(1),
                sessions: Mutex::new(Vec::new()),
                replay: Mutex::new(ReplayBuffer::new()),
                proxy_queue: Mutex::new(ProxyQueue::new()),
                cancel: CancelState::new(),
                permits: TurnPermits::new(),
                events,
            }),
        }
    }

    /// Shared engine handles: provider routing, sessions, tools, policy,
    /// store, events, turn permits.
    #[must_use]
    pub fn engine(&self) -> &EngineHandles {
        &self.inner.engine
    }

    /// Shared event bus for daemon state changes.
    #[must_use]
    pub fn events(&self) -> &EventBus {
        &self.inner.events
    }

    /// Cooperative cancel/reclaim state.
    #[must_use]
    pub fn cancel_state(&self) -> &CancelState {
        &self.inner.cancel
    }

    /// Attach one client. Bounded by [`MAX_ENGINE_CLIENTS`]; the sender is
    /// retained so daemon broadcasts reach this client.
    pub fn attach(
        &self,
        sender: mpsc::Sender<ClientMessage>,
    ) -> Result<ClientId, WiringError> {
        let mut attached = self.inner.attached.lock().expect("client registry");
        if attached.len() >= MAX_ENGINE_CLIENTS {
            return Err(WiringError::TooManyClients);
        }
        let id = ClientId(self.inner.next_client.fetch_add(1, Ordering::SeqCst));
        attached.insert(id, sender);
        Ok(id)
    }

    /// Detach a client. Unknown IDs fail closed without mutating anything.
    pub fn detach(&self, client: ClientId) -> Result<(), WiringError> {
        let mut attached = self.inner.attached.lock().expect("client registry");
        attached
            .remove(&client)
            .map(|_| ())
            .ok_or(WiringError::UnknownClient)
    }

    /// Number of attached clients.
    #[must_use]
    pub fn attached_count(&self) -> usize {
        self.inner
            .attached
            .lock()
            .map(|attached| attached.len())
            .unwrap_or(0)
    }

    /// Whether a client is currently attached.
    #[must_use]
    pub fn is_attached(&self, client: ClientId) -> bool {
        self.inner
            .attached
            .lock()
            .map(|attached| attached.contains_key(&client))
            .unwrap_or(false)
    }

    /// Register one session as engine-owned. Idempotent; bounded by
    /// [`MAX_RUNTIME_SESSIONS`].
    pub fn register_session(&self, session: SessionId) -> Result<(), WiringError> {
        let mut sessions = self.inner.sessions.lock().expect("session registry");
        if sessions.iter().any(|known| *known == session) {
            return Ok(());
        }
        if sessions.len() >= MAX_RUNTIME_SESSIONS {
            return Err(WiringError::TooManySessions);
        }
        sessions.push(session);
        Ok(())
    }

    /// Whether a session is registered on this engine.
    #[must_use]
    pub fn has_session(&self, session: SessionId) -> bool {
        self.inner
            .sessions
            .lock()
            .map(|sessions| sessions.iter().any(|known| *known == session))
            .unwrap_or(false)
    }

    /// Number of registered sessions.
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.inner
            .sessions
            .lock()
            .map(|sessions| sessions.len())
            .unwrap_or(0)
    }

    /// Headless submit through daemon, execution permits, events and proxy
    /// state on one shared engine: the client must be attached, the session
    /// registered, the model a non-empty `provider/model` reference within
    /// [`MAX_MODEL_REF_BYTES`], and a turn permit must be free. Admission
    /// records a durable replay event so late subscribers observe the turn.
    pub fn submit_headless(
        &self,
        client: ClientId,
        session: SessionId,
        turn: AgentId,
        model: &str,
    ) -> Result<(SubmittedTurn, tokio::sync::OwnedSemaphorePermit), WiringError> {
        if !self.is_attached(client) {
            return Err(WiringError::UnknownClient);
        }
        if !self.has_session(session) {
            return Err(WiringError::UnknownSession);
        }
        if model.is_empty() || model.len() > MAX_MODEL_REF_BYTES {
            return Err(WiringError::InvalidModelRef);
        }
        let permit = self.inner.permits.try_acquire().map_err(WiringError::from)?;
        self.record("message.appended");
        let _ = self.inner.events.publish(ServerEvent::MessageAppended {
            session,
            seq: self.replay_head(),
        });
        Ok((
            SubmittedTurn {
                client,
                session,
                turn,
                model: model.to_owned(),
            },
            permit,
        ))
    }

    /// Route one workspace request on the shared engine. Every routed request
    /// enforces session membership: `None` is [`WiringError::SessionRequired`],
    /// an unregistered session is [`WiringError::UnknownSession`]. Admitted
    /// requests delegate to the pure [`crate::workspace_proxy::route`].
    pub fn route_for_session(
        &self,
        hint: &RouteHint,
        session: Option<SessionId>,
    ) -> Result<ProxyTarget, WiringError> {
        let session = session.ok_or(WiringError::SessionRequired)?;
        if !self.has_session(session) {
            return Err(WiringError::UnknownSession);
        }
        crate::workspace_proxy::route(hint).map_err(WiringError::from)
    }

    /// Buffer one proxy payload on the shared bounded queue.
    pub fn proxy_push(&self, item: ProxyItem) -> Result<(), WiringError> {
        self.inner
            .proxy_queue
            .lock()
            .expect("proxy queue")
            .push(item)
            .map_err(WiringError::from)
    }

    /// Drain buffered proxy payloads in FIFO order.
    #[must_use]
    pub fn proxy_drain(&self) -> Vec<ProxyItem> {
        self.inner
            .proxy_queue
            .lock()
            .map(|mut queue| queue.drain())
            .unwrap_or_default()
    }

    /// Queued proxy payload count.
    #[must_use]
    pub fn proxy_len(&self) -> usize {
        self.inner
            .proxy_queue
            .lock()
            .map(|queue| queue.queue_len())
            .unwrap_or(0)
    }

    /// Record one event type in the durable replay window. Returns the stored
    /// event (callers derive cursors via [`Cursor::after`]).
    pub fn record(&self, event_type: &str) -> StoredEvent {
        self.inner
            .replay
            .lock()
            .expect("replay window")
            .push(event_type)
    }

    /// Replay durable events after `cursor`, each exactly once in `seq`
    /// order. Ephemeral types are skipped; a stale cursor fails closed so
    /// the subscriber takes a snapshot instead of a gapped prefix.
    pub fn replay(&self, cursor: &Cursor) -> Result<Vec<StoredEvent>, WiringError> {
        self.inner
            .replay
            .lock()
            .expect("replay window")
            .replay(cursor)
            .map_err(WiringError::from)
    }

    /// Durability of one event type on this runtime's replay window.
    #[must_use]
    pub fn durability(event_type: &str) -> crate::event_cursor::Durability {
        classify(event_type)
    }

    /// Last retained sequence (0 when the window is empty).
    #[must_use]
    pub fn replay_head(&self) -> u64 {
        self.inner
            .replay
            .lock()
            .map(|replay| replay.head_seq())
            .unwrap_or(0)
    }

    /// Available turn permits on the shared engine.
    #[must_use]
    pub fn available_permits(&self) -> usize {
        self.inner.permits.available_permits()
    }

    /// Admit one HTTP/headless turn onto the daemon-owned concurrency budget.
    /// The owned permit releases capacity when the request or response stream
    /// is dropped, including client disconnect and cancellation paths.
    pub fn try_acquire_turn(&self) -> Result<tokio::sync::OwnedSemaphorePermit, WiringError> {
        self.inner.permits.try_acquire().map_err(WiringError::from)
    }
}

impl fmt::Debug for RuntimeWiring {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeWiring")
            .field("attached", &self.attached_count())
            .field("sessions", &self.session_count())
            .field("permits", &self.available_permits())
            .field("replay_head", &self.replay_head())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_runtime::MAX_CONCURRENT_TURNS;
    use crate::event_cursor::Durability;

    fn wiring() -> RuntimeWiring {
        RuntimeWiring::with_sessions(
            ProviderRegistry::new(),
            test_sessions(),
            ToolRegistry::new(),
            EnginePolicy::default_deny(),
        )
    }

    fn test_sessions() -> SessionService {
        let dir = tempfile::tempdir().expect("disposable fixture dir");
        // Leak the guard: the in-memory store owns its rows; the blob dir is
        // only touched on blob writes, which these tests never perform.
        std::mem::forget(dir);
        let storage = opencode_rk_storage::Storage::open_in_memory(
            std::env::temp_dir().join(format!(
                "disc113-blobs-{}-{}",
                std::process::id(),
                next_fixture_id()
            )),
        )
        .expect("in-memory storage fixture");
        SessionService::new(Arc::new(storage))
    }

    fn next_fixture_id() -> u64 {
        static FIXTURES: AtomicU64 = AtomicU64::new(1);
        FIXTURES.fetch_add(1, Ordering::SeqCst)
    }

    fn attached(wiring: &RuntimeWiring) -> ClientId {
        let (sender, _receiver) = mpsc::channel(16);
        wiring.attach(sender).expect("attach fixture client")
    }

    fn registered_session(wiring: &RuntimeWiring) -> SessionId {
        let session = SessionId::new();
        wiring
            .register_session(session)
            .expect("register fixture session");
        session
    }

    #[test]
    fn disc113_t01_headless_submit_flows_on_shared_engine() {
        let wiring = wiring();
        let client = attached(&wiring);
        let session = registered_session(&wiring);
        let turn = AgentId::new();
        assert_eq!(wiring.available_permits(), MAX_CONCURRENT_TURNS);
        let (submitted, _permit) = wiring
            .submit_headless(client, session, turn, "openai/gpt-5")
            .expect("headless submit on shared engine");
        assert_eq!(
            submitted,
            SubmittedTurn {
                client,
                session,
                turn,
                model: "openai/gpt-5".to_owned(),
            }
        );
        // Admission consumed a permit and recorded a durable replay event.
        assert_eq!(wiring.available_permits(), MAX_CONCURRENT_TURNS - 1);
        let replayed = wiring.replay(&Cursor::genesis()).expect("replay submit");
        assert_eq!(replayed.len(), 1);
        assert_eq!(replayed[0].event_type, "message.appended");
        assert_eq!(replayed[0].durability(), Durability::Durable);
        // Permit release readmits the next submit: one engine, bounded turns.
        drop(_permit);
        assert_eq!(wiring.available_permits(), MAX_CONCURRENT_TURNS);
        // Empty model and overlong model refs are denied without side effects.
        let permits_before = wiring.available_permits();
        let head_before = wiring.replay_head();
        assert_eq!(
            wiring
                .submit_headless(client, session, AgentId::new(), "")
                .unwrap_err(),
            WiringError::InvalidModelRef
        );
        assert_eq!(wiring.available_permits(), permits_before);
        assert_eq!(wiring.replay_head(), head_before);
        // Unknown clients and unregistered sessions fail closed.
        let (stray_sender, _stray_receiver) =
            mpsc::channel::<ClientMessage>(8);
        let _ = stray_sender;
        let ghost = ClientId(u64::MAX);
        assert_eq!(
            wiring
                .submit_headless(ghost, session, AgentId::new(), "openai/gpt-5")
                .unwrap_err(),
            WiringError::UnknownClient
        );
        assert_eq!(
            wiring
                .submit_headless(client, SessionId::new(), AgentId::new(), "openai/gpt-5")
                .unwrap_err(),
            WiringError::UnknownSession
        );
    }

    #[test]
    fn disc113_t02_sdk_and_cli_observe_same_turn() {
        let sdk = TurnObservation {
            client: ClientId(7),
            session: SessionId::new(),
            turn: AgentId::new(),
        };
        let cli = TurnObservation {
            client: ClientId(9),
            session: sdk.session,
            turn: sdk.turn,
        };
        assert_eq!(
            assert_shared_turn(&sdk, &cli).expect("shared turn"),
            sdk.turn
        );
        let forked_turn = TurnObservation {
            client: cli.client,
            session: sdk.session,
            turn: AgentId::new(),
        };
        assert!(
            matches!(
                assert_shared_turn(&sdk, &forked_turn).unwrap_err(),
                WiringError::TurnMismatch { .. }
            ),
            "duplicate runtimes must fail, never fork"
        );
        let forked_session = TurnObservation {
            client: cli.client,
            session: SessionId::new(),
            turn: sdk.turn,
        };
        assert_eq!(
            assert_shared_turn(&sdk, &forked_session).unwrap_err(),
            WiringError::SessionMismatch
        );
    }

    #[test]
    fn disc113_t03_proxy_enforces_session_membership() {
        let wiring = wiring();
        let session = registered_session(&wiring);
        let local = RouteHint {
            path: "/api/sessions/x".to_owned(),
            has_directory: true,
            remote_endpoint: None,
        };
        assert_eq!(
            wiring.route_for_session(&local, Some(session)),
            Ok(ProxyTarget::Local)
        );
        // Every routed request requires a session: None fails closed.
        assert_eq!(
            wiring.route_for_session(&local, None).unwrap_err(),
            WiringError::SessionRequired
        );
        // Foreign sessions fail closed even on well-formed paths.
        assert_eq!(
            wiring
                .route_for_session(&local, Some(SessionId::new()))
                .unwrap_err(),
            WiringError::UnknownSession
        );
        // Membership does not launder routing errors: bridge path without an
        // endpoint still fails at the proxy layer.
        let bridge = RouteHint {
            path: "/__workspace_ws".to_owned(),
            has_directory: false,
            remote_endpoint: None,
        };
        assert!(
            matches!(
                wiring.route_for_session(&bridge, Some(session)).unwrap_err(),
                WiringError::Proxy(_)
            ),
            "bridge path without endpoint must stay rejected"
        );
    }

    #[test]
    fn disc113_t04_second_engine_lease_fails_closed() {
        assert!(assert_one_engine(1).is_ok());
        assert_eq!(
            assert_one_engine(0).unwrap_err(),
            WiringError::UnexpectedEngineCount(0)
        );
        assert_eq!(
            assert_one_engine(2).unwrap_err(),
            WiringError::UnexpectedEngineCount(2)
        );
        assert!(!EngineLease::is_leased());
        let lease = EngineLease::acquire().expect("first engine lease");
        assert!(EngineLease::is_leased());
        assert_eq!(
            EngineLease::acquire().unwrap_err(),
            WiringError::AlreadyOwned
        );
        drop(lease);
        assert!(!EngineLease::is_leased());
        let _reclaimed = EngineLease::acquire().expect("reclaim after release");
    }

    #[test]
    fn disc113_t05_events_replay_identically_to_late_subscribers() {
        let wiring = wiring();
        let first = wiring.record("session.created");
        let second = wiring.record("message.appended");
        // Ephemeral frames are retained in-window but never replay as state.
        wiring.record("assistant.delta");
        let third = wiring.record("tool.completed");
        assert_eq!(third.seq, first.seq + 3);
        let from_genesis = wiring.replay(&Cursor::genesis()).expect("genesis replay");
        // Only durable events replay: the delta is skipped, never promoted.
        assert_eq!(
            from_genesis
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>(),
            ["session.created", "message.appended", "tool.completed"]
        );
        // A late subscriber resuming from a cursor sees exactly the suffix.
        let late = wiring.replay(&Cursor::after(&second)).expect("late replay");
        assert_eq!(
            late.iter().map(|event| event.seq).collect::<Vec<_>>(),
            [second.seq + 2]
        );
        assert_eq!(late[0].event_type, "tool.completed");
        assert_eq!(late[0].digest, third.digest);
        // Stale cursors fail closed with resync, never a gapped prefix.
        assert_eq!(
            wiring
                .replay(&Cursor::new(0, 7))
                .unwrap_err(),
            WiringError::BadCursor
        );
        assert_eq!(
            wiring
                .replay(&Cursor::new(u64::MAX, 0))
                .unwrap_err(),
            WiringError::BadCursor
        );
    }

    #[test]
    fn disc113_t06_cancel_fires_and_reclaim_issues_fresh_token() {
        let wiring = wiring();
        let state = wiring.cancel_state();
        assert_eq!(state.generation(), 0);
        assert!(!state.is_cancelled());
        let live = state.token();
        assert!(!live.is_cancelled());
        state.cancel();
        assert!(state.is_cancelled());
        assert!(live.is_cancelled());
        assert!(state.token().is_cancelled());
        // Reclaim retires the fired token: fresh token, generation bumped.
        let generation = state.reclaim();
        assert_eq!(generation, 1);
        assert_eq!(state.generation(), 1);
        assert!(!state.is_cancelled());
        assert!(!state.token().is_cancelled());
        // A second cancel/reclaim cycle bumps exactly once more.
        state.cancel();
        assert_eq!(state.reclaim(), 2);
    }
}
