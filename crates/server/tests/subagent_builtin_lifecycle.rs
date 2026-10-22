//! Built-in subagent roles, alias resolution, validated child models,
//! bounded admission retries for busy-turn capacity, and active-child
//! follow-ups. Serial execution assumed: turn admission uses process-global
//! permits, matching the frozen `session_child_execution` fixtures.
use std::{collections::HashSet, env, future::IntoFuture, sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}}, time::{Duration, Instant}};
use axum::{Router, Json, body::{Body, to_bytes}, extract::State, http::{Request, StatusCode, Method}, routing::post};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{Value, json};
use tower::ServiceExt;

struct Env(&'static str, Option<std::ffi::OsString>);
impl Env { fn set(name: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
    let previous = env::var_os(name); env::set_var(name, value); Self(name, previous)
} }
impl Drop for Env { fn drop(&mut self) { if let Some(v) = &self.1 { env::set_var(self.0, v); } else { env::remove_var(self.0); } } }

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Canonical built-in roles, their enforced read-only policy and one
/// distinctive phrase from each role prompt. The integration crate cannot
/// import the private registry, so parity is proved through the public
/// catalogue and the real child execution below.
const MATRIX: [(&str, bool, &str); 10] = [
    ("general", false, "Complete the delegated task end to end"),
    ("explore", true, "Investigate the codebase read-only"),
    ("research", true, "Cross-check claims"),
    ("debug", false, "Form the smallest reproduction"),
    ("planner", true, "step-by-step implementation plan"),
    ("implementer", false, "Implement exactly the specified change"),
    ("reviewer", true, "Review the specified code or diff read-only"),
    ("tester", false, "Run exactly the tests"),
    ("build", false, "Build or repair the requested change"),
    ("plan", true, "Produce a read-only plan"),
];

/// One observed matrix child provider round: identity, advertised policy and
/// the tool feedback the real turn engine returned to the provider.
struct MatrixCall {
    role: &'static str,
    model: String,
    system: String,
    tools: Vec<String>,
    feedback: String,
}

#[derive(Default)]
struct Fixture {
    /// Scheduled child holds consumed at the child's first provider round.
    child_holds: AtomicUsize,
    /// Deterministic release flags (no Notify wakeup races).
    released_child: std::sync::atomic::AtomicBool,
    released_blocker: std::sync::atomic::AtomicBool,
    child_requests: AtomicUsize,
    /// Deterministic admission/held signals for capacity tests.
    seen: Mutex<Option<tokio::sync::mpsc::UnboundedSender<&'static str>>>,
    /// Every-role matrix: project root for the in-project write attempt.
    matrix_root: Mutex<Option<std::path::PathBuf>>,
    /// Observed child identity/policy and the tool feedback it received.
    matrix_calls: Mutex<Vec<MatrixCall>>,
}
impl Fixture {
    fn signal(&self, event: &'static str) {
        if let Ok(seen) = self.seen.lock() {
            if let Some(sender) = seen.as_ref() { let _ = sender.send(event); }
        }
    }
}
async fn provider(State(fixture): State<Arc<Fixture>>, Json(body): Json<Value>) -> ([(&'static str, &'static str);1], String) {
    let items = body["input"].as_array().unwrap();
    let last_user = items.iter().rposition(|item| item["role"] == "user").unwrap();
    let user_prompt = items[last_user]["content"].to_string();
    let output_done = items[last_user+1..].iter().any(|item| item["type"] == "function_call_output");
    let matrix_role = MATRIX.iter().find(|entry| {
        let role = entry.0;
        user_prompt.contains(&format!("ROLE_MATRIX {role}")) || user_prompt.contains(&format!("CHILD_STEP_ROLE {role}"))
    }).map(|entry| entry.0);
    if let Some(role) = matrix_role {
        if user_prompt.contains("ROLE_MATRIX") {
            // Root turn: delegate exactly the requested canonical role.
            return simple(if output_done {
                json!({"type":"response.output_text.delta","delta":"matrix parent done"})
            } else {
                json!({"type":"response.output_item.done","item":{"type":"function_call","id":"fc_matrix_task","call_id":"matrix_parent_task","name":"task","arguments":json!({"description":format!("{role} matrix probe"),"prompt":format!("CHILD_STEP_ROLE {role} attempt"),"subagent_type":role,"background":true}).to_string()}})
            });
        }
        if !output_done {
            // Child round one: record what the real turn engine advertised,
            // then attempt one in-project write (denied for read-only roles).
            let tools: Vec<String> = body["tools"].as_array().map(|rows| rows.iter().filter_map(|tool| tool["name"].as_str().map(str::to_owned)).collect()).unwrap_or_default();
            let system = body["input"][0]["content"].as_str().unwrap_or_default().to_owned();
            let model = body["model"].as_str().unwrap_or_default().to_owned();
            fixture.matrix_calls.lock().unwrap().push(MatrixCall { role, model, system, tools, feedback: String::new() });
            let root = fixture.matrix_root.lock().unwrap().clone().expect("matrix root fixture");
            return simple(json!({"type":"response.output_item.done","item":{"type":"function_call","id":format!("fc_matrix_{role}_write"),"call_id":format!("matrix_{role}_write"),"name":"write","arguments":json!({"path":root.join(format!("matrix_{role}.txt")).to_string_lossy(),"content":format!("{role} wrote")}).to_string()}}));
        }
        // Child continuation: observe the tool's real result. If the denied
        // write returned as function output, request one allowed read as a
        // second provider call to prove readonly tools remain executable.
        let outputs = &items[last_user+1..];
        let feedback = outputs.iter().filter(|item| item["type"] == "function_call_output")
            .filter_map(|item| item["output"].as_str()).collect::<Vec<_>>().join("\n");
        let mut calls = fixture.matrix_calls.lock().unwrap();
        if let Some(call) = calls.iter_mut().find(|call| call.role == role) { call.feedback.push_str(&feedback); }
        let readonly = MATRIX.iter().find(|entry| entry.0 == role).is_some_and(|entry| entry.1);
        let readback_done = outputs.iter().any(|item| item["call_id"] == format!("matrix_{role}_readback"));
        if readonly && feedback.contains("not permitted by turn policy") && !readback_done {
            return simple(json!({"type":"response.output_item.done","item":{"type":"function_call","id":format!("fc_matrix_{role}_readback"),"call_id":format!("matrix_{role}_readback"),"name":"read","arguments":json!({"path":"matrix-read-probe.txt"}).to_string()}}));
        }
        return simple(json!({"type":"response.output_text.delta","delta":format!("role {role} complete")}));
    }
    if user_prompt.contains("BLOCKER_MARKER") {
        // Hold the streaming response so the turn keeps its admission permit.
        if !output_done { fixture.signal("blocker_held"); while !fixture.released_blocker.load(Ordering::SeqCst) { tokio::time::sleep(Duration::from_millis(15)).await; } }
        return simple(if output_done {
            json!({"type":"response.output_text.delta","delta":"blocker done"})
        } else {
            json!({"type":"response.output_item.done","item":{"type":"function_call","id":"fc_block","call_id":"blocker_read","name":"read","arguments":json!({"path":"probe.txt"}).to_string()}})
        });
    }
    if !user_prompt.contains("CHILD_STEP") {
        // Root turn: delegate via the task tool on the first provider round.
        let child_type = if user_prompt.contains("ALIAS_RESEARCH") { "researcher" }
            else if user_prompt.contains("CUSTOM_MODEL_DELEGATE") { "explorer" }
            else { "explore" };
        return simple(if output_done {
            json!({"type":"response.output_text.delta","delta":"parent done"})
        } else {
            json!({"type":"response.output_item.done","item":{"type":"function_call","id":"fc_task","call_id":"parent_task","name":"task","arguments":json!({"description":"delegate probe","prompt":"CHILD_STEP read probe.txt","subagent_type":child_type,"background":!user_prompt.contains("FOREGROUND_DELEGATE")}).to_string()}})
        });
    }
    // Child turn.
    fixture.child_requests.fetch_add(1, Ordering::SeqCst);
    let step_two = user_prompt.contains("CHILD_STEP_TWO");
    let event = if !output_done {
        if fixture.child_holds.load(Ordering::SeqCst) > 0 {
            fixture.child_holds.fetch_sub(1, Ordering::SeqCst);
            fixture.signal("child_held");
            while !fixture.released_child.load(Ordering::SeqCst) { tokio::time::sleep(Duration::from_millis(15)).await; }
        }
        json!({"type":"response.output_item.done","item":{"type":"function_call","id":"fc_child_read","call_id":"child_read","name":"read","arguments":json!({"path":"probe.txt"}).to_string()}})
    } else if step_two {
        json!({"type":"response.output_text.delta","delta":"step two done"})
    } else {
        json!({"type":"response.output_text.delta","delta":"step one done"})
    };
    simple(event)
}
fn simple(event: Value) -> ([(&'static str, &'static str);1], String) {
    ([("content-type", "text/event-stream")], format!("data: {event}\n\ndata: {{\"type\":\"response.completed\",\"response\":{{\"status\":\"completed\"}}}}\n\n"))
}
fn request(method: Method, uri: String, body: Value) -> Request<Body> {
    Request::builder().method(method).uri(uri).header("content-type","application/json").body(Body::from(body.to_string())).unwrap()
}
async fn value(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}
async fn poll(app: &Router, path: String, predicate: impl Fn(&Value)->bool) -> (Value, Duration) {
    let started = Instant::now();
    let mut last = Value::Null;
    let result = tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            last = value(app.clone().oneshot(request(Method::GET, path.clone(), json!({}))).await.unwrap()).await;
            if predicate(&last) { return last.clone(); }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    }).await.unwrap_or_else(|_| panic!("fixture state at {path} must become observable; last snapshot: {last}"));
    (result, started.elapsed())
}
/// Approve the parent's first task-tool permission request and drain the
/// parent turn SSE body.
async fn delegate(app: &Router, parent: &opencode_rk_contracts::SessionId, marker: &str) {
    let response = app.clone().oneshot(request(Method::POST, format!("/api/sessions/{}/turns/stream",parent), json!({"text":marker, "model":"openai/fixture"}))).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = tokio::spawn(async move { to_bytes(response.into_body(),1024*1024).await.unwrap() });
    let (permission, _) = poll(app, format!("/api/session/{}/permission",parent), |v| v["data"].as_array().is_some_and(|rows| !rows.is_empty())).await;
    let request_id = permission["data"][0]["id"].as_str().unwrap().to_owned();
    let approved = app.clone().oneshot(request(Method::POST, format!("/api/session/{}/permission/{request_id}/reply",parent), json!({"reply":"once"}))).await.unwrap();
    assert_eq!(approved.status(), StatusCode::NO_CONTENT);
    let body = tokio::time::timeout(Duration::from_secs(10),body).await.unwrap().unwrap();
    let events: Vec<Value> = body.split(|byte| *byte == b'\n').filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap()).collect();
    assert!(!events.iter().any(|event| event["type"] == "error"), "parent turn must not fail: {events:?}");
}
async fn continue_child(app: &Router, parent: &opencode_rk_contracts::SessionId, child_id: &str, text: &str) -> axum::response::Response {
    tokio::time::timeout(Duration::from_secs(3),
        app.clone().oneshot(request(Method::POST, format!("/api/sessions/{}/children/{child_id}/continue",parent), json!({"text":text}))))
        .await.expect("continue request must not deadlock before its old regression").unwrap()
}

/// A full-loop write correctly pauses for human approval under the default
/// broker policy. Approve that real child request through the parent projection.
async fn approve_child_write(app: &Router, parent: &opencode_rk_contracts::SessionId) {
    let (permissions, _) = poll(app, format!("/api/session/{parent}/permission"), |value|
        value["data"].as_array().is_some_and(|rows| rows.iter().any(|row| row["action"] == "Subagent: write"))).await;
    let request_id = permissions["data"].as_array().unwrap().iter()
        .find(|row| row["action"] == "Subagent: write").unwrap()["id"].as_str().unwrap();
    let response = app.clone().oneshot(request(Method::POST,
        format!("/api/session/{parent}/permission/{request_id}/reply"), json!({"reply":"once"}))).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT, "parent must approve the child's exact write request: {permissions}");
}

#[tokio::test(flavor="multi_thread", worker_threads=2)]
async fn builtin_catalogue_lists_all_roles_and_preserves_custom_overrides() {
    let _serial = TEST_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let _env = [Env::set("HOME",dir.path()), Env::set("XDG_CONFIG_HOME",dir.path()), Env::set("OPENCODE_CONFIG_DIR",dir.path().join("unused")), Env::set("OPENCODE_RK_HOME",dir.path()), Env::set("OPENCODE_RK_PERMISSION_RULES",dir.path().join("permission-rules.json"))];
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap()));
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });
    let response = app.clone().oneshot(request(Method::GET, "/api/commands".into(), json!({}))).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let listing = value(response).await;
    let agents = listing["agents"].as_array().cloned().unwrap();
    for expected in ["general","explore","research","debug","planner","implementer","reviewer","tester","build","plan"] {
        assert!(agents.iter().any(|a| a["name"] == expected && !a["description"].as_str().unwrap_or("").is_empty()), "missing builtin {expected}: {agents:?}");
    }
    assert!(!agents.iter().any(|a| a["name"] == "general-purpose"), "alias must not be a duplicate catalogue entry");
    for role in ["explore","research","planner","reviewer","plan"] {
        assert_eq!(agents.iter().find(|a| a["name"] == role).unwrap()["read_only"], true, "{role} must stay enforceable read-only");
    }
    for role in ["general","build","implementer","debug","tester"] {
        assert_eq!(agents.iter().find(|a| a["name"] == role).unwrap()["read_only"], false);
    }
    // A configured agent with the same name overrides the builtin entry.
    let config = dir.path().join("conf");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("opencode.json"), json!({"agents":{"reviewer":{"description":"Custom reviewer"}}}).to_string()).unwrap();
    let _override = Env::set("OPENCODE_CONFIG_DIR", config.as_os_str());
    let response = app.clone().oneshot(request(Method::GET, "/api/commands".into(), json!({}))).await.unwrap();
    let listing = value(response).await;
    let agents = listing["agents"].as_array().cloned().unwrap();
    assert_eq!(agents.iter().filter(|a| a["name"] == "reviewer").count(), 1, "no duplicate agent names");
    assert_eq!(agents.iter().find(|a| a["name"] == "reviewer").unwrap()["description"], "Custom reviewer", "configured override must win");
}

#[tokio::test(flavor="multi_thread", worker_threads=2)]
async fn alias_subagent_types_resolve_to_canonical_builtin_roles() {
    let _serial = TEST_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("probe.txt"), "fixture child content").unwrap();
    let fixture = Arc::new(Fixture::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}",listener.local_addr().unwrap());
    let mock = tokio::spawn(axum::serve(listener, Router::new().fallback(post(provider)).with_state(fixture.clone())).into_future());
    let _env = [Env::set("OPENAI_BASE_URL", base), Env::set("OPENAI_API_KEY", "disposable-fixture-key"), Env::set("HOME",dir.path()), Env::set("XDG_CONFIG_HOME",dir.path()), Env::set("OPENCODE_CONFIG_DIR",dir.path().join("global-config")), Env::set("OPENCODE_RK_HOME",dir.path()), Env::set("OPENCODE_RK_PERMISSION_RULES",dir.path().join("permission-rules.json")), Env::set("OPENCODE_RK_TURN_TOOLS","task,read")];
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap()));
    let parent = sessions.create_in_directory("parent",Some(dir.path().into())).await.unwrap();
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });
    delegate(&app, &parent.id, "ALIAS_RESEARCH delegate probe").await;
    let (listing, _) = poll(&app, format!("/api/sessions/{}/children",parent.id), |v| v["children"][0]["state"] == "completed").await;
    let child = &listing["children"][0];
    assert_eq!(child["agent"], "research", "alias 'researcher' must resolve to the canonical builtin role");
    assert_eq!(child["read_only"], true, "research role must stay enforceable read-only");
    assert_eq!(child["output_tail"], "step one done");
    assert_eq!(child["generation"], 1);
    assert_eq!(child["pending_followups"], 0);
    mock.abort();
}

#[tokio::test(flavor="multi_thread", worker_threads=2)]
async fn config_agent_bare_model_fails_before_any_child_session_exists() {
    let _serial = TEST_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".opencode")).unwrap();
    std::fs::write(dir.path().join(".opencode/opencode.json"),
        json!({"agents":{"explorer":{"model":"bare-model","prompt":"inspect"}}}).to_string()).unwrap();
    let fixture = Arc::new(Fixture::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}",listener.local_addr().unwrap());
    let mock = tokio::spawn(axum::serve(listener, Router::new().fallback(post(provider)).with_state(fixture.clone())).into_future());
    let _env = [Env::set("OPENAI_BASE_URL", base), Env::set("OPENAI_API_KEY", "disposable-fixture-key"), Env::set("HOME",dir.path()), Env::set("XDG_CONFIG_HOME",dir.path()), Env::set("OPENCODE_CONFIG_DIR",dir.path().join("global-config")), Env::set("OPENCODE_RK_HOME",dir.path()), Env::set("OPENCODE_RK_PERMISSION_RULES",dir.path().join("permission-rules.json")), Env::set("OPENCODE_RK_TURN_TOOLS","task,read")];
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap()));
    let parent = sessions.create_in_directory("parent",Some(dir.path().into())).await.unwrap();
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });
    let response = app.clone().oneshot(request(Method::POST, format!("/api/sessions/{}/turns/stream",parent.id), json!({"text":"CUSTOM_MODEL_DELEGATE delegate probe","model":"openai/fixture"}))).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = tokio::time::timeout(Duration::from_secs(10), to_bytes(response.into_body(), 1024 * 1024)).await
        .expect("config validation must settle the parent turn without waiting for approval").unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("subagent model must use provider/model format"), "expected model validation failure in tool output, got {text}");
    let permissions = value(app.clone().oneshot(request(Method::GET, format!("/api/session/{}/permission",parent.id), json!({}))).await.unwrap()).await;
    assert_eq!(permissions["data"], json!([]), "invalid config must fail before requesting a task permission");
    let listing = value(app.clone().oneshot(request(Method::GET, format!("/api/sessions/{}/children",parent.id), json!({}))).await.unwrap()).await;
    assert_eq!(listing["children"], json!([]), "invalid child model must be rejected before child session creation");
    assert_eq!(fixture.child_requests.load(Ordering::SeqCst), 0, "provider must never see a misconfigured child");
    assert_eq!(sessions.list(true).await.unwrap().len(), 1, "no orphan child session may remain");
    mock.abort();
}

#[tokio::test(flavor="multi_thread", worker_threads=2)]
async fn foreground_delegation_with_busy_second_permit_settles_quickly_and_manual_retry_recovers() {
    let _serial = TEST_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("probe.txt"), "fixture child content").unwrap();
    let fixture = Arc::new(Fixture::default());
    let (seen_tx, mut seen_rx) = tokio::sync::mpsc::unbounded_channel();
    *fixture.seen.lock().unwrap() = Some(seen_tx);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}",listener.local_addr().unwrap());
    let mock = tokio::spawn(axum::serve(listener, Router::new().fallback(post(provider)).with_state(fixture.clone())).into_future());
    let _env = [Env::set("OPENAI_BASE_URL", base), Env::set("OPENAI_API_KEY", "disposable-fixture-key"), Env::set("HOME",dir.path()), Env::set("XDG_CONFIG_HOME",dir.path()), Env::set("OPENCODE_CONFIG_DIR",dir.path().join("global-config")), Env::set("OPENCODE_RK_HOME",dir.path()), Env::set("OPENCODE_RK_PERMISSION_RULES",dir.path().join("permission-rules.json")), Env::set("OPENCODE_RK_TURN_TOOLS","task,read")];
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap()));
    let first = sessions.create_in_directory("first parent",Some(dir.path().into())).await.unwrap();
    let second = sessions.create_in_directory("second parent",Some(dir.path().into())).await.unwrap();
    let third = sessions.create_in_directory("third parent",Some(dir.path().into())).await.unwrap();
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });

    // Hold BOTH reserved child lanes with real background child turns. The
    // provider signals after admission, before withholding response headers.
    fixture.child_holds.store(2, Ordering::SeqCst);
    delegate(&app, &first.id, "ALIAS_RESEARCH lane one").await;
    delegate(&app, &second.id, "ALIAS_RESEARCH lane two").await;
    for _ in 0..2 {
        tokio::time::timeout(Duration::from_secs(5), seen_rx.recv()).await
            .expect("held child must signal admission").expect("fixture signal channel closed");
    }

    // A third child cannot obtain either CHILD permit; bounded admission
    // retries must exhaust while both child lanes remain held. Root turns stay
    // on their separate lane, so this is child-lane exhaustion, not root
    // starvation.
    let started = Instant::now();
    delegate(&app, &third.id, "ROOT_DELEGATE busy").await;
    let (failed, poll_duration) = poll(&app, format!("/api/sessions/{}/children",third.id),
        |v| v["children"].as_array().is_some_and(|rows| !rows.is_empty()) && v["children"][0]["state"] == "error").await;
    let attempt = &failed["children"][0];
    assert!(started.elapsed() < Duration::from_secs(8),
        "busy-child-lane delegation must settle bounded, took {:?} (backoff budget exceeded)", started.elapsed());
    assert!(started.elapsed() >= Duration::from_millis(1500),
        "five exponential admission retries must actually back off, took {:?}", started.elapsed());
    assert!(poll_duration < Duration::from_secs(8));
    assert_eq!(fixture.child_requests.load(Ordering::SeqCst), 2, "third child must never reach the provider while both child lanes are held");
    assert_eq!(attempt["auto_retry_attempts"].as_u64(), Some(5), "all bounded admission retries must be exhausted and observable: {failed}");
    assert_eq!(attempt["error"].as_str(), Some("child turn not admitted after 5 admission retries: too many active turns"),
        "unexpected final capacity error: {failed}");
    assert_eq!(attempt["state"], "error");

    // Free both held child lanes, then manual retry recovers the third child.
    fixture.released_child.store(true, Ordering::SeqCst);
    poll(&app, format!("/api/sessions/{}/children",first.id), |v| v["children"][0]["state"] == "completed").await;
    poll(&app, format!("/api/sessions/{}/children",second.id), |v| v["children"][0]["state"] == "completed").await;
    let retried = app.clone().oneshot(request(Method::POST, format!("/api/sessions/{}/children/{}/retry",third.id, attempt["session_id"].as_str().unwrap()), json!({}))).await.unwrap();
    assert_eq!(retried.status(), StatusCode::OK);
    let (recovered, _) = poll(&app, format!("/api/sessions/{}/children",third.id), |v| v["children"][0]["state"] == "completed").await;
    let child = &recovered["children"][0];
    assert_eq!(child["output_tail"], "step one done");
    assert_eq!(child["manual_retries"], 1);
    assert_eq!(child["auto_retry_attempts"], 0, "a fresh manual generation starts without automatic attempts");
    assert!(fixture.child_requests.load(Ordering::SeqCst) >= 3, "recovered child must reach the provider");
    mock.abort();
}

#[tokio::test(flavor="multi_thread", worker_threads=2)]
async fn active_child_followups_queue_bounded_and_drain_one_per_generation() {
    let _serial = TEST_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("probe.txt"), "fixture child content").unwrap();
    let fixture = Arc::new(Fixture::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}",listener.local_addr().unwrap());
    let mock = tokio::spawn(axum::serve(listener, Router::new().fallback(post(provider)).with_state(fixture.clone())).into_future());
    let _env = [Env::set("OPENAI_BASE_URL", base), Env::set("OPENAI_API_KEY", "disposable-fixture-key"), Env::set("HOME",dir.path()), Env::set("XDG_CONFIG_HOME",dir.path()), Env::set("OPENCODE_CONFIG_DIR",dir.path().join("global-config")), Env::set("OPENCODE_RK_HOME",dir.path()), Env::set("OPENCODE_RK_PERMISSION_RULES",dir.path().join("permission-rules.json")), Env::set("OPENCODE_RK_TURN_TOOLS","task,read")];
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap()));
    let parent = sessions.create_in_directory("parent",Some(dir.path().into())).await.unwrap();
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });
    fixture.child_holds.store(1, Ordering::SeqCst);
    delegate(&app, &parent.id, "ALIAS_RESEARCH queue drain").await;
    let (queued, _) = poll(&app, format!("/api/sessions/{}/children",parent.id), |v| v["children"][0]["state"] == "running").await;
    let child_id = queued["children"][0]["session_id"].as_str().unwrap().to_owned();
    // Follow-ups during an active generation queue instead of refusing.
    let follow = continue_child(&app, &parent.id, &child_id, "CHILD_STEP_TWO follow-up question").await;
    assert_eq!(follow.status(), StatusCode::OK);
    let bounded = value(app.clone().oneshot(request(Method::GET, format!("/api/sessions/{}/children",parent.id), json!({}))).await.unwrap()).await;
    assert_eq!(bounded["children"][0]["pending_followups"], 1);
    // Four more accepted, fifth rejected.
    for index in 0..3 {
        let text = format!("CHILD_STEP_TWO more {index}");
        let more = continue_child(&app, &parent.id, &child_id, &text).await;
        assert_eq!(more.status(), StatusCode::OK);
    }
    let overflow = continue_child(&app, &parent.id, &child_id, "CHILD_STEP_TWO overflow").await;
    assert_eq!(overflow.status(), StatusCode::TOO_MANY_REQUESTS, "queue must stay bounded");
    // Generation one completes, then the queued follow-up runs as its own
    // generation with its own run owner.
    fixture.released_child.store(true, Ordering::SeqCst);
    let (done, _) = poll(&app, format!("/api/sessions/{}/children",parent.id),
        |v| v["children"][0]["state"] == "completed" && v["children"][0]["pending_followups"] == 0 && v["children"][0]["output_tail"] == "step two done").await;
    assert_eq!(done["children"][0]["generation"], 5, "initial prompt plus four queued follow-ups must each run as a generation");
    let child: opencode_rk_contracts::SessionId = child_id.parse().unwrap();
    let user_texts: Vec<String> = sessions.messages(child,100).await.unwrap().into_iter()
        .filter(|m| m.role == opencode_rk_contracts::MessageRole::User)
        .filter_map(|m| match &m.body { opencode_rk_contracts::PayloadRef::Inline { text } => Some(text.clone()), _ => None })
        .collect();
    assert_eq!(user_texts.len(), 5, "initial prompt and four distinct follow-ups must be durable: {user_texts:?}");
    assert_eq!(user_texts.iter().collect::<HashSet<_>>().len(), 5, "each queued prompt must remain distinct: {user_texts:?}");
    mock.abort();
}

#[tokio::test(flavor="multi_thread", worker_threads=2)]
async fn cancelled_child_preserves_queued_followups_without_draining() {
    let _serial = TEST_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("probe.txt"), "fixture child content").unwrap();
    let fixture = Arc::new(Fixture::default());
    let (seen_tx, mut seen_rx) = tokio::sync::mpsc::unbounded_channel();
    *fixture.seen.lock().unwrap() = Some(seen_tx);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}",listener.local_addr().unwrap());
    let mock = tokio::spawn(axum::serve(listener, Router::new().fallback(post(provider)).with_state(fixture.clone())).into_future());
    let _env = [Env::set("OPENAI_BASE_URL", base), Env::set("OPENAI_API_KEY", "disposable-fixture-key"), Env::set("HOME",dir.path()), Env::set("XDG_CONFIG_HOME",dir.path()), Env::set("OPENCODE_CONFIG_DIR",dir.path().join("global-config")), Env::set("OPENCODE_RK_HOME",dir.path()), Env::set("OPENCODE_RK_PERMISSION_RULES",dir.path().join("permission-rules.json")), Env::set("OPENCODE_RK_TURN_TOOLS","task,read")];
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap()));
    let parent = sessions.create_in_directory("parent",Some(dir.path().into())).await.unwrap();
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });
    fixture.child_holds.store(1, Ordering::SeqCst);
    delegate(&app, &parent.id, "ALIAS_RESEARCH cancel queue").await;
    tokio::time::timeout(Duration::from_secs(5), seen_rx.recv()).await
        .expect("held child must reach the provider before stop").expect("fixture signal channel closed");
    let (queued, _) = poll(&app, format!("/api/sessions/{}/children",parent.id), |v| v["children"][0]["state"] == "running").await;
    let child_id = queued["children"][0]["session_id"].as_str().unwrap().to_owned();
    for index in 0..4 {
        let follow = continue_child(&app, &parent.id, &child_id, &format!("CHILD_STEP_TWO q{index}")).await;
        assert_eq!(follow.status(), StatusCode::OK);
    }
    // The held child never sends provider response headers; stop must drop the
    // admission future and its child-lane permit rather than wait for headers.
    let stopped = tokio::time::timeout(Duration::from_secs(3),
        app.clone().oneshot(request(Method::POST, format!("/api/sessions/{}/children/{child_id}/stop",parent.id), json!({}))))
        .await.expect("stop must cancel before provider headers arrive").unwrap();
    let stopped = value(stopped).await;
    assert_eq!(stopped["state"], "cancelled");
    assert_eq!(stopped["pending_followups"], 4, "cancellation must preserve queued prompts, never silently drain");
    mock.abort();
}

/// Every canonical built-in role must be reachable through the real parent
/// task tool, carry its canonical name/prompt and the inherited model into the
/// actual provider request, complete successfully, and enforce the advertised
/// read-only policy as a real tool dispatch decision: read-only roles attempt
/// an in-project write that is denied with no file, full roles perform the
/// same in-project write for real.
#[tokio::test(flavor="multi_thread", worker_threads=2)]
async fn all_builtin_roles_execute_with_canonical_identity_and_enforced_read_only() {
    let _serial = TEST_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let fixture = Arc::new(Fixture::default());
    *fixture.matrix_root.lock().unwrap() = Some(dir.path().to_path_buf());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}",listener.local_addr().unwrap());
    let mock = tokio::spawn(axum::serve(listener, Router::new().fallback(post(provider)).with_state(fixture.clone())).into_future());
    let _env = [Env::set("OPENAI_BASE_URL", base), Env::set("OPENAI_API_KEY", "disposable-fixture-key"), Env::set("HOME",dir.path()), Env::set("XDG_CONFIG_HOME",dir.path()), Env::set("OPENCODE_CONFIG_DIR",dir.path().join("global-config")), Env::set("OPENCODE_RK_HOME",dir.path()), Env::set("OPENCODE_RK_PERMISSION_RULES",dir.path().join("permission-rules.json")), Env::set("OPENCODE_RK_TURN_TOOLS","task,read,write")];
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap()));
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });

    // Public catalogue parity: exactly the ten canonical roles, no alias rows.
    let listing = value(app.clone().oneshot(request(Method::GET, "/api/commands".into(), json!({}))).await.unwrap()).await;
    let agents = listing["agents"].as_array().cloned().unwrap();
    let mut listed: Vec<&str> = agents.iter().filter_map(|agent| agent["name"].as_str()).collect();
    listed.sort_unstable();
    let mut expected: Vec<&str> = MATRIX.iter().map(|(role, _, _)| *role).collect();
    expected.sort_unstable();
    assert_eq!(listed, expected, "catalogue must expose exactly the ten canonical builtin roles: {agents:?}");
    for (role, read_only, _) in MATRIX {
        let entry = agents.iter().find(|agent| agent["name"] == role).expect("listed role");
        assert_eq!(entry["read_only"], json!(read_only), "{role} read_only policy drifted");
        assert!(!entry["description"].as_str().unwrap_or_default().is_empty(), "{role} needs a description");
    }

    for (role, read_only, phrase) in MATRIX {
        let parent = sessions.create_in_directory(format!("{role} matrix parent"),Some(dir.path().into())).await.unwrap();
        delegate(&app, &parent.id, &format!("ROLE_MATRIX {role} delegate")).await;
        if !read_only {
            approve_child_write(&app, &parent.id).await;
        }
        let (listing, _) = poll(&app, format!("/api/sessions/{}/children",parent.id),
            |value| value["children"].as_array().is_some_and(|rows| rows.len() == 1 && matches!(rows[0]["state"].as_str(), Some("completed" | "error")))).await;
        let child = &listing["children"][0];
        assert_eq!(child["agent"], role, "{role} child must carry the canonical builtin name");
        assert_eq!(child["model"], "openai/fixture", "{role} child must inherit the parent model");
        assert_eq!(child["read_only"], json!(read_only), "{role} snapshot must expose the enforced policy");
        assert_eq!(child["state"], if read_only { "error" } else { "completed" }, "{role} child terminal result must reflect its real tool policy: {child}");
        if read_only {
            assert!(child["error"].as_str().is_some_and(|error| error.contains("tool failed or was rejected")), "{role} rejected write must be reported as failed, not silently successful: {child}");
        } else {
            assert!(child["error"].is_null(), "{role} approved full-loop write must complete without error: {child}");
        }
        assert_eq!(child["output_tail"], format!("role {role} complete"), "{role} child must produce its own final text");
        assert_eq!(child["has_tool_calls"], true, "{role} child must dispatch its write attempt");

        let calls = fixture.matrix_calls.lock().unwrap();
        let call = calls.iter().find(|call| call.role == role).unwrap_or_else(|| panic!("{role} child never reached the provider"));
        assert_eq!(call.model, "fixture", "{role} child provider request must use the inherited model id");
        assert!(call.system.starts_with(&format!("Agent: {role}\n")), "{role} agent prompt must start with its canonical identity: {:?}", call.system);
        assert!(call.system.contains(phrase), "{role} agent prompt must contain its role briefing {phrase:?}: {:?}", call.system);
        assert!(call.system.contains("You cannot delegate further tasks."), "{role} agent prompt must forbid nested delegation");
        assert!(!call.tools.iter().any(|tool| tool == "task"), "{role} child must never be advertised nested delegation");
        assert_eq!(call.tools.iter().any(|tool| tool == "write"), !read_only,
            "{role} write advertisement must match the enforced policy: {:?}", call.tools);
        if read_only {
            assert!(call.feedback.contains("not permitted by turn policy"), "{role} write must be denied by the real turn policy, got {:?}", call.feedback);
            assert!(!dir.path().join(format!("matrix_{role}.txt")).exists(), "{role} denied write must leave no file");
        } else {
            assert!(call.feedback.contains("success") || call.feedback.contains("written"),
                "{role} full-loop write must succeed, got {:?}", call.feedback);
            assert_eq!(std::fs::read_to_string(dir.path().join(format!("matrix_{role}.txt"))).unwrap(), format!("{role} wrote"),
                "{role} full-loop write must persist the exact content");
        }
    }
    mock.abort();
}
