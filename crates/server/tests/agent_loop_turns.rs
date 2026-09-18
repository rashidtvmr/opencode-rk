//! RED lane: bounded agentic tool loop over the live turn endpoints.
//!
//! Contract (TDD, frozen once RED is observed):
//! - E1 (tools enabled via OPENCODE_RK_TURN_TOOLS=bash): a provider response
//!   with assistant text + one `bash` function call makes the server execute
//!   the command through the real tool executor, persist transcript records
//!   (user, assistant, tool), feed a Responses `function_call_output` back in
//!   a second provider request, and return the final assistant answer with a
//!   `stop_reason`. NDJSON events appear on the streaming endpoint in order:
//!   user, delta(s), tool_call, tool_output, final assistant_message.
//! - E2 (cap): with OPENCODE_RK_TURN_MAX_STEPS=1 the same provider script
//!   must NOT fire the second provider round; the turn terminates with
//!   stop_reason `max_steps` and the assistant text from round one.
//! - E3 (default policy is deny): without the enable flag, the bash call is
//!   NOT executed (fixture asserts command never ran), output says denied,
//!   and the turn still terminates with the second round answer.
//! - E4 (policy honesty): capabilities mark tools `available_for_web_turn`
//!   true only under the enable flag, with an explanatory reason either way.
//!
//! One test fn drives all scenarios sequentially: turn configuration reads
//! process-global env at request time, and parallel tests would race.
#![forbid(unsafe_code)]

use std::{
    env,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{mpsc, Arc},
    thread,
    time::Duration,
};

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

fn build_app() -> (axum::Router, tempfile::TempDir) {
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
            respond_sse(&mut stream, &events.iter().map(String::as_str).collect::<Vec<_>>());
        }
        bodies
    });
    (format!("http://{address}/v1"), task)
}

async fn spawn_http(app: axum::Router) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind server");
    let address = listener.local_addr().expect("server address");
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    (address, task)
}

fn stream_turn(address: SocketAddr, session_id: String) -> Vec<u8> {
    let stream = TcpStream::connect(address).expect("connect server");
    let mut stream = stream;
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("read timeout");
    let body = json!({
        "text": "list the loop fixture directory",
        "model": "openai/gpt-5.6",
        "reasoning_effort": "high"
    })
    .to_string();
    let request = format!(
        "POST /api/sessions/{session_id}/turns/stream HTTP/1.1\r\nhost: localhost\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
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
}

fn ndjson_events(response: &[u8]) -> Vec<Value> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("response header terminator");
    let payload = std::str::from_utf8(&response[header_end + 4..]).expect("utf-8 body");
    // Chunked transfer framing interleaves hex sizes; decode JSON objects
    // line-wise the same way the existing streaming fixture does.
    payload
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let start = line.find('{')?;
            serde_json::from_str(&line[start..]).ok()
        })
        .collect()
}

async fn create_session(app: &axum::Router, title: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "title": title }).to_string()))
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

#[tokio::test]
async fn agentic_loop_e2e_execute_cap_and_policy() {
    let _base_url = EnvGuard::set("OPENAI_BASE_URL", "http://set-per-scenario.invalid");
    let _api_key = EnvGuard::set("OPENAI_API_KEY", "fixture-secret");

    // ---------- E1: tools enabled, full loop ----------
    let (provider_base, provider_task) = spawn_scripted_provider(vec![
        round_one_events(),
        round_two_events(),
    ]);
    let _enable = EnvGuard::set("OPENCODE_RK_TURN_TOOLS", "bash");
    let _base = EnvGuard::set("OPENAI_BASE_URL", &provider_base);
    let (app, _dir) = build_app();
    let session_id = create_session(&app, "Loop E1").await;
    let (address, server) = spawn_http(app.clone()).await;
    let stream_session = session_id.clone();
    let response = tokio::task::spawn_blocking(move || stream_turn(address, stream_session))
        .await
        .expect("stream task");
    server.abort();

    let events = ndjson_events(&response);
    let kinds: Vec<&str> = events
        .iter()
        .map(|event| event["type"].as_str().unwrap_or_default())
        .collect();
    assert!(
        kinds.contains(&"tool_call") && kinds.contains(&"tool_output"),
        "expected tool events in stream, got: {kinds:?}"
    );
    let final_event = events
        .iter()
        .find(|event| event["type"] == "assistant_message")
        .expect("final assistant event");
    assert_eq!(final_event["message"]["body"]["text"], "Loop done");
    assert_eq!(final_event["stop_reason"], "completed");

    let bodies = provider_task.join().expect("provider fixture finished");
    assert_eq!(bodies.len(), 2, "two provider rounds expected");
    // Round one carried the tool schema.
    assert_eq!(bodies[0]["tools"][0]["name"], "bash");
    // Round two fed the function call and its output back.
    let round2_items = bodies[1]["input"].as_array().expect("round two input");
    assert!(
        round2_items.iter().any(|item| item["type"] == "function_call"
            && item["call_id"] == "call_fixture_1"),
        "round two must replay the function_call"
    );
    assert!(
        round2_items.iter().any(|item| item["type"] == "function_call_output"
            && item["output"].as_str().unwrap_or_default().contains("loop-fixture-ran")),
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
    assert_eq!(
        messages.last().expect("final message")["role"],
        "assistant"
    );

    // ---------- E2: iteration cap ----------
    // Under cap=1 the loop must stop after round one: the fixture serves a
    // single round and a second provider request would be a contract breach.
    let (provider_base, provider_task) = spawn_scripted_provider(vec![round_one_events()]);
    let _enable = EnvGuard::set("OPENCODE_RK_TURN_TOOLS", "bash");
    let _cap = EnvGuard::set("OPENCODE_RK_TURN_MAX_STEPS", "1");
    let _base = EnvGuard::set("OPENAI_BASE_URL", &provider_base);
    let (app, _dir) = build_app();
    let session_id = create_session(&app, "Loop E2").await;
    let (address, server) = spawn_http(app.clone()).await;
    let response = tokio::task::spawn_blocking(move || stream_turn(address, session_id))
        .await
        .expect("stream task");
    server.abort();

    let events = ndjson_events(&response);
    let final_event = events
        .iter()
        .find(|event| event["type"] == "assistant_message")
        .expect("capped turn still ends with assistant message");
    let stop = final_event["stop_reason"].as_str().unwrap_or_default();
    assert!(
        stop.starts_with("max_steps"),
        "cap turn must stop for step limit, got: {stop}"
    );
    let bodies = provider_task.join().expect("provider fixture finished");
    assert_eq!(bodies.len(), 1, "cap=1 must stop before the second round");

    // ---------- E3: default policy denies ----------
    let (provider_base, provider_task) = spawn_scripted_provider(vec![
        round_one_events(),
        round_two_events(),
    ]);
    env::remove_var("OPENCODE_RK_TURN_TOOLS");
    env::remove_var("OPENCODE_RK_TURN_MAX_STEPS");
    let _base = EnvGuard::set("OPENAI_BASE_URL", &provider_base);
    let (app, _dir) = build_app();
    let session_id = create_session(&app, "Loop E3").await;
    let (address, server) = spawn_http(app.clone()).await;
    let response = tokio::task::spawn_blocking(move || stream_turn(address, session_id))
        .await
        .expect("stream task");
    server.abort();

    let events = ndjson_events(&response);
    let denied = events
        .iter()
        .find(|event| event["type"] == "tool_output")
        .expect("denied call still reports an output");
    assert!(
        denied["output"]
            .as_str()
            .unwrap_or_default()
            .contains("not permitted"),
        "denial must be explicit, got: {denied}"
    );
    let bodies = provider_task.join().expect("provider fixture finished");
    assert_eq!(bodies.len(), 2);
    let round2_items = bodies[1]["input"].as_array().expect("round two input");
    assert!(
        round2_items.iter().any(|item| item["type"] == "function_call_output"
            && item["output"].as_str().unwrap_or_default().contains("not permitted")),
        "denial must be fed back to the provider"
    );

    // ---------- E4: capabilities honesty ----------
    // Re-enable for the "on" report (E3 cleared the allowlist).
    let _enable4 = EnvGuard::set("OPENCODE_RK_TURN_TOOLS", "bash");
    let caps = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("capabilities");
    let caps = json_body(caps).await;
    let bash = caps["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .find(|tool| tool["id"] == "bash")
        .expect("bash capability entry")
        .clone();
    assert_eq!(bash["available_for_web_turn"], true);
    assert!(bash["reason"].as_str().unwrap_or_default().contains("OPENCODE_RK_TURN_TOOLS"));

    env::remove_var("OPENCODE_RK_TURN_TOOLS");
    let (app2, _dir2) = build_app();
    let caps2 = app2
        .oneshot(
            Request::builder()
                .uri("/api/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("capabilities default");
    let caps2 = json_body(caps2).await;
    let bash2 = caps2["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .find(|tool| tool["id"] == "bash")
        .expect("bash capability entry")
        .clone();
    assert_eq!(bash2["available_for_web_turn"], false);
}
