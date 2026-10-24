//! Router-owned, bounded child turns. All child effects use the ordinary turn
//! engine and child-bound permission broker; this module never executes a tool.
use super::*;
use std::{collections::{BTreeMap, VecDeque}, sync::{Mutex, atomic::{AtomicBool, AtomicU32, Ordering}}};
use tokio::sync::watch;

use crate::builtin_agents;

const MAX_CHILDREN: usize = 128;
const MAX_TAIL: usize = 8192;
const MAX_FOLLOWUPS: usize = 4;
const MAX_MANUAL_RETRIES: u32 = 3;
const MANUAL_RETRY_EXHAUSTION: &str = "manual retry budget exhausted (maximum 3 retries)";
/// Bounded automatic admission retry: only pre-side-effect capacity refusals
/// are retried, at exponential backoff, ending in a visible final error.
const MAX_ADMISSION_ATTEMPTS: u32 = 5;
const ADMISSION_BACKOFF_BASE_MS: u64 = 100;
const MAX_PROVIDER_RETRIES: u32 = 3;

#[derive(Clone)]
pub(crate) struct Children(Arc<Mutex<BTreeMap<String, Arc<Child>>>>, mcp_http::McpStatusRegistry);
impl Default for Children { fn default() -> Self { Self(Arc::new(Mutex::new(BTreeMap::new())), mcp_http::McpStatusRegistry::new()) } }
impl Children {
    pub(crate) fn with_mcp(registry: mcp_http::McpStatusRegistry) -> Self { Self(Arc::new(Mutex::new(BTreeMap::new())), registry) }
    pub(crate) fn mcp_registry(&self) -> mcp_http::McpStatusRegistry { self.1.clone() }
}

struct Child {
    parent: SessionId,
    id: SessionId,
    description: String,
    agent: String,
    model: String,
    prompt: String,
    workdir: String,
    agent_prompt: String,
    read_only: bool,
    status: Mutex<Status>,
    control: Mutex<GenerationControl>,
    tool_seen: AtomicBool,
    generation: AtomicU32,
    /// The one durable user message admitted for the current generation.
    /// Kept in the child registry so an internal retry context cannot be
    /// reused for another generation or another registered child.
    auto_retries: AtomicU32,
    manual_retries: AtomicU32,
    followups: Mutex<VecDeque<String>>,
}
struct Status { state: &'static str, output: String, error: Option<String> }
struct GenerationControl {
    generation: u32,
    chain_id: u64,
    stop: watch::Sender<bool>,
    finished: watch::Sender<bool>,
    claimed: bool,
    retry_message: Option<(u32, MessageId)>,
    explicit_cancelled: bool,
}
struct GenerationClaim {
    generation: u32,
    chain_id: u64,
    stopped: watch::Receiver<bool>,
    finished: watch::Sender<bool>,
}
struct ForegroundClaim { chain_id: u64, finished: watch::Receiver<bool> }

impl GenerationClaim {
    fn foreground(&self) -> ForegroundClaim {
        ForegroundClaim { chain_id: self.chain_id, finished: self.finished.subscribe() }
    }
}

impl Child {
    fn finished(&self) -> bool {
        self.control.lock().map_or(true, |control| *control.finished.borrow())
    }
    fn claim_generation_receipt(&self, manual_retry: bool) -> Result<GenerationClaim, ApiFailure> {
        let (stop, _) = watch::channel(false);
        let (finished, _) = watch::channel(false);
        let mut control = self.control.lock().expect("child control unavailable");
        if control.claimed || !*control.finished.borrow() {
            return Err(ApiFailure::bad_request("child task is still running"));
        }
        if manual_retry && self.manual_retries.load(Ordering::Acquire) >= MAX_MANUAL_RETRIES {
            return Err(ApiFailure::too_many_requests(MANUAL_RETRY_EXHAUSTION));
        }
        let next_generation = control.generation.checked_add(1).ok_or_else(|| ApiFailure::internal("child generation exhausted"))?;
        let next_chain = control.chain_id.checked_add(1).ok_or_else(|| ApiFailure::internal("child chain exhausted"))?;
        control.generation = next_generation;
        control.chain_id = next_chain;
        control.stop = stop;
        control.finished = finished;
        control.claimed = true;
        control.explicit_cancelled = false;
        if manual_retry { self.manual_retries.fetch_add(1, Ordering::AcqRel); }
        Ok(GenerationClaim { generation: control.generation, chain_id: control.chain_id, stopped: control.stop.subscribe(), finished: control.finished.clone() })
    }
    fn claim_generation(&self, manual_retry: bool) -> Result<(u32, watch::Receiver<bool>, watch::Sender<bool>), ApiFailure> {
        let claim = self.claim_generation_receipt(manual_retry)?;
        Ok((claim.generation, claim.stopped, claim.finished))
    }
    /// Atomically moves an already-owned worker to its queued continuation.
    /// The completion signal is retained so a waiter for the logical child
    /// cannot be orphaned between generations; only the cancellation token is
    /// replaced.
    fn current_control(&self) -> (u32, watch::Sender<bool>, watch::Receiver<bool>) {
        let control = self.control.lock().expect("child control unavailable");
        (control.generation, control.stop.clone(), control.finished.subscribe())
    }
    fn current_chain(&self) -> (u64, watch::Receiver<bool>) {
        let control = self.control.lock().expect("child control unavailable");
        (control.chain_id, control.finished.subscribe())
    }
    fn stop_receiver(&self) -> watch::Receiver<bool> {
        self.control.lock().expect("child control unavailable").stop.subscribe()
    }
    fn is_stopped(&self) -> bool {
        self.control
            .lock()
            .map(|control| *control.stop.borrow())
            .unwrap_or(true)
    }
    fn explicit_owner_cancelled(&self) -> bool {
        self.control.lock().ok().is_some_and(|control| control.explicit_cancelled)
    }
    fn mark_explicit_owner_cancelled(&self) {
        if let Ok(mut control) = self.control.lock() { control.explicit_cancelled = true; }
    }
    fn mark_explicit_owner_cancelled_for(&self, generation: u32) {
        if let Ok(mut control) = self.control.lock() {
            if control.generation == generation { control.explicit_cancelled = true; }
        }
    }
    fn cancel_current(&self) -> (u32, watch::Receiver<bool>) {
        let control = self.control.lock().expect("child control unavailable");
        control.stop.send_replace(true);
        (control.generation, control.finished.subscribe())
    }
    fn cancel_generation(&self, generation: u32) {
        if let Ok(control) = self.control.lock() {
            if control.generation == generation { control.stop.send_replace(true); }
        }
    }
    fn cancel_chain(&self, chain_id: u64) {
        if let Ok(control) = self.control.lock() {
            if control.chain_id == chain_id { control.stop.send_replace(true); }
        }
    }
    fn enqueue_followup(&self, text: String) -> Result<bool, ApiFailure> {
        let control = self.control.lock().map_err(|_| ApiFailure::internal("child control unavailable"))?;
        if *control.stop.borrow() || !control.claimed {
            return Ok(false);
        }
        let mut queue = self.followups.lock().map_err(|_| ApiFailure::internal("child state unavailable"))?;
        if queue.len() >= MAX_FOLLOWUPS {
            return Err(ApiFailure::too_many_requests(format!("child follow-up queue is full (max {MAX_FOLLOWUPS})")));
        }
        queue.push_back(text);
        Ok(true)
    }
    fn settle_or_claim_followup(&self, generation: u32) -> Result<Option<(String, u32, watch::Receiver<bool>)>, ApiFailure> {
        let mut control = self.control.lock().map_err(|_| ApiFailure::internal("child control unavailable"))?;
        if control.generation != generation || !control.claimed { return Ok(None); }
        if *control.stop.borrow() { control.claimed = false; control.finished.send_replace(true); return Ok(None); }
        let mut queue = self.followups.lock().map_err(|_| ApiFailure::internal("child state unavailable"))?;
        let Some(text) = queue.pop_front() else { control.claimed = false; control.finished.send_replace(true); return Ok(None); };
        let (stop, _) = watch::channel(false);
        control.generation = control.generation.checked_add(1).ok_or_else(|| ApiFailure::internal("child generation exhausted"))?;
        control.stop = stop;
        control.retry_message = None;
        Ok(Some((text, control.generation, control.stop.subscribe())))
    }

    fn finish_generation(&self, generation: u32) -> bool {
        let mut control = match self.control.lock() { Ok(control) => control, Err(_) => return false };
        if control.generation != generation { return false; }
        control.claimed = false;
        control.finished.send_replace(true);
        true
    }
}

impl Children {
    pub fn can_continue_after_write_rejection(&self, id: SessionId) -> bool {
        // Unlike admission's conservative child-lane classification, this
        // grants a behavior exception and therefore requires a known identity.
        self.0.lock().is_ok_and(|rows| rows.contains_key(&id.to_string()))
    }

    pub fn is_child(&self, id: SessionId) -> bool {
        self.0.lock().map_or(true, |rows| rows.contains_key(&id.to_string()))
    }
    fn child(&self, parent: SessionId, id: &str) -> Result<Arc<Child>, ApiFailure> {
        self.0.lock().map_err(|_| ApiFailure::internal("child registry unavailable"))?
            .get(id).filter(|entry| entry.parent == parent).cloned()
            .ok_or_else(|| ApiFailure::not_found("child task not found"))
    }

    pub(crate) fn validate_retry_context(
        &self,
        id: SessionId,
        generation: u32,
        message_id: MessageId,
    ) -> Result<(), ApiFailure> {
        let child = self
            .0
            .lock()
            .map_err(|_| ApiFailure::internal("child registry unavailable"))?
            .get(&id.to_string())
            .cloned()
            .ok_or_else(|| ApiFailure::bad_request("retry context is not a registered child"))?;
        if child.generation.load(Ordering::Acquire) != generation || generation == 0 {
            return Err(ApiFailure::bad_request("retry context generation is stale"));
        }
        let recorded = child.control.lock().map_err(|_| ApiFailure::internal("child state unavailable"))?;
        if recorded.generation != generation || recorded.retry_message != Some((generation, message_id)) {
            return Err(ApiFailure::bad_request("retry context message is not owned by this generation"));
        }
        Ok(())
    }

    pub(crate) fn record_retry_message(
        &self,
        id: SessionId,
        generation: u32,
        message_id: MessageId,
    ) -> Result<(), ApiFailure> {
        let child = self
            .0
            .lock()
            .map_err(|_| ApiFailure::internal("child registry unavailable"))?
            .get(&id.to_string())
            .cloned()
            .ok_or_else(|| ApiFailure::bad_request("retry context is not a registered child"))?;
        let mut recorded = child.control.lock().map_err(|_| ApiFailure::internal("child state unavailable"))?;
        if recorded.generation != generation || !recorded.claimed { return Err(ApiFailure::bad_request("retry context generation is stale")); }
        recorded.retry_message = Some((generation, message_id));
        Ok(())
    }
    fn rows(&self, parent: SessionId) -> Result<Vec<Arc<Child>>, ApiFailure> {
        Ok(self.0.lock().map_err(|_| ApiFailure::internal("child registry unavailable"))?
            .values().filter(|entry| entry.parent == parent).cloned().collect())
    }
    pub fn pending(&self, parent: SessionId) -> Vec<(SessionId, PermissionRequest)> {
        self.rows(parent).unwrap_or_default().into_iter().filter_map(|entry| {
            run_owner::active(entry.id).and_then(|owner| owner.pending()).map(|pending| (entry.id, pending))
        }).collect()
    }
    pub fn permission_owner(&self, parent: SessionId, request: &str) -> Option<Arc<RunOwner>> {
        self.pending(parent).into_iter().find(|(_, pending)| pending.id == request)
            .and_then(|(id, _)| run_owner::active(id))
    }
}

#[cfg(test)]
mod rejection_continuation_tests {
    use super::*;

    #[test]
    fn poisoned_registry_cannot_authorize_rejection_continuation() {
        let children = Children::default();
        let root = SessionId::new();
        assert!(!children.can_continue_after_write_rejection(root));
        let registry = children.0.clone();
        assert!(std::panic::catch_unwind(move || {
            let _guard = registry.lock().unwrap();
            panic!("poison disposable fixture registry");
        }).is_err());
        assert!(!children.can_continue_after_write_rejection(root),
            "unknown identity must retain rejection-stop behavior");
    }
}

fn snapshot(child: &Child) -> Value {
    let followups = child.followups.lock().map_or(0, |queue| queue.len());
    match child.status.lock() {
        Ok(status) => json!({"session_id":child.id,"parent_id":child.parent,"description":child.description,
            "agent":child.agent,"model":child.model,"read_only":child.read_only,"state":if status.state == "queued" && run_owner::active(child.id).is_some() { "running" } else { status.state },"output_tail":status.output,
            "error":status.error,"has_tool_calls":child.tool_seen.load(Ordering::Acquire),
            "generation":child.generation.load(Ordering::Acquire),
            "auto_retry_attempts":child.auto_retries.load(Ordering::Acquire),
            "manual_retries":child.manual_retries.load(Ordering::Acquire),
            "pending_followups":followups,
            "awaiting_permission":run_owner::active(child.id).is_some_and(|owner| owner.pending().is_some())}),
        Err(_) => json!({"session_id":child.id,"state":"unknown","error":"child state unavailable"}),
    }
}

pub(crate) fn schema() -> ResponsesTool {
    ResponsesTool::function("task", "Delegate a bounded child session. Predefined: explore, research, planner, reviewer, plan are enforceably read-only; general, build, implementer, debug and tester use the full tool loop. Configured agents and explicit prompts are honored. Background tasks can be inspected, followed up and retried through the Subagents panel.",
        json!({"type":"object","properties":{"description":{"type":"string","maxLength":160},"prompt":{"type":"string","maxLength":32768},"subagent_type":{"type":"string"},"task_id":{"type":"string"},"background":{"type":"boolean"}},"required":["description","prompt","subagent_type"],"additionalProperties":false}))
}

pub(crate) async fn list(State(state): State<AppState>, Path(id): Path<String>, Extension(children): Extension<Children>) -> Result<Json<Value>, ApiFailure> {
    let parent = parse_session_id(&id)?;
    state.sessions.get(parent).await.map_err(ApiFailure::internal)?;
    Ok(Json(json!({"children":children.rows(parent)?.iter().map(|entry| snapshot(entry)).collect::<Vec<_>>(),
        "registry_scope":"current daemon lifetime; child transcripts remain durable"})))
}

pub(crate) async fn stop(Path((parent, id)): Path<(String, String)>, Extension(children): Extension<Children>) -> Result<Json<Value>, ApiFailure> {
    let child = children.child(parse_session_id(&parent)?, &id)?;
    let (_, mut finished) = child.cancel_current();
    while !*finished.borrow_and_update() {
        if finished.changed().await.is_err() { break; }
    }
    Ok(Json(snapshot(&child)))
}

#[derive(Deserialize)]
pub(crate) struct Followup { text: String }

pub(crate) async fn retry(State(state): State<AppState>, Path((parent, id)): Path<(String, String)>, Extension(children): Extension<Children>, Extension(events): Extension<event_bus::EventBus>) -> Result<Json<Value>, ApiFailure> {
    let child = children.child(parse_session_id(&parent)?, &id)?;
    // A cancellation may win before the response consumer observes a tool
    // event. The durable admission ledger, not just that consumer's flag,
    // decides whether a replay could repeat an effect.
    if child.tool_seen.load(Ordering::Acquire)
        || state.sessions.has_tool_admissions(child.id).await.map_err(ApiFailure::internal)? {
        return Err(ApiFailure::bad_request("retry refused: a tool was dispatched; use an explicit follow-up after inspecting history"));
    }
    let prompt = child.prompt.clone();
    let _ = restart(state, children, events, child.clone(), prompt, true, true)?;
    Ok(Json(snapshot(&child)))
}

pub(crate) async fn followup(State(state): State<AppState>, Path((parent, id)): Path<(String, String)>, Extension(children): Extension<Children>, Extension(events): Extension<event_bus::EventBus>, Json(body): Json<Followup>) -> Result<Json<Value>, ApiFailure> {
    if body.text.trim().is_empty() || body.text.len() > 32768 { return Err(ApiFailure::bad_request("follow-up must contain 1..32768 bytes")); }
    let child = children.child(parse_session_id(&parent)?, &id)?;
    let active = {
        let status = child.status.lock().map_err(|_| ApiFailure::internal("child state unavailable"))?;
        status.state == "running" || status.state == "queued" || !child.finished()
    };
    // A running or scheduled child accepts bounded follow-ups that run as a
    // fresh generation once the current one settles.
    if active {
        if !child.enqueue_followup(body.text)? { return Err(ApiFailure::bad_request("child task was cancelled")); }
        return Ok(Json(snapshot(&child)));
    }
    let _ = restart(state, children, events, child.clone(), body.text, false, false)?;
    Ok(Json(snapshot(&child)))
}

fn restart(state: AppState, children: Children, events: event_bus::EventBus, child: Arc<Child>, prompt: String, replay: bool, manual_retry: bool) -> Result<ForegroundClaim, ApiFailure> {
    {
        let status = child.status.lock().map_err(|_| ApiFailure::internal("child state unavailable"))?;
        if status.state == "running" || !child.finished() {
            return Err(ApiFailure::bad_request("child task is still running"));
        }
        if status.state == "completed" && prompt == child.prompt {
            return Err(ApiFailure::bad_request("completed tasks cannot be retried; send an explicit follow-up"));
        }
    }
    // Claim before publishing queued state or releasing any lifecycle lock.
    // A competing restart therefore observes an unfinished claimed control
    // and cannot create a second producer.
    let claim = child.claim_generation_receipt(manual_retry)?;
    let foreground = claim.foreground();
    let mut status = child.status.lock().map_err(|_| ApiFailure::internal("child state unavailable"))?;
    *status = Status { state:"queued", output:String::new(), error:None };
    drop(status);
    spawn_claimed(state, children, events, child, prompt, replay, claim);
    Ok(foreground)
}

fn spawn(state: AppState, children: Children, events: event_bus::EventBus, child: Arc<Child>, prompt: String, replay: bool) -> Result<(), ApiFailure> {
    let claim = child.claim_generation_receipt(false)?;
    spawn_claimed(state, children, events, child, prompt, replay, claim);
    Ok(())
}

fn spawn_claimed(state: AppState, children: Children, events: event_bus::EventBus, child: Arc<Child>, prompt: String, replay: bool, claim: GenerationClaim) {
    let GenerationClaim { generation: mut generation_id, chain_id: _chain_id, mut stopped, mut finished } = claim;
    tokio::spawn(async move {
        child.generation.store(generation_id, Ordering::Release);
        child.auto_retries.store(0, Ordering::Release);
        let mut prompt = prompt;
        let mut replay = replay;
        // One generation per loop turn: bounded admission retries, a terminal
        // state, then at most one bounded queued follow-up continues as its
        // own fresh generation. Cancellation preserves the remaining queue.
        loop {
            let result = generation(state.clone(), children.clone(), events.clone(), child.clone(), prompt.clone(), replay).await;
            let cancelled = *stopped.borrow() || child.is_stopped() || child.explicit_owner_cancelled();
            if let Ok(mut status) = child.status.lock() {
                match &result {
                    Ok(()) => status.state = "completed",
                    Err(error) => {
                        status.state = if cancelled { "cancelled" } else { "error" };
                        status.error = Some(error.message.clone());
                    }
                }
            }
            if cancelled { break; }
            // A queued follow-up is a distinct continuation prompt, not a
            // blind replay of the original prompt.
            let next = match child.settle_or_claim_followup(generation_id) {
                Ok(next) => next,
                Err(error) => {
                    if let Ok(mut status) = child.status.lock() {
                        status.state = "error";
                        status.error = Some(error.message);
                    }
                    return;
                }
            };
            match next {
                Some((text, next_generation, next_stopped)) => {
                    prompt = text;
                    replay = false;
                    generation_id_for_child(&child, next_generation);
                    generation_id = next_generation;
                    stopped = next_stopped;
                    child.auto_retries.store(0, Ordering::Release);
                    if let Ok(mut status) = child.status.lock() {
                        status.state = "queued";
                        status.output.clear();
                        status.error = None;
                    }
                    continue;
                }
                None => break,
            }
        }
        child.finish_generation(generation_id);
    });
}

fn generation_id_for_child(child: &Child, generation: u32) {
    child.generation.store(generation, Ordering::Release);
}

#[cfg(test)]
mod generation_control_tests {
    use super::*;

    fn child_fixture() -> Arc<Child> {
        let (stop, _) = watch::channel(false);
        let (finished, _) = watch::channel(true);
        Arc::new(Child {
            parent: SessionId::new(),
            id: SessionId::new(),
            description: "generation control fixture".into(),
            agent: "general".into(),
            model: "openai/fixture".into(),
            prompt: "fixture prompt".into(),
            workdir: ".".into(),
            agent_prompt: "fixture agent".into(),
            read_only: false,
            status: Mutex::new(Status { state: "error", output: String::new(), error: Some("fixture failure".into()) }),
            control: Mutex::new(GenerationControl { generation: 0, chain_id: 0, stop, finished, claimed: false, retry_message: None, explicit_cancelled: false }),
            tool_seen: AtomicBool::new(false),
            generation: AtomicU32::new(0),
            auto_retries: AtomicU32::new(0),
            manual_retries: AtomicU32::new(0),
            followups: Mutex::new(VecDeque::new()),
        })
    }

    #[test]
    fn concurrent_claims_on_one_child_have_exactly_one_success() {
        let child = child_fixture();
        let gate = Arc::new(std::sync::Barrier::new(2));
        std::thread::scope(|scope| {
            let first = Arc::clone(&child);
            let first_gate = Arc::clone(&gate);
            let first = scope.spawn(move || { first_gate.wait(); first.claim_generation(false).is_ok() });
            let second = Arc::clone(&child);
            let second_gate = Arc::clone(&gate);
            let second = scope.spawn(move || { second_gate.wait(); second.claim_generation(false).is_ok() });
            let first_claimed = if first.join().unwrap() { 1 } else { 0 };
            let second_claimed = if second.join().unwrap() { 1 } else { 0 };
            assert_eq!(first_claimed + second_claimed, 1);
        });
    }

    #[test]
    fn old_stop_receiver_completes_without_awaiting_new_manual_generation() {
        let child = child_fixture();
        let (generation, _, _) = child.claim_generation(false).unwrap();
        let (_, mut old_waiter) = child.cancel_current();
        assert!(child.finish_generation(generation));
        assert!(*old_waiter.borrow_and_update());
        let (new_generation, _, new_finished) = child.claim_generation(true).unwrap();
        assert_eq!(new_generation, generation + 1);
        assert!(!*new_finished.borrow());
        assert!(*old_waiter.borrow_and_update());
    }

    #[test]
    fn stop_before_followup_settle_prevents_next_generation_and_signals_waiter() {
        let child = child_fixture();
        let (generation, _, _) = child.claim_generation(false).unwrap();
        assert!(child.enqueue_followup("must not run".into()).unwrap());
        let (_, mut waiter) = child.cancel_current();
        assert!(child.settle_or_claim_followup(generation).unwrap().is_none());
        assert!(*waiter.borrow_and_update());
        assert_eq!(child.followups.lock().unwrap().len(), 1);
        assert!(child.finished());
    }

    #[test]
    fn transition_before_stop_cancels_new_token_and_finishes_continuous_chain() {
        let child = child_fixture();
        let (first_generation, _, first_finished) = child.claim_generation(false).unwrap();
        let mut chain_waiter = first_finished.subscribe();
        assert!(child.enqueue_followup("continuation".into()).unwrap());
        let (_, next_generation, next_stopped) = child
            .settle_or_claim_followup(first_generation)
            .unwrap()
            .expect("follow-up must claim the next generation");
        let (_, mut current_waiter) = child.cancel_current();
        assert!(*next_stopped.borrow());
        assert!(child.settle_or_claim_followup(next_generation).unwrap().is_none());
        assert!(*current_waiter.borrow_and_update());
        assert!(*chain_waiter.borrow_and_update());
        assert!(child.finished());
    }

    #[test]
    fn captured_chain_cancel_stops_followup_generation_and_finishes_chain() {
        let child = child_fixture();
        let (first_generation, _, _) = child.claim_generation(false).unwrap();
        let (chain, _) = child.current_chain();
        assert!(child.enqueue_followup("chain continuation".into()).unwrap());
        let (_, next_generation, next_stopped) = child
            .settle_or_claim_followup(first_generation)
            .unwrap()
            .expect("follow-up must claim the retained logical chain");
        child.cancel_chain(chain);
        assert!(*next_stopped.borrow());
        assert!(child.settle_or_claim_followup(next_generation).unwrap().is_none());
        assert!(child.finished());
    }

    #[test]
    fn old_chain_cancel_does_not_stop_fresh_manual_generation() {
        let child = child_fixture();
        let (generation, _, _) = child.claim_generation(false).unwrap();
        let (old_chain, _) = child.current_chain();
        assert!(child.finish_generation(generation));
        let (_, old_waiter) = child.cancel_current();
        assert!(*old_waiter.borrow());
        let (manual_generation, manual_stopped, manual_finished) = child.claim_generation(true).unwrap();
        let (new_chain, new_finished) = child.current_chain();
        assert_eq!(manual_generation, generation + 1);
        assert_ne!(new_chain, old_chain);
        child.cancel_chain(old_chain);
        assert!(!*manual_stopped.borrow());
        assert!(!*manual_finished.borrow());
        assert!(!*new_finished.borrow());
    }

    #[test]
    fn claim_overflow_leaves_generation_control_unchanged() {
        let child = child_fixture();
        {
            let mut control = child.control.lock().unwrap();
            control.generation = 41;
            control.chain_id = u64::MAX;
            control.claimed = false;
            control.retry_message = Some((41, MessageId::new()));
            control.explicit_cancelled = false;
        }
        child.generation.store(41, Ordering::Release);
        child.manual_retries.store(2, Ordering::Release);
        let before = {
            let control = child.control.lock().unwrap();
            let stopped = *control.stop.borrow();
            let finished = *control.finished.borrow();
            (control.generation, control.chain_id, control.claimed, stopped, finished, control.retry_message)
        };
        let manual_before = child.manual_retries.load(Ordering::Acquire);
        assert!(child.claim_generation(true).is_err());
        let control = child.control.lock().unwrap();
        assert_eq!((control.generation, control.chain_id, control.claimed, *control.stop.borrow(), *control.finished.borrow(), control.retry_message), before);
        assert_eq!(child.manual_retries.load(Ordering::Acquire), manual_before);
        assert_eq!(child.generation.load(Ordering::Acquire), 41);
    }
}

/// One full child turn generation: bounded pre-side-effect admission
/// retries, stream consumption, then awaiting the owned turn's real cleanup.
async fn generation(state: AppState, children: Children, events: event_bus::EventBus, child: Arc<Child>, prompt: String, replay: bool) -> Result<(), ApiFailure> {
    let mut stopped = child.stop_receiver();
    let mut attempt = 0u32;
    let mut provider_retries = 0u32;
    let mut user_message_id = None;
    let generation = child.generation.load(Ordering::Acquire).max(1);
    let body = CreateTurnBody {
        text:prompt, model:child.model.clone(), reasoning_effort:"high".into(), workdir:Some(child.workdir.clone()),
        agent_prompt:Some(child.agent_prompt.clone()), read_only:child.read_only,
        replay_requires_no_tools:replay,
    };
    loop {
        // Cancellation must not wait for admission or provider headers. The
        // admission future owns the lane permit until the response headers
        // arrive, so dropping it here releases the permit before any cleanup
        // wait instead of stalling until the provider times out.
        if *stopped.borrow() { return Err(cancelled_generation(&child, generation, None).await); }
        let dispatch_stop = stopped.clone();
        let admitted = tokio::select! {
            biased;
            _ = stopped.changed() => None,
            admitted = if let Some(message_id) = user_message_id {
                super::create_turn_stream_retry(state.clone(), child.id, events.clone(), children.clone(), body.clone(), message_id, generation, dispatch_stop)
            } else {
                super::create_turn_stream_initial(state.clone(), child.id, events.clone(), children.clone(), body.clone(), dispatch_stop)
            } => Some(admitted),
        };
        let Some(result) = admitted else { return Err(cancelled_generation(&child, generation, None).await); };
        match result {
            Ok((response, message_id, owner)) => {
                user_message_id.get_or_insert(message_id);
                if user_message_id == Some(message_id) {
                    let _ = children.record_retry_message(child.id, generation, message_id);
                }
                if let Ok(mut status) = child.status.lock() { status.state = "running"; }
                // The HTTP interrupt route cancels the owned RunOwner directly;
                // retain that identity for the whole response-consumption
                // window so an owner cancellation cannot be mistaken for a
                // retryable bad-gateway stream ending.
                let mut owner_cancelled = owner.explicit_cancellation_receiver();
                let consumed = async {
                    let mut stream = response.into_body().into_data_stream();
                    let mut settled = false;
                    let mut last_tool_failed = false;
                    while let Some(bytes) = stream.next().await {
                        let bytes = bytes.map_err(|_| ApiFailure::bad_gateway("child response interrupted"))?;
                        for line in bytes.split(|byte| *byte == b'\n').filter(|line| !line.is_empty()) {
                            let event: Value = serde_json::from_slice(line).map_err(|_| ApiFailure::bad_gateway("invalid child event"))?;
                            match event["type"].as_str() {
                                Some("tool_call") => { child.tool_seen.store(true, Ordering::Release); }
                                Some("tool_output") => {
                                    last_tool_failed = if event["name"] == "write" { event["output"] != "write success" } else { event["success"] == false || event["is_error"] == true };
                                }
                                Some("assistant_delta") => {
                                    if let Some(delta) = event["delta"].as_str() {
                                        let mut status = child.status.lock().map_err(|_| ApiFailure::internal("child state unavailable"))?;
                                        status.output.push_str(delta);
                                        if status.output.len() > MAX_TAIL {
                                            let mut start = status.output.len() - MAX_TAIL;
                                            while !status.output.is_char_boundary(start) { start += 1; }
                                            status.output.drain(..start);
                                        }
                                    }
                                }
                                Some("assistant_message") => {
                                    if last_tool_failed { return Err(ApiFailure::bad_gateway("child's last tool failed or was rejected; inspect its history")); }
                                    if event["stop_reason"].as_str().is_some_and(|reason| reason != "completed") { return Err(ApiFailure::bad_gateway("child reached its execution budget")); }
                                    settled = true;
                                }
                                Some("error") => {
                                    let code = event["code"].as_str().unwrap_or("unknown");
                                    let message = event["message"].as_str().unwrap_or("child turn failed; inspect its history");
                                    return Err(ApiFailure { status: StatusCode::BAD_GATEWAY, code: provider_event_code(code), message: message.chars().filter(|ch| !ch.is_control()).take(1024).collect() });
                                }
                                _ => {}
                            }
                        }
                    }
                    if settled { Ok(()) } else { Err(ApiFailure::bad_gateway("child ended without a completed turn")) }
                };
                let result = if *stopped.borrow() || owner.is_explicitly_cancelled() { Err(ApiFailure { status: StatusCode::BAD_GATEWAY, code: "cancelled", message: "child cancelled".into() }) } else {
                    tokio::select! {
                        biased;
                        _=stopped.changed()=>Err(ApiFailure { status: StatusCode::BAD_GATEWAY, code: "cancelled", message: "child cancelled".into() }),
                        _=owner_cancelled.changed()=>Err(ApiFailure { status: StatusCode::BAD_GATEWAY, code: "cancelled", message: "child cancelled".into() }),
                        result=consumed=>result
                    }
                };
                let owner_was_cancelled = owner.is_explicitly_cancelled();
                if owner_was_cancelled { child.mark_explicit_owner_cancelled(); }
                if *stopped.borrow() || owner_was_cancelled { owner.cancel_explicit(); owner.wait_finished().await; }
                else { owner.wait_finished().await; }
                let owner_was_cancelled = owner.is_explicitly_cancelled();
                if owner_was_cancelled { child.mark_explicit_owner_cancelled(); }
                if child.is_stopped() {
                    return Err(cancelled_generation(&child, generation, Some(&owner)).await);
                }
                if owner_was_cancelled {
                    return Err(cancelled_generation(&child, generation, Some(&owner)).await);
                }
                match result {
                    Ok(()) => return Ok(()),
                    Err(error) if !child.is_stopped() && provider_retries < MAX_PROVIDER_RETRIES
                        && is_transient_provider_code(error.code)
                        && state.sessions.has_tool_admissions(child.id).await.map_err(ApiFailure::internal)? == false => {
                        provider_retries += 1;
                        child.auto_retries.store(provider_retries, Ordering::Release);
                        // The generation remains non-terminal while its owned
                        // provider retry is in bounded backoff. Keep the
                        // externally observable lifecycle in the existing
                        // queued/running vocabulary: publishing `error` here
                        // would make cancellation race with a terminal
                        // snapshot and hide the pending retry.
                        if let Ok(mut status) = child.status.lock() { status.state = "running"; }
                        tokio::select! {
                            biased;
                            _ = stopped.changed() => return Err(cancelled_generation(&child, generation, Some(&owner)).await),
                            _ = tokio::time::sleep(std::time::Duration::from_millis(ADMISSION_BACKOFF_BASE_MS << (provider_retries - 1).min(4))) => {}
                        }
                        continue;
                    }
                    Err(error) => return Err(if child.is_stopped() { cancelled_generation(&child, generation, Some(&owner)).await } else { error }),
                }
            }
            // Only the pre-side-effect capacity refusal retries: it happens
            // before owner acquisition, before any durable prompt and before
            // any provider request. Anything else could replay a durable
            // effect, so it is terminal; the explicit manual retry keeps its
            // own safety gates.
            Err(error) if attempt < MAX_ADMISSION_ATTEMPTS
                && error.status == StatusCode::TOO_MANY_REQUESTS
                && error.message == "too many active turns" => {
                attempt += 1;
                child.auto_retries.store(attempt, Ordering::Release);
                tokio::select! {
                    biased;
                    _ = stopped.changed() => return Err(cancelled_generation(&child, generation, None).await),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(
                        ADMISSION_BACKOFF_BASE_MS << (attempt - 1).min(4))) => {}
                }
            }
            Err(error) => {
                if error.code == "cancelled" {
                    return Err(cancelled_generation(&child, generation, None).await);
                }
                return Err(if attempt > 0 {
                    ApiFailure::bad_gateway(format!(
                        "child turn not admitted after {attempt} admission retries: {}", error.message))
                } else { error });
            }
        }
    }
}

fn is_transient_provider_code(code: &str) -> bool {
    matches!(code, "bad_gateway" | "service_unavailable" | "timeout" | "rate_limit" | "rate_limited")
}

fn provider_event_code(code: &str) -> &'static str {
    match code {
        "bad_gateway" => "bad_gateway",
        "service_unavailable" => "service_unavailable",
        "timeout" => "timeout",
        "rate_limit" | "rate_limited" => "rate_limited",
        _ => "provider_stream_failed",
    }
}

/// Admission was cancelled after the dropped future released its permit;
/// waiting cleanup is therefore bounded and must not recreate a new turn.
async fn cancelled_generation(child: &Child, generation: u32, owner: Option<&Arc<RunOwner>>) -> ApiFailure {
    child.mark_explicit_owner_cancelled_for(generation);
    if let Some(owner) = owner {
        owner.cancel_explicit();
        owner.wait_finished().await;
    }
    ApiFailure { status: StatusCode::BAD_GATEWAY, code: "cancelled", message: "child cancelled".into() }
}

pub(crate) async fn execute(state: AppState, children: Children, events: event_bus::EventBus, parent: &Arc<RunOwner>, broker: &PermissionBroker, message_id: MessageId, call: &RequestedCall, model: &str, workdir: &std::path::Path) -> NativeToolOutcome {
    let failed = |output: String| NativeToolOutcome { output, success:false, stop_after_outputs:false };
    match execute_inner(state, children, events, parent, broker, message_id, call, model, workdir).await {
        Ok(value) => NativeToolOutcome { output:value.to_string(), success:matches!(value["state"].as_str(), Some("queued" | "running" | "completed")), stop_after_outputs:false },
        Err(error) => {
            let stop = matches!(error.message.as_str(), "task rejected" | "task cancelled" | "task approval invalid" | "task approval proof missing");
            NativeToolOutcome { stop_after_outputs:stop, ..failed(format!("error: {}", error.message)) }
        }
    }
}

async fn execute_inner(state: AppState, children: Children, events: event_bus::EventBus, parent: &Arc<RunOwner>, broker: &PermissionBroker, message_id: MessageId, call: &RequestedCall, model: &str, workdir: &std::path::Path) -> Result<Value, ApiFailure> {
    if children.is_child(parent.session_id) { return Err(ApiFailure::bad_request("subagent depth limit reached")); }
    if call.arguments.len() > 40 * 1024 { return Err(ApiFailure::bad_request("task arguments exceed limit")); }
    let input: Value = serde_json::from_str(&call.arguments).map_err(|_| ApiFailure::bad_request("invalid task arguments"))?;
    let object = input.as_object().ok_or_else(|| ApiFailure::bad_request("task arguments must be an object"))?;
    if object.keys().any(|key| !matches!(key.as_str(), "description" | "prompt" | "subagent_type" | "task_id" | "background")) {
        return Err(ApiFailure::bad_request("unsupported task argument"));
    }
    let text = |field: &str, max: usize| -> Result<String, ApiFailure> {
        input[field].as_str().filter(|s| !s.trim().is_empty() && s.len() <= max)
            .map(str::to_owned).ok_or_else(|| ApiFailure::bad_request(format!("invalid task {field}")))
    };
    let description = text("description", 160)?;
    let prompt = text("prompt", 32768)?;
    let agent = text("subagent_type", 128)?;
    if agent.chars().any(|ch| ch.is_control() || ch.is_whitespace()) {
        return Err(ApiFailure::bad_request("invalid subagent type"));
    }
    if input.get("task_id").is_some_and(|value| !value.is_string()) {
        return Err(ApiFailure::bad_request("task_id must be a child session id"));
    }
    let background = match input.get("background") {
        None => false,
        Some(value) => value.as_bool().ok_or_else(|| ApiFailure::bad_request("background must be boolean"))?,
    };
    let root = workdir.to_owned();
    let agent_name = agent.clone();
    let configuration = tokio::task::spawn_blocking(move || configured_commands::load_agent(&root, &agent_name))
        .await.map_err(|_| ApiFailure::internal("agent configuration worker failed"))?.map_err(ApiFailure::bad_request)?;
    let config = configuration.as_ref();
    if config.and_then(|value| value["prompt"].as_str()).is_some_and(|prompt| prompt.len() > 32768)
        || config.and_then(|value| value["model"].as_str()).is_some_and(|model| model.len() > 512) {
        return Err(ApiFailure::bad_request("subagent configuration exceeds limits"));
    }
    // Configured agents resolve exactly as written; built-in roles also
    // resolve common standard spellings to a canonical bounded role.
    let role = if config.is_none() {
        match builtin_agents::resolve(&agent) {
            Some(role) => Some(role),
            None => {
                let catalogue = builtin_agents::ROLES.iter().map(|role| role.name).collect::<Vec<_>>().join(", ");
                return Err(ApiFailure::bad_request(format!("unknown subagent '{agent}'; available built-in agents: {catalogue}, plus configured agents")));
            }
        }
    } else { builtin_agents::resolve(&agent) };
    let canonical = role.map_or_else(|| agent.clone(), |role| role.name.to_owned());
    if config.is_some_and(|v| v.get("disable").or(v.get("disabled")).and_then(Value::as_bool) == Some(true) || v.get("permissions").is_some() || v.get("permission").is_some()) {
        return Err(ApiFailure::bad_request("disabled agents and agent-specific permission overrides are unavailable"));
    }
    // A configured model must already be provider-qualified: this check runs
    // before any child session creation, so a bad model can never leave an
    // orphan child session behind.
    let child_model = match config.and_then(|v| v["model"].as_str()).filter(|model| !model.is_empty()) {
        Some(configured) => {
            if configured.len() > 512 || configured.split_once('/').is_none_or(|(provider, id)| provider.is_empty() || id.is_empty()) {
                return Err(ApiFailure::bad_request("subagent model must use provider/model format"));
            }
            configured.to_owned()
        }
        None => model.to_owned(),
    };
    // The task admission itself is brokered separately from each child effect.
    let intent = OperationIntent::Tool { name:"task".into(), description:call.arguments.clone() };
    if crate::permission_rules::denied("task", &parent.session_id.to_string(), &intent, broker) { return Err(ApiFailure::bad_request("task rejected")); }
    match broker.authorize(&intent) {
        Decision::Allow => {}
        Decision::Deny { reason } => return Err(ApiFailure::bad_request(reason)),
        Decision::RequireHuman { .. } => {
            let request = PermissionRequest { id:format!("per_{}",opencode_rk_contracts::ApprovalId::new()), session_id:parent.session_id.to_string(), action:"task".into(), resources:vec![serde_json::to_string(&intent).map_err(ApiFailure::internal)?], source:PermissionSource { kind:"tool", message_id:message_id.to_string(), call_id:call.call_id.clone() } };
            let request_id = request.id.clone();
            let receiver = parent.install_pending(request, intent.clone(), broker).map_err(|_| ApiFailure::bad_request("task approval unavailable"))?;
            let _guard = PendingPermissionGuard::new(parent.clone(), request_id);
            let mut cancelled = parent.cancellation_receiver();
            let approval = tokio::select! { biased; _=cancelled.changed()=>return Err(ApiFailure::bad_request("task cancelled")), reply=receiver=>reply.map_err(|_| ApiFailure::bad_request("task cancelled"))? };
            match approval.reply {
                PermissionReply::Reject => return Err(ApiFailure::bad_request("task rejected")),
                PermissionReply::Cancelled => return Err(ApiFailure::bad_request("task cancelled")),
                PermissionReply::Once | PermissionReply::Always => {}
            }
            let proof = approval.capability.ok_or_else(|| ApiFailure::bad_request("task approval proof missing"))?;
            if !matches!(broker.authorize_with_human_approval(&intent, proof, &parent.session_id.to_string(), parent.run_id, &call.call_id), Decision::Allow) { return Err(ApiFailure::bad_request("task approval invalid")); }
        }
    }
    if parent.is_cancelled() { return Err(ApiFailure::bad_request("task cancelled")); }
    let mut foreground_claim = None;
    let child = if let Some(id) = input.get("task_id").and_then(Value::as_str) {
        let child = children.child(parent.session_id, id)?;
        if child.agent != canonical { return Err(ApiFailure::bad_request("task agent does not match child")); }
        foreground_claim = Some(restart(state.clone(), children.clone(), events.clone(), child.clone(), prompt, false, false)?);
        child
    } else {
        // Serialize registration with a short reservation before any child turn.
        if children.0.lock().map_err(|_| ApiFailure::internal("child registry unavailable"))?.len() >= MAX_CHILDREN { return Err(ApiFailure::too_many_requests("child registry is full; durable child histories are preserved")); }
        // A new child inherits the parent's session-effective MCP selection.
        // Persist it before registering or spawning the child so discovery on
        // its first provider request cannot restore parent-excluded tools.
        let mcp_exclusions = state.sessions.mcp_exclusions(parent.session_id).await.map_err(ApiFailure::internal)?;
        let summary = state.sessions.create_in_directory(format!("{description} (@{canonical} subagent)"), Some(workdir.to_owned())).await.map_err(ApiFailure::internal)?;
        for server in mcp_exclusions {
            state.sessions.set_mcp_included(summary.id, server, false).await.map_err(ApiFailure::internal)?;
        }
        if parent.is_cancelled() { return Err(ApiFailure::bad_request("task cancelled")); }
        // A configured prompt/model override wins, but a built-in role never
        // loses its enforced read-only policy through configuration.
        let configured_prompt = config.and_then(|v| v["prompt"].as_str()).map(str::to_owned);
        let base_prompt = configured_prompt.unwrap_or_else(|| role.map_or_else(|| "Complete the delegated task, use tools as needed, and report a concise factual result.".to_owned(), |role| role.prompt.to_owned()));
        let configured_read_only = config.and_then(|v| v.get("read_only").and_then(Value::as_bool)).unwrap_or(false);
        let (stop, _) = watch::channel(false);
        let (finished, _) = watch::channel(true);
        let child = Arc::new(Child { parent:parent.session_id, id:summary.id, description, agent:canonical.clone(),
            model:child_model, prompt:prompt.clone(), workdir:workdir.to_string_lossy().into_owned(),
            agent_prompt:format!("Agent: {canonical}\n{base_prompt}\nYou cannot delegate further tasks."),
            read_only:configured_read_only || role.is_some_and(|role| role.read_only), status:Mutex::new(Status { state:"queued", output:String::new(), error:None }), control:Mutex::new(GenerationControl { generation:0, chain_id:0, stop, finished, claimed:false, retry_message:None, explicit_cancelled:false }), tool_seen:AtomicBool::new(false),
            generation:AtomicU32::new(0), auto_retries:AtomicU32::new(0), manual_retries:AtomicU32::new(0), followups:Mutex::new(VecDeque::new()) });
        let claimed = child.claim_generation_receipt(false)?;
        {
            let mut rows = children.0.lock().map_err(|_| ApiFailure::internal("child registry unavailable"))?;
            if rows.len() >= MAX_CHILDREN { return Err(ApiFailure::too_many_requests("child registry is full; created child session preserved")); }
            rows.insert(child.id.to_string(), child.clone());
        }
        foreground_claim = Some(claimed.foreground());
        spawn_claimed(state, children, events, child.clone(), prompt, false, claimed);
        child
    };
    if background { return Ok(snapshot(&child)); }
    let foreground = foreground_claim.expect("foreground claim retained before spawn");
    let foreground_chain = foreground.chain_id;
    let mut finished = foreground.finished;
    let mut cancelled = parent.cancellation_receiver();
    loop {
        if *finished.borrow_and_update() { break; }
        if parent.is_cancelled() {
            child.cancel_chain(foreground_chain);
            while !*finished.borrow_and_update() { if finished.changed().await.is_err() { break; } }
            return Err(ApiFailure::bad_request("foreground child cancelled with parent"));
        }
        tokio::select! {
            biased;
            _=cancelled.changed()=>{
                child.cancel_chain(foreground_chain);
                while !*finished.borrow_and_update() { if finished.changed().await.is_err() { break; } }
                return Err(ApiFailure::bad_request("foreground child cancelled with parent"));
            }
            result=finished.changed()=>{ if result.is_err() { break; } }
        }
    }
    Ok(snapshot(&child))
}
