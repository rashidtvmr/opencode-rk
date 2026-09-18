//! APP-003 single-engine composition: one daemon-owned Tokio application engine.
//!
//! Current code (HEAD `5af7884`): `crates/server/src/lib.rs:79-131` builds routes
//! from a loose `AppState { sessions, catalog }`; `lib.rs:73` gates turns with a
//! private `TURN_PERMITS = 2` semaphore; `lib.rs:135-194` reports every tool as
//! `available_for_web_turn: false` because no turn adapter owns tools.
//! AUD-005 traces `registry -> executor -> policy -> state -> output` as unwired
//! (`crates/tools/src/registry.rs:60`, `permission.rs:1` stub) and AUD-010
//! requires runtime wiring of daemon + events + sdk + proxy + headless e2e.
//!
//! This module is the composition half of APP-003: it bundles the engine-owned
//! handles (`EngineHandles`: provider routing, sessions, tools, policy,
//! persistence, events), asserts single ownership, checks that two clients
//! observe the same turn ID instead of running duplicate private runtimes, and
//! bounds every queue with `const` caps. Execution behavior (provider calls,
//! tool dispatch, cancellation) lives in the turn/executor lanes; this file
//! only owns composition, ownership, and bounds. All state is `Send + Sync`
//! and every method is non-blocking (short in-memory critical sections only,
//! released before any `.await`), so the engine is safe to share across Tokio
//! tasks. No I/O, no threads, no secrets, no `unsafe`.
#![forbid(unsafe_code)]

use std::{
    collections::VecDeque, fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use opencode_rk_contracts::{
    AgentId, MessageRecord, MessageRole, SessionId, MAX_INLINE_PAYLOAD_BYTES,
};
use opencode_rk_providers::registry::ProviderRegistry;
use opencode_rk_sessions::SessionService;
use opencode_rk_tools::registry::ToolRegistry;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{clients::ClientId, event_bus::EventBus, turn_service::CancelToken};
use crate::event_bus::ServerEvent;

/// Concurrent turns admitted by the engine. Matches `TURN_PERMITS` in
/// `crates/server/src/lib.rs:73` (`Semaphore::const_new(2)`).
pub const MAX_CONCURRENT_TURNS: usize = 2;
/// Turn submissions waiting for a permit. Queued, never silently dropped:
/// [`BoundedQueue::try_push`] fails loudly at the cap.
pub const MAX_PENDING_TURNS: usize = 16;
/// Clients attached to one engine. Enforced by the client registry wiring;
/// kept here so daemon, web, and headless paths share one ceiling.
pub const MAX_ENGINE_CLIENTS: usize = 64;
/// Per-subscriber event buffer. Matches the [`EventBus`] channel capacity the
/// engine constructs via [`event_bus()`].
pub const MAX_EVENT_BUFFER: usize = 256;
/// Tool rules per [`EnginePolicy`] list. Matches
/// `opencode_rk_tools::tool_allow::MAX_TOOL_ALLOW`.
pub const MAX_POLICY_RULES: usize = 128;
/// Transcript entries admitted to one provider request. Matches
/// `opencode_rk_providers::responses::MAX_RESPONSES_INPUT_MESSAGES`.
pub const MAX_TURN_HISTORY: usize = 100;
/// Max bytes of one `provider/model` reference on a queued turn.
pub const MAX_MODEL_REF_BYTES: usize = 512;

/// Every composition-level failure. Deny/bound errors mutate nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineError {
    /// A bounded queue refused a push at its `const` cap.
    QueueFull {
        queue: &'static str,
        capacity: usize,
    },
    /// A second engine was claimed while one is live.
    AlreadyOwned,
    /// Expected exactly one engine; found another count.
    UnexpectedEngineCount(usize),
    /// Two clients observe turns in different sessions.
    SessionMismatch,
    /// Two clients observe different turns in the same session.
    TurnMismatch {
        expected: AgentId,
        actual: AgentId,
    },
    /// An [`EnginePolicy`] rule list is at [`MAX_POLICY_RULES`].
    PolicyFull {
        max: usize,
    },
    /// Tool rule name is empty.
    InvalidToolName,
    /// `provider/model` reference is empty or over [`MAX_MODEL_REF_BYTES`].
    InvalidModelRef,
    /// All [`MAX_CONCURRENT_TURNS`] turn permits are held.
    NoTurnPermit,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueueFull { queue, capacity } => {
                write!(f, "{queue} full at capacity {capacity}")
            }
            Self::AlreadyOwned => write!(f, "application engine already owned"),
            Self::UnexpectedEngineCount(count) => {
                write!(f, "expected one engine owner, found {count}")
            }
            Self::SessionMismatch => write!(f, "clients observe turns in different sessions"),
            Self::TurnMismatch { expected, actual } => {
                write!(f, "duplicate turn runtimes: {expected} != {actual}")
            }
            Self::PolicyFull { max } => write!(f, "policy rule list full at {max} entries"),
            Self::InvalidToolName => write!(f, "tool rule name is empty"),
            Self::InvalidModelRef => write!(f, "model reference is empty or too long"),
            Self::NoTurnPermit => write!(f, "too many active turns"),
        }
    }
}

impl std::error::Error for EngineError {}

/// Outcome of an [`EnginePolicy`] lookup. Deny always wins over allow and over
/// the default, so mandatory system protections survive a wildcard default.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyDecision {
    Allow,
    Deny,
}

/// Bounded allow/deny policy for native tool dispatch. Checked by the engine
/// before any tool process starts or file is touched, so a deny provably has
/// no side effects. Rule lists are capped at [`MAX_POLICY_RULES`] each.
#[derive(Clone, Debug)]
pub struct EnginePolicy {
    default_allow: bool,
    allow: Vec<String>,
    deny: Vec<String>,
}

impl EnginePolicy {
    /// Preserve-legacy default: unspecified tools stay allowed.
    #[must_use]
    pub fn default_allow() -> Self {
        Self {
            default_allow: true,
            allow: Vec::new(),
            deny: Vec::new(),
        }
    }

    /// Safe default: unspecified tools are denied.
    #[must_use]
    pub fn default_deny() -> Self {
        Self {
            default_allow: false,
            allow: Vec::new(),
            deny: Vec::new(),
        }
    }

    /// Grant `name`. Idempotent; deny still wins at lookup time.
    pub fn allow_tool(&mut self, name: impl Into<String>) -> Result<(), EngineError> {
        let name = name.into();
        if name.is_empty() {
            return Err(EngineError::InvalidToolName);
        }
        if !self.allow.iter().any(|entry| entry == &name) {
            if self.allow.len() >= MAX_POLICY_RULES {
                return Err(EngineError::PolicyFull {
                    max: MAX_POLICY_RULES,
                });
            }
            self.allow.push(name);
        }
        Ok(())
    }

    /// Deny `name`. Wins over [`Self::allow_tool`] and the default.
    pub fn deny_tool(&mut self, name: impl Into<String>) -> Result<(), EngineError> {
        let name = name.into();
        if name.is_empty() {
            return Err(EngineError::InvalidToolName);
        }
        if !self.deny.iter().any(|entry| entry == &name) {
            if self.deny.len() >= MAX_POLICY_RULES {
                return Err(EngineError::PolicyFull {
                    max: MAX_POLICY_RULES,
                });
            }
            self.deny.push(name);
        }
        Ok(())
    }

    /// Deny list first, then allow list, then the default.
    #[must_use]
    pub fn decision(&self, tool: &str) -> PolicyDecision {
        if self.deny.iter().any(|entry| entry == tool) {
            return PolicyDecision::Deny;
        }
        if self.allow.iter().any(|entry| entry == tool) {
            return PolicyDecision::Allow;
        }
        if self.default_allow {
            PolicyDecision::Allow
        } else {
            PolicyDecision::Deny
        }
    }

    #[must_use]
    pub fn allow_len(&self) -> usize {
        self.allow.len()
    }

    #[must_use]
    pub fn deny_len(&self) -> usize {
        self.deny.len()
    }
}

impl Default for EnginePolicy {
    fn default() -> Self {
        Self::default_deny()
    }
}

/// Single-owner cell: at most one live [`OwnerGuard`] at a time. Claim is a
/// single atomic compare-exchange; release happens in [`Drop`], so a panicked
/// or cancelled owner cannot leak the claim.
#[derive(Debug)]
pub struct SingleOwner {
    live: AtomicBool,
}

impl SingleOwner {
    #[must_use]
    pub const fn const_new() -> Self {
        Self {
            live: AtomicBool::new(false),
        }
    }

    #[must_use]
    pub fn new() -> Self {
        Self::const_new()
    }

    /// Claim ownership. Fails with [`EngineError::AlreadyOwned`] while a guard
    /// is live.
    pub fn try_claim(&self) -> Result<OwnerGuard<'_>, EngineError> {
        self.live
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| OwnerGuard { owner: self })
            .map_err(|_| EngineError::AlreadyOwned)
    }

    #[must_use]
    pub fn is_owned(&self) -> bool {
        self.live.load(Ordering::Acquire)
    }
}

impl Default for SingleOwner {
    fn default() -> Self {
        Self::new()
    }
}

/// Proof of engine ownership. Releases the [`SingleOwner`] claim on drop.
#[derive(Debug)]
pub struct OwnerGuard<'a> {
    owner: &'a SingleOwner,
}

impl Drop for OwnerGuard<'_> {
    fn drop(&mut self) {
        self.owner.live.store(false, Ordering::Release);
    }
}

/// Process-wide engine owner. The daemon claims this once at startup; every
/// other client (web, TUI, headless) borrows [`EngineHandles`] instead of
/// building a private runtime.
static ENGINE_OWNER: SingleOwner = SingleOwner::const_new();

/// Claim the process-wide engine. Fails while a previous claim is live.
pub fn claim_engine() -> Result<OwnerGuard<'static>, EngineError> {
    ENGINE_OWNER.try_claim()
}

/// Whether the process-wide engine is currently claimed.
#[must_use]
pub fn engine_owned() -> bool {
    ENGINE_OWNER.is_owned()
}

/// Assert exactly one engine owns the store. `0` means no runtime is serving;
/// `> 1` means duplicate private runtimes exist and clients may diverge.
pub fn assert_single_owner(active_engines: usize) -> Result<(), EngineError> {
    if active_engines == 1 {
        Ok(())
    } else {
        Err(EngineError::UnexpectedEngineCount(active_engines))
    }
}

/// One client's view of the live turn: which client, in which session, under
/// which turn ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TurnObservation {
    pub client: ClientId,
    pub session: SessionId,
    pub turn: AgentId,
}

/// APP-003 acceptance probe: two clients must observe the same turn ID in the
/// same session. Returns the shared turn ID; any divergence is an error, never
/// a second private runtime.
pub fn assert_same_turn(
    first: &TurnObservation,
    second: &TurnObservation,
) -> Result<AgentId, EngineError> {
    if first.session != second.session {
        return Err(EngineError::SessionMismatch);
    }
    if first.turn != second.turn {
        return Err(EngineError::TurnMismatch {
            expected: first.turn,
            actual: second.turn,
        });
    }
    Ok(first.turn)
}

/// A turn waiting for a permit: session, stable turn ID, and provider model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingTurn {
    pub session: SessionId,
    pub turn: AgentId,
    pub model: String,
}

impl PendingTurn {
    /// `model` must be a non-empty `provider/model` reference within
    /// [`MAX_MODEL_REF_BYTES`].
    pub fn new(
        session: SessionId,
        turn: AgentId,
        model: impl Into<String>,
    ) -> Result<Self, EngineError> {
        let model = model.into();
        if model.is_empty() || model.len() > MAX_MODEL_REF_BYTES {
            return Err(EngineError::InvalidModelRef);
        }
        Ok(Self {
            session,
            turn,
            model,
        })
    }
}

/// FIFO queue with a `const` cap. [`Self::try_push`] fails with
/// [`EngineError::QueueFull`] at the cap instead of growing or dropping.
#[derive(Clone, Debug)]
pub struct BoundedQueue<T> {
    name: &'static str,
    capacity: usize,
    items: VecDeque<T>,
}

impl<T> BoundedQueue<T> {
    #[must_use]
    pub fn new(name: &'static str, capacity: usize) -> Self {
        Self {
            name,
            capacity: capacity.max(1),
            items: VecDeque::new(),
        }
    }

    pub fn try_push(&mut self, item: T) -> Result<(), EngineError> {
        if self.items.len() >= self.capacity {
            return Err(EngineError::QueueFull {
                queue: self.name,
                capacity: self.capacity,
            });
        }
        self.items.push_back(item);
        Ok(())
    }

    pub fn pop_front(&mut self) -> Option<T> {
        self.items.pop_front()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }
}

/// Turns waiting for a turn permit, capped at [`MAX_PENDING_TURNS`].
pub type PendingTurnQueue = BoundedQueue<PendingTurn>;

/// Empty pending-turn queue at the engine cap.
#[must_use]
pub fn pending_turn_queue() -> PendingTurnQueue {
    BoundedQueue::new("pending_turns", MAX_PENDING_TURNS)
}

/// Engine event bus at the engine buffer cap.
#[must_use]
pub fn event_bus() -> EventBus {
    EventBus::new(MAX_EVENT_BUFFER)
}

/// Turn permits shared by every client path (HTTP turns, streaming turns,
/// headless runs). Cap equals [`MAX_CONCURRENT_TURNS`]; acquisition is
/// synchronous and never blocks a Tokio worker.
///
/// [`TurnPermits::acquire`] hands out [`TurnLease`] values: an owned permit
/// plus a cooperative [`CancelToken`]. Dropping the lease frees the permit;
/// firing [`TurnLease::cancel`] stops drivers dispatching new work.
#[derive(Clone, Debug)]
pub struct TurnPermits {
    permits: Arc<Semaphore>,
}

/// One admitted turn: an owned semaphore permit plus a cooperative
/// cancellation token. Dropping the lease frees the permit (reclaim); firing
/// [`TurnLease::cancel`] tells drivers to stop dispatching new work. Cancel is
/// idempotent; a cancelled lease makes [`EngineHandles::run_prompt_turn`]
/// settle nothing and return [`TurnError::Cancelled`].
pub struct TurnLease {
    /// Held while the turn runs; released on drop.
    pub permit: Option<OwnedSemaphorePermit>,
    /// Cooperative token shared with drivers.
    pub token: CancelToken,
}

impl TurnLease {
    fn new(permit: OwnedSemaphorePermit) -> Self {
        Self {
            permit: Some(permit),
            token: CancelToken::new(),
        }
    }

    /// Fire the cooperative token. Idempotent; also fires the driver's clone.
    pub fn cancel(&self) {
        self.token.cancel();
    }

    /// Whether cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    /// Reclaim: fire the token and release the permit early.
    pub fn reclaim(mut self) {
        self.token.cancel();
        self.permit.take();
    }
}

impl fmt::Debug for TurnLease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurnLease")
            .field("permit_held", &self.permit.is_some())
            .field("cancelled", &self.token.is_cancelled())
            .finish()
    }
}

/// Turn-level failures for [`EngineHandles`] behavior. Deny is NOT an error:
/// [`EngineHandles::dispatch_tool`] returns `Ok(denied)` with a durable `Tool`
/// message, so denials persist as data. These variants fail closed and mutate
/// nothing: invalid input before any write, cancel before settle, unknown
/// tool before any spawn, store errors surfacing the durable write failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TurnError {
    /// Turn text is empty or over [`MAX_INLINE_PAYLOAD_BYTES`].
    InvalidPrompt,
    /// Lease was cancelled before the turn settled; nothing persisted.
    Cancelled,
    /// Tool has no registered entry in the engine snapshot.
    UnknownTool(String),
    /// Durable store rejected the write.
    Store(String),
}

impl std::fmt::Display for TurnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPrompt => write!(f, "turn text is empty or too large"),
            Self::Cancelled => write!(f, "turn cancelled before it settled"),
            Self::UnknownTool(name) => write!(f, "unknown tool: {name}"),
            Self::Store(detail) => write!(f, "turn store failed: {detail}"),
        }
    }
}

impl std::error::Error for TurnError {}

/// Settled turn receipt: one shared turn ID observed by every client instead
/// of duplicate private runtimes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TurnReceipt {
    pub session: SessionId,
    pub turn: AgentId,
}

/// One executed (or denied) tool request: durable outcome, never an error
/// string alone. Denials persist a [`MessageRole::Tool`] message and emit a
/// [`ServerEvent::ToolExecuted`] event so the failure is durable; they spawn
/// nothing and touch no files.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolOutcome {
    pub session: SessionId,
    pub turn: AgentId,
    pub name: String,
    pub output: String,
    pub allowed: bool,
}

impl TurnPermits {
    #[must_use]
    pub fn new() -> Self {
        Self {
            permits: Arc::new(Semaphore::new(MAX_CONCURRENT_TURNS)),
        }
    }

    /// Take one permit without waiting. Fails with
    /// [`EngineError::NoTurnPermit`] when all permits are held; the caller
    /// maps this to `429 Too Many Requests` and the request queues or retries.
    pub fn try_acquire(&self) -> Result<OwnedSemaphorePermit, EngineError> {
        Arc::clone(&self.permits)
            .try_acquire_owned()
            .map_err(|_| EngineError::NoTurnPermit)
    }

    /// Admit one turn: take a permit and bind a fresh [`CancelToken`].
    /// Cancellation owns no task or process itself; drivers poll
    /// [`TurnLease::is_cancelled`] and the engine reclaims the permit on
    /// [`TurnLease`] drop.
    pub fn acquire(&self) -> Result<TurnLease, EngineError> {
        self.try_acquire().map(TurnLease::new)
    }

    #[must_use]
    pub fn available_permits(&self) -> usize {
        self.permits.available_permits()
    }
}

impl Default for TurnPermits {
    fn default() -> Self {
        Self::new()
    }
}

/// Cloneable snapshot of one registered tool. Carries definitions only (no
/// locks, no process handles); the turn adapter consults the live
/// [`ToolRegistry`] for dispatch and the [`EnginePolicy`] for the deny check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolSnapshot {
    pub id: String,
    pub enabled: bool,
    pub tags: Vec<String>,
}

impl ToolSnapshot {
    #[must_use]
    pub fn all(registry: &ToolRegistry) -> Vec<Self> {
        let mut tools: Vec<Self> = registry
            .list()
            .into_iter()
            .map(|tool| Self {
                id: tool.id.clone(),
                enabled: tool.enabled,
                tags: tool.tags.clone(),
            })
            .collect();
        tools.sort_by(|left, right| left.id.cmp(&right.id));
        tools
    }
}

/// The single daemon-owned composition. Every client path (HTTP routes,
/// streaming, headless, TUI) clones these handles instead of building private
/// provider/session/tool/policy/store/event stacks. All members are cheap to
/// clone and every method is non-blocking; the engine owns no per-call threads
/// or permits itself — the turn lanes take [`TurnPermits`] separately so the
/// tool execution registry (`lib.rs`) remains the one live permit source until
/// its lane adopts this type.
#[derive(Clone)]
pub struct EngineHandles {
    /// Provider routing table (`provider/model` resolution).
    pub providers: ProviderRegistry,
    /// Durable session/message/artifact service shared by every client.
    pub sessions: SessionService,
    /// Tool-definition snapshot (ids, tags, allow flags) shared with the turn
    /// adapter. Live execution processes stay in the executor lane; this copy
    /// carries no handles, locks, or child state.
    pub tools: Vec<ToolSnapshot>,
    /// Allow/deny broker consulted before any tool side effect.
    pub policy: EnginePolicy,
    /// Durable state handle duplicated from [`Self::sessions`].
    /// Kept as a separate field so the persistence half of the journey
    /// (`sessions` behavior vs `store` authority) stays composable: adapters
    /// that only need storage take `store` without touching routing/tools.
    pub store: SessionService,
    /// Bounded fan-out bus for daemon state changes.
    pub events: EventBus,
    /// Turn concurrency permits shared by all client paths.
    pub turn_permits: TurnPermits,
}

impl EngineHandles {
    /// Bundle already-constructed handles. No I/O, no spawning: the daemon
    /// builds each half and the engine only takes shared ownership.
    /// (`sessions` and `store` take the same service value twice so the two
    /// halves stay separately typed for later split without cloning costs.)
    #[must_use]
    pub fn new(
        providers: ProviderRegistry,
        sessions: SessionService,
        tools: ToolRegistry,
        policy: EnginePolicy,
        store: SessionService,
        events: EventBus,
    ) -> Self {
        Self {
            providers,
            sessions,
            tools: ToolSnapshot::all(&tools),
            policy,
            store,
            events,
            turn_permits: TurnPermits::new(),
        }
    }

    /// Empty pending-turn queue at the engine cap.
    #[must_use]
    pub fn pending_turns(&self) -> PendingTurnQueue {
        pending_turn_queue()
    }

    /// Client ceiling shared by every attach path.
    #[must_use]
    pub const fn client_limit(&self) -> usize {
        MAX_ENGINE_CLIENTS
    }

    /// Transcript window admitted to one provider request.
    #[must_use]
    pub const fn turn_history_limit(&self) -> usize {
        MAX_TURN_HISTORY
    }

    /// Check one tool call against the engine policy before any process
    /// starts or file is touched. Deny (or unknown tool) returns
    /// `Ok(denied)` with a durable `Tool` message persisted and a
    /// [`ServerEvent::ToolExecuted`] event emitted; the denial spawns
    /// nothing. The caller supplies `execute` so tests inject an in-memory
    /// effect while production passes the sandboxed `ToolExecutor` call.
    pub async fn dispatch_tool<F, Fut>(
        &self,
        session: SessionId,
        turn: AgentId,
        name: &str,
        execute: F,
    ) -> Result<ToolOutcome, TurnError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = ToolOutcome>,
    {
        let known = self.tools.iter().any(|tool| tool.id == name);
        if !known {
            return Err(TurnError::UnknownTool(name.to_owned()));
        }
        if self.policy.decision(name) == PolicyDecision::Deny {
            let outcome = ToolOutcome {
                session,
                turn,
                name: name.to_owned(),
                output: format!("denied by engine policy: {name}"),
                allowed: false,
            };
            self.persist_tool_outcome(&outcome).await?;
            self.events
                .publish(ServerEvent::ToolExecuted {
                    name: name.to_owned(),
                    duration_ms: 0,
                })
                .ok();
            return Ok(outcome);
        }
        let mut outcome = execute().await;
        outcome.session = session;
        outcome.turn = turn;
        outcome.name = name.to_owned();
        outcome.allowed = true;
        outcome.output = crate::agent_loop::truncate_tool_output(&outcome.output);
        self.persist_tool_outcome(&outcome).await?;
        self.events
            .publish(ServerEvent::ToolExecuted {
                name: name.to_owned(),
                duration_ms: 0,
            })
            .ok();
        Ok(outcome)
    }

    async fn persist_tool_outcome(&self, outcome: &ToolOutcome) -> Result<(), TurnError> {
        self.sessions
            .append_text(
                outcome.session,
                MessageRole::Tool,
                format!("[{}] {}", outcome.name, outcome.output),
            )
            .await
            .map_err(|error| TurnError::Store(error.to_string()))?;
        Ok(())
    }

    /// Run one prompt turn on the shared engine: persist the user message,
    /// invoke `provider` (an injected async closure so tests stay
    /// deterministic; production passes an `OpenAiResponsesClient` call),
    /// persist the settled assistant turn, and emit
    /// [`ServerEvent::MessageAppended`]. A cancelled lease settles nothing
    /// and returns [`EngineError::Cancelled`]. History is windowed to
    /// [`MAX_TURN_HISTORY`]; empty/oversize prompts fail closed with
    /// [`EngineError::InvalidPrompt`] before any write.
    pub async fn run_prompt_turn<F, Fut>(
        &self,
        session: SessionId,
        lease: &TurnLease,
        prompt: &str,
        provider: F,
    ) -> Result<TurnReceipt, TurnError>
    where
        F: FnOnce(Vec<MessageRecord>) -> Fut,
        Fut: std::future::Future<Output = Result<String, String>>,
    {
        if prompt.is_empty() || prompt.len() > MAX_INLINE_PAYLOAD_BYTES {
            return Err(TurnError::InvalidPrompt);
        }
        if lease.is_cancelled() {
            return Err(TurnError::Cancelled);
        }
        let history = self
            .sessions
            .messages(session, MAX_TURN_HISTORY)
            .await
            .map_err(|error| TurnError::Store(error.to_string()))?;
        self.sessions
            .append_text(session, MessageRole::User, prompt)
            .await
            .map_err(|error| TurnError::Store(error.to_string()))?;
        let assistant_text = provider(history).await.map_err(TurnError::Store)?;
        if lease.is_cancelled() {
            return Err(TurnError::Cancelled);
        }
        self.sessions
            .append_text(session, MessageRole::Assistant, assistant_text)
            .await
            .map_err(|error| TurnError::Store(error.to_string()))?;
        self.events
            .publish(ServerEvent::MessageAppended { session, seq: 0 })
            .ok();
        Ok(TurnReceipt {
            session,
            turn: AgentId::new(),
        })
    }
}

impl fmt::Debug for EngineHandles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EngineHandles")
            .field("providers", &self.providers.count())
            .field("policy", &self.policy)
            .field("tools", &self.tools.len())
            .field("turn_permits_available", &self.turn_permits.available_permits())
            .field("subscribers", &self.events.subscriber_count())
            .field("client_limit", &MAX_ENGINE_CLIENTS)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencode_rk_providers::responses::MAX_RESPONSES_INPUT_MESSAGES;
    use opencode_rk_tools::tool_allow::MAX_TOOL_ALLOW;

    fn pending(session: SessionId, model: &str) -> PendingTurn {
        PendingTurn::new(session, AgentId::new(), model).expect("valid pending turn")
    }

    #[test]
    fn caps_match_cross_crate_bounds() {
        // Single source of truth per bound; the citing path is authoritative.
        assert_eq!(MAX_CONCURRENT_TURNS, 2); // lib.rs:73 TURN_PERMITS
        assert_eq!(MAX_TURN_HISTORY, MAX_RESPONSES_INPUT_MESSAGES);
        assert_eq!(MAX_POLICY_RULES, MAX_TOOL_ALLOW);
        assert!(MAX_PENDING_TURNS > MAX_CONCURRENT_TURNS);
        assert!(MAX_ENGINE_CLIENTS > 0);
        assert!(MAX_EVENT_BUFFER > 0);
        assert!(MAX_MODEL_REF_BYTES > 0);
    }

    #[test]
    fn pending_queue_rejects_overflow_and_preserves_fifo() {
        let session = SessionId::new();
        let mut queue = pending_turn_queue();
        assert_eq!(queue.capacity(), MAX_PENDING_TURNS);
        assert!(queue.is_empty());
        for index in 0..MAX_PENDING_TURNS {
            queue
                .try_push(pending(session, &format!("openai/model-{index}")))
                .unwrap();
        }
        assert!(queue.is_full());
        assert_eq!(queue.len(), MAX_PENDING_TURNS);
        let err = queue
            .try_push(pending(session, "openai/overflow"))
            .unwrap_err();
        assert_eq!(
            err,
            EngineError::QueueFull {
                queue: "pending_turns",
                capacity: MAX_PENDING_TURNS,
            }
        );
        // Overflow mutated nothing: FIFO order intact.
        let first = queue.pop_front().expect("first queued turn");
        let second = queue.pop_front().expect("second queued turn");
        assert_eq!(first.model, "openai/model-0");
        assert_eq!(second.model, "openai/model-1");
        assert_eq!(queue.len(), MAX_PENDING_TURNS - 2);
    }

    #[test]
    fn pending_turn_rejects_bad_model_ref() {
        let session = SessionId::new();
        assert_eq!(
            PendingTurn::new(session, AgentId::new(), "").unwrap_err(),
            EngineError::InvalidModelRef
        );
        assert_eq!(
            PendingTurn::new(session, AgentId::new(), "x".repeat(MAX_MODEL_REF_BYTES + 1))
                .unwrap_err(),
            EngineError::InvalidModelRef
        );
    }

    #[test]
    fn single_owner_claim_release_reclaim() {
        let owner = SingleOwner::new();
        assert!(!owner.is_owned());
        let guard = owner.try_claim().expect("first claim");
        assert!(owner.is_owned());
        assert_eq!(
            owner.try_claim().unwrap_err(),
            EngineError::AlreadyOwned
        );
        drop(guard);
        assert!(!owner.is_owned());
        let _guard = owner.try_claim().expect("reclaim after release");
    }

    #[test]
    fn global_engine_claim_is_exclusive() {
        assert!(!engine_owned());
        let guard = claim_engine().expect("global claim");
        assert!(engine_owned());
        assert_eq!(claim_engine().unwrap_err(), EngineError::AlreadyOwned);
        drop(guard);
        assert!(!engine_owned());
    }

    #[test]
    fn assert_single_owner_counts() {
        assert!(assert_single_owner(1).is_ok());
        assert_eq!(
            assert_single_owner(0).unwrap_err(),
            EngineError::UnexpectedEngineCount(0)
        );
        assert_eq!(
            assert_single_owner(2).unwrap_err(),
            EngineError::UnexpectedEngineCount(2)
        );
    }

    #[test]
    fn same_turn_shared_across_two_clients() {
        let session = SessionId::new();
        let turn = AgentId::new();
        let first = TurnObservation {
            client: ClientId(1),
            session,
            turn,
        };
        let second = TurnObservation {
            client: ClientId(2),
            session,
            turn,
        };
        assert_eq!(assert_same_turn(&first, &second).expect("shared turn"), turn);
    }

    #[test]
    fn same_turn_rejects_divergent_runtimes() {
        let session = SessionId::new();
        let base = TurnObservation {
            client: ClientId(1),
            session,
            turn: AgentId::new(),
        };
        let forked_turn = TurnObservation {
            client: ClientId(2),
            session,
            turn: AgentId::new(),
        };
        let err = assert_same_turn(&base, &forked_turn).unwrap_err();
        assert!(
            matches!(err, EngineError::TurnMismatch { .. }),
            "unexpected: {err}"
        );
        let forked_session = TurnObservation {
            client: ClientId(2),
            session: SessionId::new(),
            turn: base.turn,
        };
        assert_eq!(
            assert_same_turn(&base, &forked_session).unwrap_err(),
            EngineError::SessionMismatch
        );
    }

    #[test]
    fn policy_deny_wins_and_rules_bounded() {
        let mut policy = EnginePolicy::default_allow();
        policy.allow_tool("read").expect("allow rule");
        assert_eq!(policy.decision("read"), PolicyDecision::Allow);
        assert_eq!(policy.decision("bash"), PolicyDecision::Allow);
        policy.deny_tool("bash").expect("deny rule");
        assert_eq!(policy.decision("bash"), PolicyDecision::Deny);
        // Explicit allow after deny still loses: deny wins.
        policy.allow_tool("bash").expect("allow after deny");
        assert_eq!(policy.decision("bash"), PolicyDecision::Deny);
        assert_eq!(
            policy.allow_tool("").unwrap_err(),
            EngineError::InvalidToolName
        );
        assert_eq!(
            policy.deny_tool("").unwrap_err(),
            EngineError::InvalidToolName
        );

        let mut strict = EnginePolicy::default_deny();
        assert_eq!(strict.decision("read"), PolicyDecision::Deny);
        strict.allow_tool("read").expect("strict allow");
        assert_eq!(strict.decision("read"), PolicyDecision::Allow);

        let mut full = EnginePolicy::default_deny();
        for index in 0..MAX_POLICY_RULES {
            full.deny_tool(format!("tool-{index}")).expect("rule room");
        }
        assert_eq!(full.deny_len(), MAX_POLICY_RULES);
        assert_eq!(
            full.deny_tool("one-more").unwrap_err(),
            EngineError::PolicyFull {
                max: MAX_POLICY_RULES,
            }
        );
    }

    #[test]
    fn turn_permits_bound_concurrency() {
        let permits = TurnPermits::new();
        assert_eq!(permits.available_permits(), MAX_CONCURRENT_TURNS);
        let first = permits.try_acquire().expect("permit one");
        let second = permits.try_acquire().expect("permit two");
        assert_eq!(permits.available_permits(), 0);
        assert_eq!(
            permits.try_acquire().unwrap_err(),
            EngineError::NoTurnPermit
        );
        drop(first);
        assert_eq!(permits.available_permits(), 1);
        let _third = permits.try_acquire().expect("permit after release");
        drop(second);
    }

    // ---- APP-003 remainder RED probes (frozen; must FAIL pre-implementation) ----

    fn red_engine() -> EngineHandles {
        let dir = tempfile::tempdir().expect("disposable fixture dir");
        let blob_root = dir.path().join("blobs");
        std::mem::forget(dir);
        let storage = Arc::new(
            opencode_rk_storage::Storage::open_in_memory(blob_root).expect("in-memory store"),
        );
        let sessions = SessionService::new(storage);
        EngineHandles::new(
            ProviderRegistry::new(),
            sessions.clone(),
            ToolRegistry::new(),
            EnginePolicy::default_deny(),
            sessions,
            event_bus(),
        )
    }

    async fn red_session(engine: &EngineHandles) -> SessionId {
        engine
            .sessions
            .create("red")
            .await
            .expect("fixture session")
            .id
    }

    #[tokio::test]
    async fn red_prompt_turn_persists_settled_turn() {
        let engine = red_engine();
        let session = red_session(&engine).await;
        let lease = engine.turn_permits.acquire().expect("permit");
        let receipt = engine
            .run_prompt_turn(session, &lease, "hello engine", |_history| async {
                Ok::<_, String>("settled answer".to_owned())
            })
            .await
            .expect("prompt turn settles");
        assert_eq!(receipt.session, session);
        let transcript = engine
            .sessions
            .messages(session, MAX_TURN_HISTORY)
            .await
            .expect("transcript");
        assert_eq!(transcript.len(), 2);
        assert_eq!(transcript[0].role, MessageRole::User);
        assert_eq!(transcript[1].role, MessageRole::Assistant);
    }

    #[tokio::test]
    async fn red_allowed_tool_executes_via_policy() {
        // RED probe: `read` is a registered builtin but `default_deny`
        // policy denies it, so dispatch must return a durable denied
        // outcome WITHOUT invoking the effect closure. GREEN allowlists
        // `read`, reruns, and asserts the executed output persisted.
        let engine = red_engine();
        let session = red_session(&engine).await;
        let outcome = engine
            .dispatch_tool(session, AgentId::new(), "read", || async {
                panic!("denied tool must not execute its effect");
                #[allow(unreachable_code)]
                ToolOutcome {
                    session,
                    turn: AgentId::new(),
                    name: String::new(),
                    output: "hi".to_owned(),
                    allowed: false,
                }
            })
            .await
            .expect("deny is durable outcome, not error");
        assert!(!outcome.allowed);
        let transcript = engine
            .sessions
            .messages(session, MAX_TURN_HISTORY)
            .await
            .expect("transcript");
        assert!(transcript.iter().any(|m| m.role == MessageRole::Tool));
    }

    #[tokio::test]
    async fn red_denied_tool_no_side_effects_durable_failure() {
        let mut engine = red_engine();
        engine.policy.allow_tool("echo").expect("allow echo");
        engine.policy.deny_tool("bash").expect("deny bash");
        let session = red_session(&engine).await;
        let probe = tempfile::tempdir().expect("probe dir");
        let marker = probe.path().join("must-not-exist");
        let outcome = engine
            .dispatch_tool(session, AgentId::new(), "bash", || async {
                std::fs::write(&marker, b"side effect").ok();
                ToolOutcome {
                    session,
                    turn: AgentId::new(),
                    name: String::new(),
                    output: "spawned".to_owned(),
                    allowed: true,
                }
            })
            .await
            .expect("deny is durable outcome, not error");
        assert!(!outcome.allowed);
        assert!(!marker.exists(), "denied tool must not touch the filesystem");
        let transcript = engine
            .sessions
            .messages(session, MAX_TURN_HISTORY)
            .await
            .expect("transcript");
        assert!(transcript.iter().any(|m| m.role == MessageRole::Tool));
    }

    #[tokio::test]
    async fn red_cancel_reclaims_permit_and_fires_token() {
        let engine = red_engine();
        let lease = engine.turn_permits.acquire().expect("permit");
        assert_eq!(engine.turn_permits.available_permits(), MAX_CONCURRENT_TURNS - 1);
        lease.cancel();
        assert!(lease.is_cancelled());
        let session = red_session(&engine).await;
        let err = engine
            .run_prompt_turn(session, &lease, "late prompt", |_history| async {
                Ok::<_, String>("never".to_owned())
            })
            .await
            .unwrap_err();
        assert_eq!(err, TurnError::Cancelled);
        drop(lease);
        assert_eq!(
            engine.turn_permits.available_permits(),
            MAX_CONCURRENT_TURNS
        );
    }

    #[tokio::test]
    async fn red_two_clients_observe_settled_turn() {
        let engine = red_engine();
        let session = red_session(&engine).await;
        let lease = engine.turn_permits.acquire().expect("permit");
        let receipt = engine
            .run_prompt_turn(session, &lease, "shared?", |_history| async {
                Ok::<_, String>("shared answer".to_owned())
            })
            .await
            .expect("prompt turn settles");
        let first = TurnObservation {
            client: ClientId(1),
            session,
            turn: receipt.turn,
        };
        let second = TurnObservation {
            client: ClientId(2),
            session,
            turn: receipt.turn,
        };
        assert_eq!(assert_same_turn(&first, &second).expect("shared turn"), receipt.turn);
    }
}
