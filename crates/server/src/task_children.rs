//! Router-owned, bounded child turns. All child effects use the ordinary turn
//! engine and child-bound permission broker; this module never executes a tool.
use super::*;
use std::{collections::BTreeMap, sync::{Mutex, atomic::{AtomicBool, Ordering}}};
use tokio::sync::watch;

const MAX_CHILDREN: usize = 128;
const MAX_TAIL: usize = 8192;

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
    match child.status.lock() {
        Ok(status) => json!({"session_id":child.id,"parent_id":child.parent,"description":child.description,
            "agent":child.agent,"model":child.model,"state":if status.state == "queued" && run_owner::active(child.id).is_some() { "running" } else { status.state },"output_tail":status.output,
            "error":status.error,"has_tool_calls":child.tool_seen.load(Ordering::Acquire),
            "awaiting_permission":run_owner::active(child.id).is_some_and(|owner| owner.pending().is_some())}),
        Err(_) => json!({"session_id":child.id,"state":"unknown","error":"child state unavailable"}),
    }
}

pub(crate) fn schema() -> ResponsesTool {
    ResponsesTool::function("task", "Delegate a bounded child session. Use explore for read-only investigation or general for a full tool loop. Background tasks can be inspected and controlled through the Subagents panel.",
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
    restart(state, children, events, child.clone(), prompt, true)?;
    Ok(Json(snapshot(&child)))
}

pub(crate) async fn followup(State(state): State<AppState>, Path((parent, id)): Path<(String, String)>, Extension(children): Extension<Children>, Extension(events): Extension<event_bus::EventBus>, Json(body): Json<Followup>) -> Result<Json<Value>, ApiFailure> {
    if body.text.trim().is_empty() || body.text.len() > 32768 { return Err(ApiFailure::bad_request("follow-up must contain 1..32768 bytes")); }
    let child = children.child(parse_session_id(&parent)?, &id)?;
    restart(state, children, events, child.clone(), body.text, false)?;
    Ok(Json(snapshot(&child)))
}

fn restart(state: AppState, children: Children, events: event_bus::EventBus, child: Arc<Child>, prompt: String, replay: bool) -> Result<(), ApiFailure> {
    let mut status = child.status.lock().map_err(|_| ApiFailure::internal("child state unavailable"))?;
    if status.state == "running" || !*child.finished.borrow() {
        return Err(ApiFailure::bad_request("child task is still running"));
    }
    if status.state == "completed" && prompt == child.prompt {
        return Err(ApiFailure::bad_request("completed tasks cannot be retried; send an explicit follow-up"));
    }
    *status = Status { state:"queued", output:String::new(), error:None };
    child.stop.send_replace(false);
    child.finished.send_replace(false);
    drop(status);
    spawn(state, children, events, child, prompt, replay);
    Ok(())
}

fn spawn(state: AppState, children: Children, events: event_bus::EventBus, child: Arc<Child>, prompt: String, replay: bool) {
    tokio::spawn(async move {
        let mut stopped = child.stop.subscribe();
        let result = async {
            let body = CreateTurnBody {
                text:prompt, model:child.model.clone(), reasoning_effort:"high".into(), workdir:Some(child.workdir.clone()),
                agent_prompt:Some(child.agent_prompt.clone()), read_only:child.read_only,
                replay_requires_no_tools:replay,
            };
            let waiting = std::time::Instant::now();
            let response = loop {
                match super::create_turn_stream(State(state.clone()), Path(child.id.to_string()), Extension(events.clone()), Extension(children.clone()), Json(body.clone())).await {
                    Ok(response) => break response,
                    // This exact refusal occurs before owner acquisition and
                    // before any durable prompt or provider request. Waiting
                    // for admission is not replaying a dispatched turn.
                    Err(error) if error.status == StatusCode::TOO_MANY_REQUESTS
                        && error.message == "too many active turns"
                        && waiting.elapsed() < std::time::Duration::from_secs(60) => {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    Err(error) => return Err(error),
                }
            };
            if let Ok(mut status) = child.status.lock() { status.state = "running"; }
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
            tokio::select! { biased; _=stopped.changed()=>Err(ApiFailure::bad_gateway("child cancelled")), result=result=>result }
        };
        // Dropping an HTTP receiver never drops the daemon's producer. Explicit
        // cancellation must await the owned turn's real cleanup before terminal state.
        if child.stop.borrow().to_owned() { run_owner::cancel_and_wait(child.id).await; }
        else if let Some(owner) = run_owner::active(child.id) { owner.wait_finished().await; }
        if let Ok(mut status) = child.status.lock() {
            match result {
                Ok(()) => status.state = "completed",
                Err(error) => {
                    status.state = if *child.stop.borrow() { "cancelled" } else { "error" };
                    status.error = Some(error.message);
                }
            }
        }
        child.finished.send_replace(true);
    });
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
    if !matches!(agent.as_str(), "general" | "explore" | "build" | "plan") && config.is_none() { return Err(ApiFailure::bad_request("subagent type not found")); }
    if config.is_some_and(|v| v.get("disable").or(v.get("disabled")).and_then(Value::as_bool) == Some(true) || v.get("permissions").is_some() || v.get("permission").is_some()) {
        return Err(ApiFailure::bad_request("disabled agents and agent-specific permission overrides are unavailable"));
    }
    // The task admission itself is brokered separately from each child effect.
    let intent = OperationIntent::Tool { name:"task".into(), description:call.arguments.clone() };
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
        if child.agent != agent { return Err(ApiFailure::bad_request("task agent does not match child")); }
        restart(state.clone(), children.clone(), events.clone(), child.clone(), prompt, false)?;
        child
    } else {
        // Serialize registration with a short reservation before any child turn.
        if children.0.lock().map_err(|_| ApiFailure::internal("child registry unavailable"))?.len() >= MAX_CHILDREN { return Err(ApiFailure::too_many_requests("child registry is full; durable child histories are preserved")); }
        let summary = state.sessions.create_in_directory(format!("{description} (@{agent} subagent)"), Some(workdir.to_owned())).await.map_err(ApiFailure::internal)?;
        let (stop, _) = watch::channel(false);
        let (finished, _) = watch::channel(false);
        let child = Arc::new(Child { parent:parent.session_id, id:summary.id, description, agent:agent.clone(),
            model:config.and_then(|v| v["model"].as_str()).unwrap_or(model).to_owned(), prompt:prompt.clone(), workdir:workdir.to_string_lossy().into_owned(),
            agent_prompt:format!("Agent: {agent}\n{}", config.and_then(|v| v["prompt"].as_str()).unwrap_or("Complete the delegated task, use tools as needed, and report a concise factual result. You cannot delegate further.")),
            read_only:matches!(agent.as_str(), "explore" | "plan"), status:Mutex::new(Status { state:"queued", output:String::new(), error:None }), stop, finished, tool_seen:AtomicBool::new(false) });
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
