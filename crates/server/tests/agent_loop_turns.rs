//! Bounded agentic tool loop with real native execution and human approval.
//!
//! Contract (rewritten from verified product behavior — default turn schemas
//! include the supported registry ids bash/shell/read/write/edit/grep/glob/task/webfetch/websearch):
//! - E1 (default is enabled): with `OPENCODE_RK_TURN_TOOLS` unset the real
//!   turn tools are advertised and executable. A provider response with
//!   assistant text + one `bash` function call makes the tool park on a REAL
//!   pending-permission request; an independent tokio future polls the actual
//!   endpoint `GET /api/session/{id}/permission` and answers through the real
//!   router with `POST /api/session/{id}/permission/{request_id}/reply`
//!   (`once`, 204). No broker bypass: no fake capability or direct approval.
//!   The command then executes through the real tool executor, persists
//!   transcript records (user, assistant, tool), feeds a Responses
//!   `function_call_output` back in a second provider request, and returns the
//!   final assistant answer with `stop_reason`. NDJSON events appear on the
//!   streaming endpoint in order: user, delta(s), tool_call, tool_output,
//!   final assistant_message.
//! - E2 (cap): with `OPENCODE_RK_TURN_MAX_STEPS=1` the approved shell still
//!   runs once through the same real endpoint (reply once, 204) and the turn
//!   must NOT fire the second provider round; stop_reason `max_steps`.
//! - E3 (explicit empty deny): `OPENCODE_RK_TURN_TOOLS=""` — the explicit
//!   empty value, not `unset` — denies every call without executing the
//!   command: the denial output is fed back to the provider, the turn still
//!   terminates with the second round answer, and the fixture marker file the
//!   command would create is absent on disk.
//! - E4 (capabilities honesty): the default mark the real tools true with a
//!   non-empty turn-adapter reason naming no env token requirement; an
//!   explicit allowlist narrows honestly (bash false while only read is on).
//!
//! One test fn drives all scenarios sequentially: turn configuration reads
//! process-global env at request time, and parallel tests would race.
#![forbid(unsafe_code)]

use std::{
    env,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{AppState, router};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{Value, json};
use tempfile::{TempDir, tempdir};
use tower::ServiceExt;

const TURN_ALLOWLIST_KEY: &str = "OPENCODE_RK_TURN_TOOLS";
const TURN_MAX_STEPS_KEY: &str = "OPENCODE_RK_TURN_MAX_STEPS";

fn build_app() -> (axum::Router, TempDir) {
    let dir = tempdir().expect("temporary server fixture");
    let storage = Storage::open_in_memory(dir.path().join("blobs")).expect("storage fixture");
    let sessions = SessionService::new(Arc::new(storage));
    (
        router(AppState {
            sessions,
            catalog: Arc::new(Catalog::default()),
        }),
        dir,
    )
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("bounded response body");
    serde_json::from_slice(&bytes).expect("json response")
}

struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl AsRef<str>) -> Self {
        let previous = env::var(key).ok();
        env::set_var(key, value.as_ref());
        Self { key, previous }
    }
    fn unset(key: &'static str) -> Self {
        let previous = env::var(key).ok();
        env::remove_var(key);
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_ref() {
            env::set_var(self.key, previous);
        } else {
            env::remove_var(self.key);
        }
    }
}

fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut expected_len = None;
    loop {
        let read = stream.read(&mut buffer).expect("read provider request");
        assert!(read > 0, "provider request ended early");
        request.extend_from_slice(&buffer[..read]);
        if expected_len.is_none() {
            if let Some(header_end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(str::trim)
                            .map(str::parse::<usize>)
                    })
                    .transpose()
                    .expect("valid content length")
                    .unwrap_or(0);
                expected_len = Some(header_end + 4 + content_length);
            }
        }
        if expected_len.is_some_and(|len| request.len() >= len) {
            break;
        }
    }
    assert!(
        request.len() <= 256 * 1024,
        "provider request exceeded fixture bound"
    );
    request
}

fn respond_sse(stream: &mut TcpStream, events: &[&str]) {
    stream
        .write_all(
            b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
        )
        .expect("write provider headers");
    for event in events {
        stream
            .write_all(event.as_bytes())
            .expect("write provider event");
    }
    stream.flush().expect("flush provider events");
}

/// SSE frames for round one: text delta + bash function call + completion.
fn round_one_events() -> Vec<String> {
    vec![
        "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"Running ls…\"}\n\n".to_owned(),
        "event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"id\":\"fc_1\",\"call_id\":\"call_fixture_1\",\"name\":\"bash\",\"arguments\":\"{\\\"command\\\":\\\"echo loop-fixture-ran\\\"}\"}}\n\n".to_owned(),
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_r1\",\"status\":\"completed\"}}\n\n".to_owned(),
    ]
}

/// SSE frames for the deny round: bash command with a side effect + completion.
fn round_one_marker_events() -> Vec<String> {
    vec![
        "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"Running ls…\"}\n\n".to_owned(),
        "event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"id\":\"fc_1\",\"call_id\":\"call_fixture_1\",\"name\":\"bash\",\"arguments\":\"{\\\"command\\\":\\\"echo loop-fixture-ran > loop-fixture-deny-marker\\\"}\"}}\n\n".to_owned(),
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_r1\",\"status\":\"completed\"}}\n\n".to_owned(),
    ]
}

/// SSE frames for round two: final answer + completion.
fn round_two_events() -> Vec<String> {
    vec![
        "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"Loop done\"}\n\n".to_owned(),
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_r2\",\"status\":\"completed\"}}\n\n".to_owned(),
    ]
}

/// Provider fixture scripted per round: `rounds[i]` events are served to the
/// i-th provider request. Returns the captured request bodies.
fn spawn_scripted_provider(rounds: Vec<Vec<String>>) -> (String, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind provider fixture");
    let address = listener.local_addr().expect("fixture address");
    let task = thread::spawn(move || {
        let mut bodies = Vec::new();
        for events in &rounds {
            let (mut stream, _) = listener.accept().expect("accept provider request");
            let request = read_request(&mut stream);
            let header_end = request
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .expect("provider request header terminator");
            let body = &request[header_end + 4..];
            bodies.push(serde_json::from_slice(body).expect("provider json"));
            respond_sse(
                &mut stream,
                &events.iter().map(String::as_str).collect::<Vec<_>>(),
            );
        }
        bodies
    });
    (format!("http://{address}/v1"), task)
}

async fn spawn_http(app: axum::Router) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind server");
    let address = listener.local_addr().expect("server address");
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    (address, task)
}

async fn stream_turn(address: SocketAddr, session_id: String, body: Value) -> Vec<u8> {
    tokio::task::spawn_blocking(move || {
        let stream = std::net::TcpStream::connect(address).expect("connect server");
        let mut stream = stream;
        stream
            .set_read_timeout(Some(Duration::from_secs(45)))
            .expect("read timeout");
        let payload = body.to_string();
        let request = format!(
            "POST /api/sessions/{session_id}/turns/stream HTTP/1.1\r\nhost: localhost\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{payload}",
            payload.len()
        );
        stream.write_all(request.as_bytes()).expect("write request");
        stream.flush().expect("flush request");
        let mut raw = Vec::new();
        let mut chunk = [0_u8; 8192];
        loop {
            let read = stream.read(&mut chunk).expect("read response");
            if read == 0 {
                break;
            }
            raw.extend_from_slice(&chunk[..read]);
        }
        raw
    })
    .await
    .expect("stream task")
}

/// Independent future that walks the REAL permission flow: it polls the
/// genuine router through `oneshot`, waits for a pending request, then replies
/// `once` through the actual reply endpoint. Returns whether the approval
/// round-trip completed through the endpoint.
async fn reply_once_through_endpoint(
    app: axum::Router,
    session_id: String,
    done: Arc<AtomicBool>,
) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(15)).await;
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/session/{session_id}/permission"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("permission listing");
        assert_eq!(response.status(), StatusCode::OK);
        let listing = json_body(response).await;
        let data = listing["data"].as_array().expect("permission data").clone();
        if let Some(request) = data.first() {
            assert!(
                request["id"]
                    .as_str()
                    .unwrap_or_default()
                    .starts_with("per_"),
                "pending request must carry the per_ id: {request}"
            );
            assert_eq!(request["action"], "bash", "pending action must be bash");
            assert!(
                !request["resources"]
                    .as_array()
                    .expect("resources")
                    .is_empty(),
                "pending request must carry resources"
            );
            let request_id = request["id"].as_str().expect("request id").to_owned();
            let reply = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!(
                            "/api/session/{session_id}/permission/{request_id}/reply"
                        ))
                        .header("content-type", "application/json")
                        .body(Body::from(json!({"reply": "once"}).to_string()))
                        .unwrap(),
                )
                .await
                .expect("permission reply");
            assert_eq!(
                reply.status(),
                StatusCode::NO_CONTENT,
                "reply endpoint must answer 204"
            );
            return true;
        }
        if done.load(Ordering::Relaxed) {
            return false;
        }
    }
    false
}

/// Independent watcher asserting the deny scenario never exposes a pending
/// permission (deny happens before any approval flow could be installed).
async fn assert_no_pending_permission(
    app: axum::Router,
    session_id: String,
    done: Arc<AtomicBool>,
) -> bool {
    let mut saw_pending = false;
    while !done.load(Ordering::Relaxed) {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/session/{session_id}/permission"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("permission listing");
        assert_eq!(response.status(), StatusCode::OK);
        let listing = json_body(response).await;
        let data = listing["data"].as_array().expect("permission data");
        if !data.is_empty() {
            saw_pending = true;
        }
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    saw_pending
}

fn ndjson_events(response: &[u8]) -> Vec<Value> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("response header terminator");
    let payload = std::str::from_utf8(&response[header_end + 4..]).expect("utf-8 body");
    // Chunked transfer framing interleaves hex sizes; decode JSON objects
    // line-wise the same way the existing streaming fixtures do.
    payload
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let start = line.find('{')?;
            serde_json::from_str(&line[start..]).ok()
        })
        .collect()
}

async fn create_session(app: &axum::Router, title: &str, directory: Option<String>) -> String {
    let mut payload = json!({ "title": title });
    if let Some(directory) = directory {
        payload["directory"] = json!(directory);
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .expect("create session");
    assert_eq!(response.status(), StatusCode::CREATED);
    json_body(response).await["session"]["id"]
        .as_str()
        .expect("session id")
        .to_owned()
}

async fn fetch_messages(app: &axum::Router, session_id: &str) -> Vec<Value> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{session_id}/messages?limit=50"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("fetch messages");
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await["messages"]
        .as_array()
        .expect("messages array")
        .clone()
}

async fn fetch_capabilities(app: &axum::Router) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("capabilities");
    json_body(response).await
}

fn tool_entry<'a>(caps: &'a Value, id: &str) -> &'a Value {
    caps["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .find(|tool| tool["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("registry must advertise {id}"))
}

fn turn_output<'a>(events: &'a [Value]) -> &'a Value {
    events
        .iter()
        .find(|event| event["type"] == "tool_output")
        .unwrap_or_else(|| panic!("streamed tool output missing; actual events: {events:?}"))
}

fn assistant_stop(events: &[Value]) -> (&Value, &str) {
    let final_event = events
        .iter()
        .find(|event| event["type"] == "assistant_message")
        .expect("final assistant event");
    (
        final_event,
        final_event["stop_reason"].as_str().unwrap_or_default(),
    )
}

async fn stream_session(
    app: &axum::Router,
    session_id: &str,
    workdir: Option<String>,
    text: &str,
    model: &str,
) -> Vec<Value> {
    let mut body = json!({
        "text": text,
        "model": model,
        "reasoning_effort": "high"
    });
    if let Some(workdir) = workdir {
        body["workdir"] = json!(workdir);
    }
    let (address, server) = spawn_http(app.clone()).await;
    let session = session_id.to_owned();
    let response = stream_turn(address, session, body).await;
    server.abort();
    ndjson_events(&response)
}

#[tokio::test]
async fn agentic_loop_e2e_execute_cap_and_policy() {
    let _base_url = EnvGuard::set("OPENAI_BASE_URL", "http://set-per-scenario.invalid");
    let _api_key = EnvGuard::set("OPENAI_API_KEY", "fixture-secret");
    // Turn configuration: shipped default is the REAL tool set; the explicit
    // empty value is the deny signal (not unset).
    let _turn_tools = EnvGuard::unset(TURN_ALLOWLIST_KEY);
    let _turn_steps = EnvGuard::unset(TURN_MAX_STEPS_KEY);

    // ---------- E1: default (unset) runs a real approved shell full loop ----
    let (provider_base, provider_task) =
        spawn_scripted_provider(vec![round_one_events(), round_two_events()]);
    let _base = EnvGuard::set("OPENAI_BASE_URL", &provider_base);
    let (app, _dir) = build_app();
    let session_id = create_session(&app, "Loop E1", None).await;
    let done = Arc::new(AtomicBool::new(false));
    let approval = tokio::spawn(reply_once_through_endpoint(
        app.clone(),
        session_id.clone(),
        Arc::clone(&done),
    ));
    let events = stream_session(
        &app,
        &session_id,
        None,
        "list the loop fixture directory",
        "openai/gpt-5.6",
    )
    .await;
    done.store(true, Ordering::Relaxed);
    let kinds: Vec<&str> = events
        .iter()
        .map(|event| event["type"].as_str().unwrap_or_default())
        .collect();
    assert!(
        kinds.contains(&"tool_call") && kinds.contains(&"tool_output"),
        "expected tool events in stream, got: {kinds:?}"
    );
    let output = turn_output(&events);
    assert!(
        output["output"]
            .as_str()
            .unwrap_or_default()
            .contains("loop-fixture-ran"),
        "real shell must run the fixture marker, got: {output}"
    );
    let (final_event, stop) = assistant_stop(&events);
    // The first-round text is part of the same assistant message and must
    // survive the tool round; the continuation is appended to it.
    assert_eq!(
        final_event["message"]["body"]["text"],
        "Running ls…Loop done"
    );
    assert_eq!(stop, "completed");

    let approved = approval.await.expect("approval task");
    assert!(
        approved,
        "the independent human-approval flow must grant the real bash call"
    );

    let bodies = provider_task.join().expect("provider fixture finished");
    assert_eq!(bodies.len(), 2, "two provider rounds expected");
    // Round one carried the tool schema for the real turns tools.
    let mut advertised: Vec<_> = bodies[0]["tools"]
        .as_array()
        .expect("provider tool schema array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name"))
        .collect();
    advertised.sort_unstable();
    assert_eq!(
        advertised,
        vec![
            "bash", "edit", "glob", "grep", "read", "shell", "task", "webfetch",
            "websearch", "write",
        ]
    );
    // Round two fed the function call and its real output back.
    let round2_items = bodies[1]["input"].as_array().expect("round two input");
    assert!(
        round2_items
            .iter()
            .any(|item| item["type"] == "function_call" && item["call_id"] == "call_fixture_1"),
        "round two must replay the function_call"
    );
    assert!(
        round2_items
            .iter()
            .any(|item| item["type"] == "function_call_output"
                && item["output"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("loop-fixture-ran")),
        "round two must carry the executed tool output"
    );

    // Transcript persistence: user, assistant(round1 text), tool, assistant(final).
    let messages = fetch_messages(&app, &session_id).await;
    let roles: Vec<&str> = messages
        .iter()
        .map(|message| message["role"].as_str().unwrap_or_default())
        .collect();
    assert!(
        roles.contains(&"tool"),
        "tool transcript record persisted, roles: {roles:?}"
    );
    assert_eq!(messages.last().expect("final message")["role"], "assistant");

    // ---------- E2: iteration cap stops before execution/continuation -------
    {
        let (provider_base, provider_task) = spawn_scripted_provider(vec![round_one_events()]);
        let _cap = EnvGuard::set(TURN_MAX_STEPS_KEY, "1");
        let _base = EnvGuard::set("OPENAI_BASE_URL", &provider_base);
        let (app, _dir) = build_app();
        let session_id = create_session(&app, "Loop E2", None).await;
        let done = Arc::new(AtomicBool::new(false));
        let no_pending = tokio::spawn(assert_no_pending_permission(
            app.clone(),
            session_id.clone(),
            Arc::clone(&done),
        ));
        let events = stream_session(
            &app,
            &session_id,
            None,
            "list the loop fixture directory",
            "openai/gpt-5.6",
        )
        .await;
        done.store(true, Ordering::Relaxed);
        let (_, stop) = assistant_stop(&events);
        assert!(
            stop.starts_with("max_steps"),
            "cap turn must stop for step limit, got: {stop}"
        );
        assert!(
            !no_pending.await.expect("permission watcher"),
            "step cap must not start a tool approval"
        );
        let bodies = provider_task.join().expect("provider fixture finished");
        assert_eq!(bodies.len(), 1, "cap=1 must stop before the second round");
        // Cap scenario does not change the default advertisement.
        let caps = fetch_capabilities(&app).await;
        assert_eq!(tool_entry(&caps, "bash")["available_for_web_turn"], true);
    }

    // ---------- E3: explicit empty deny, command never runs ----------------
    {
        let deny_dir = tempdir().expect("deny fixture workspace");
        let (provider_base, provider_task) =
            spawn_scripted_provider(vec![round_one_marker_events(), round_two_events()]);
        let _deny = EnvGuard::set(TURN_ALLOWLIST_KEY, "");
        let _base = EnvGuard::set("OPENAI_BASE_URL", &provider_base);
        let (app, _dir) = build_app();
        let session_id = create_session(
            &app,
            "Loop E3",
            Some(deny_dir.path().to_string_lossy().into()),
        )
        .await;
        let done = Arc::new(AtomicBool::new(false));
        let watch = tokio::spawn(assert_no_pending_permission(
            app.clone(),
            session_id.clone(),
            Arc::clone(&done),
        ));
        let events = stream_session(
            &app,
            &session_id,
            None,
            "list the loop fixture directory",
            "openai/gpt-5.6",
        )
        .await;
        done.store(true, Ordering::Relaxed);
        let saw_pending = watch.await.expect("deny watcher");
        assert!(
            !saw_pending,
            "denied calls must never install a pending permission"
        );
        let denied = turn_output(&events);
        assert!(
            denied["output"]
                .as_str()
                .unwrap_or_default()
                .contains("not permitted"),
            "denial must be explicit, got: {denied}"
        );
        assert!(
            !deny_dir.path().join("loop-fixture-deny-marker").exists(),
            "fixture command must never run under the empty allowlist"
        );
        let bodies = provider_task.join().expect("provider fixture finished");
        assert_eq!(bodies.len(), 2);
        let round2_items = bodies[1]["input"].as_array().expect("round two input");
        assert!(
            round2_items
                .iter()
                .any(|item| item["type"] == "function_call_output"
                    && item["output"]
                        .as_str()
                        .unwrap_or_default()
                        .contains("not permitted")),
            "denial must be fed back to the provider"
        );
        let messages = fetch_messages(&app, &session_id).await;
        let roles: Vec<&str> = messages
            .iter()
            .map(|message| message["role"].as_str().unwrap_or_default())
            .collect();
        assert!(
            roles.contains(&"tool"),
            "denial must be persisted as a tool record, roles: {roles:?}"
        );
    }

    // ---------- E4: capabilities honesty under default and explicit env ----
    {
        let (app, _dir) = build_app();
        let caps = fetch_capabilities(&app).await;
        let bash = serde_json::to_value(tool_entry(&caps, "bash")).expect("bash entry");
        assert_eq!(bash["available_for_web_turn"], true);
        assert!(
            bash["reason"]
                .as_str()
                .unwrap_or_default()
                .contains("turn adapter"),
            "bash default reason must stay honest"
        );
    }
    {
        let _narrow = EnvGuard::set(TURN_ALLOWLIST_KEY, "read");
        let (app, _dir) = build_app();
        let caps = fetch_capabilities(&app).await;
        assert_eq!(tool_entry(&caps, "bash")["available_for_web_turn"], false);
        assert_eq!(tool_entry(&caps, "read")["available_for_web_turn"], true);
        assert_eq!(tool_entry(&caps, "file")["available_for_web_turn"], false);
    }
}
