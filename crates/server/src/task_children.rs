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

#[derive(Clone, Default)]
pub(crate) struct Children(Arc<Mutex<BTreeMap<String, Arc<Child>>>>);

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
    stop: watch::Sender<bool>,
    finished: watch::Sender<bool>,
    tool_seen: AtomicBool,
    generation: AtomicU32,
    auto_retries: AtomicU32,
    manual_retries: AtomicU32,
    followups: Mutex<VecDeque<String>>,
}
struct Status { state: &'static str, output: String, error: Option<String> }

impl Children {
    pub fn is_child(&self, id: SessionId) -> bool {
        self.0.lock().map_or(true, |rows| rows.contains_key(&id.to_string()))
    }
    fn child(&self, parent: SessionId, id: &str) -> Result<Arc<Child>, ApiFailure> {
        self.0.lock().map_err(|_| ApiFailure::internal("child registry unavailable"))?
            .get(id).filter(|entry| entry.parent == parent).cloned()
            .ok_or_else(|| ApiFailure::not_found("child task not found"))
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
    child.stop.send_replace(true);
    if let Some(owner) = run_owner::active(child.id) { owner.cancel(); }
    let mut finished = child.finished.subscribe();
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
    restart(state, children, events, child.clone(), prompt, true, true)?;
    Ok(Json(snapshot(&child)))
}

pub(crate) async fn followup(State(state): State<AppState>, Path((parent, id)): Path<(String, String)>, Extension(children): Extension<Children>, Extension(events): Extension<event_bus::EventBus>, Json(body): Json<Followup>) -> Result<Json<Value>, ApiFailure> {
    if body.text.trim().is_empty() || body.text.len() > 32768 { return Err(ApiFailure::bad_request("follow-up must contain 1..32768 bytes")); }
    let child = children.child(parse_session_id(&parent)?, &id)?;
    let active = {
        let status = child.status.lock().map_err(|_| ApiFailure::internal("child state unavailable"))?;
        status.state == "running" || status.state == "queued" || !*child.finished.borrow()
    };
    // A running or scheduled child accepts bounded follow-ups that run as a
    // fresh generation once the current one settles.
    if active {
        let mut queue = child.followups.lock().map_err(|_| ApiFailure::internal("child state unavailable"))?;
        if queue.len() >= MAX_FOLLOWUPS {
            return Err(ApiFailure::too_many_requests(format!("child follow-up queue is full (max {MAX_FOLLOWUPS})")));
        }
        queue.push_back(body.text);
        drop(queue);
        return Ok(Json(snapshot(&child)));
    }
    restart(state, children, events, child.clone(), body.text, false, false)?;
    Ok(Json(snapshot(&child)))
}

fn restart(state: AppState, children: Children, events: event_bus::EventBus, child: Arc<Child>, prompt: String, replay: bool, manual_retry: bool) -> Result<(), ApiFailure> {
    let mut status = child.status.lock().map_err(|_| ApiFailure::internal("child state unavailable"))?;
    if status.state == "running" || !*child.finished.borrow() {
        return Err(ApiFailure::bad_request("child task is still running"));
    }
    if status.state == "completed" && prompt == child.prompt {
        return Err(ApiFailure::bad_request("completed tasks cannot be retried; send an explicit follow-up"));
    }
    if manual_retry && child.manual_retries.load(Ordering::Acquire) >= MAX_MANUAL_RETRIES {
        return Err(ApiFailure::too_many_requests(MANUAL_RETRY_EXHAUSTION));
    }
    *status = Status { state:"queued", output:String::new(), error:None };
    if manual_retry {
        child.manual_retries.fetch_add(1, Ordering::AcqRel);
    }
    child.stop.send_replace(false);
    child.finished.send_replace(false);
    drop(status);
    spawn(state, children, events, child, prompt, replay);
    Ok(())
}

fn spawn(state: AppState, children: Children, events: event_bus::EventBus, child: Arc<Child>, prompt: String, replay: bool) {
    tokio::spawn(async move {
        child.generation.fetch_add(1, Ordering::AcqRel);
        child.auto_retries.store(0, Ordering::Release);
        let mut prompt = prompt;
        let mut replay = replay;
        // One generation per loop turn: bounded admission retries, a terminal
        // state, then at most one bounded queued follow-up continues as its
        // own fresh generation. Cancellation preserves the remaining queue.
        loop {
            child.finished.send_replace(false);
            let result = generation(state.clone(), children.clone(), events.clone(), child.clone(), prompt.clone(), replay).await;
            let cancelled = *child.stop.borrow();
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
            let next = match child.followups.lock() {
                Ok(mut queue) if !*child.stop.borrow() => queue.pop_front(),
                _ => None,
            };
            match next {
                Some(text) => {
                    prompt = text;
                    replay = false;
                    child.generation.fetch_add(1, Ordering::AcqRel);
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
        child.finished.send_replace(true);
    });
}

/// One full child turn generation: bounded pre-side-effect admission
/// retries, stream consumption, then awaiting the owned turn's real cleanup.
async fn generation(state: AppState, children: Children, events: event_bus::EventBus, child: Arc<Child>, prompt: String, replay: bool) -> Result<(), ApiFailure> {
    let mut stopped = child.stop.subscribe();
    let mut attempt = 0u32;
    let body = CreateTurnBody {
        text:prompt, model:child.model.clone(), reasoning_effort:"high".into(), workdir:Some(child.workdir.clone()),
        agent_prompt:Some(child.agent_prompt.clone()), read_only:child.read_only,
        replay_requires_no_tools:replay,
    };
    let response = loop {
        // Cancellation must not wait for admission or provider headers. The
        // admission future owns the lane permit until the response headers
        // arrive, so dropping it here releases the permit before any cleanup
        // wait instead of stalling until the provider times out.
        if *stopped.borrow() { return Err(cancelled_generation(&child).await); }
        let admitted = tokio::select! {
            biased;
            _ = stopped.changed() => None,
            admitted = super::create_turn_stream(State(state.clone()), Path(child.id.to_string()), Extension(events.clone()), Extension(children.clone()), Json(body.clone())) => Some(admitted),
        };
        let Some(result) = admitted else { return Err(cancelled_generation(&child).await); };
        match result {
            Ok(response) => break response,
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
                    _ = stopped.changed() => return Err(cancelled_generation(&child).await),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(
                        ADMISSION_BACKOFF_BASE_MS << (attempt - 1).min(4))) => {}
                }
            }
            Err(error) => {
                return Err(if attempt > 0 {
                    ApiFailure::bad_gateway(format!(
                        "child turn not admitted after {attempt} admission retries: {}", error.message))
                } else { error });
            }
        }
    };
    if let Ok(mut status) = child.status.lock() { status.state = "running"; }
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
                        // Native write retains its legacy fixed output
                        // vocabulary; other tools carry an explicit verdict.
                        last_tool_failed = if event["name"] == "write" {
                            event["output"] != "write success"
                        } else { event["success"] == false || event["is_error"] == true };
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
                        if event["stop_reason"].as_str().is_some_and(|reason| reason != "completed") {
                            return Err(ApiFailure::bad_gateway("child reached its execution budget"));
                        }
                        settled = true;
                    }
                    Some("error") => return Err(ApiFailure::bad_gateway("child turn failed; inspect its history")),
                    _ => {}
                }
            }
        }
        if settled { Ok(()) } else { Err(ApiFailure::bad_gateway("child ended without a completed turn")) }
    };
    let result = if *stopped.borrow() { Err(ApiFailure::bad_gateway("child cancelled")) } else {
        tokio::select! { biased; _=stopped.changed()=>Err(ApiFailure::bad_gateway("child cancelled")), result=consumed=>result }
    };
    // Dropping an HTTP receiver never drops the daemon's producer. Explicit
    // cancellation must await the owned turn's real cleanup before terminal state.
    if *child.stop.borrow() { run_owner::cancel_and_wait(child.id).await; }
    else if let Some(owner) = run_owner::active(child.id) { owner.wait_finished().await; }
    result
}

/// Admission was cancelled after the dropped future released its permit;
/// waiting cleanup is therefore bounded and must not recreate a new turn.
async fn cancelled_generation(child: &Child) -> ApiFailure {
    run_owner::cancel_and_wait(child.id).await;
    ApiFailure::bad_gateway("child cancelled")
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
    let child = if let Some(id) = input.get("task_id").and_then(Value::as_str) {
        let child = children.child(parent.session_id, id)?;
        if child.agent != canonical { return Err(ApiFailure::bad_request("task agent does not match child")); }
        restart(state.clone(), children.clone(), events.clone(), child.clone(), prompt, false, false)?;
        child
    } else {
        // Serialize registration with a short reservation before any child turn.
        if children.0.lock().map_err(|_| ApiFailure::internal("child registry unavailable"))?.len() >= MAX_CHILDREN { return Err(ApiFailure::too_many_requests("child registry is full; durable child histories are preserved")); }
        let summary = state.sessions.create_in_directory(format!("{description} (@{canonical} subagent)"), Some(workdir.to_owned())).await.map_err(ApiFailure::internal)?;
        let (stop, _) = watch::channel(false);
        let (finished, _) = watch::channel(false);
        // A configured prompt/model override wins, but a built-in role never
        // loses its enforced read-only policy through configuration.
        let configured_prompt = config.and_then(|v| v["prompt"].as_str()).map(str::to_owned);
        let base_prompt = configured_prompt.unwrap_or_else(|| role.map_or_else(|| "Complete the delegated task, use tools as needed, and report a concise factual result.".to_owned(), |role| role.prompt.to_owned()));
        let configured_read_only = config.and_then(|v| v.get("read_only").and_then(Value::as_bool)).unwrap_or(false);
        let child = Arc::new(Child { parent:parent.session_id, id:summary.id, description, agent:canonical.clone(),
            model:child_model, prompt:prompt.clone(), workdir:workdir.to_string_lossy().into_owned(),
            agent_prompt:format!("Agent: {canonical}\n{base_prompt}\nYou cannot delegate further tasks."),
            read_only:configured_read_only || role.is_some_and(|role| role.read_only), status:Mutex::new(Status { state:"queued", output:String::new(), error:None }), stop, finished, tool_seen:AtomicBool::new(false),
            generation:AtomicU32::new(0), auto_retries:AtomicU32::new(0), manual_retries:AtomicU32::new(0), followups:Mutex::new(VecDeque::new()) });
        {
            let mut rows = children.0.lock().map_err(|_| ApiFailure::internal("child registry unavailable"))?;
            if rows.len() >= MAX_CHILDREN { return Err(ApiFailure::too_many_requests("child registry is full; created child session preserved")); }
            rows.insert(child.id.to_string(), child.clone());
        }
        spawn(state, children, events, child.clone(), prompt, false);
        child
    };
    if background { return Ok(snapshot(&child)); }
    let mut finished = child.finished.subscribe();
    let mut cancelled = parent.cancellation_receiver();
    loop {
        if *finished.borrow_and_update() { break; }
        if parent.is_cancelled() {
            child.stop.send_replace(true);
            run_owner::cancel_and_wait(child.id).await;
            while !*finished.borrow_and_update() { if finished.changed().await.is_err() { break; } }
            return Err(ApiFailure::bad_request("foreground child cancelled with parent"));
        }
        tokio::select! {
            biased;
            _=cancelled.changed()=>{
                child.stop.send_replace(true);
                run_owner::cancel_and_wait(child.id).await;
                while !*finished.borrow_and_update() { if finished.changed().await.is_err() { break; } }
                return Err(ApiFailure::bad_request("foreground child cancelled with parent"));
            }
            result=finished.changed()=>{ if result.is_err() { break; } }
        }
    }
    Ok(snapshot(&child))
}
