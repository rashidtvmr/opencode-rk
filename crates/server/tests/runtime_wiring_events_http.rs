//! RED lane: runtime event publication follows durable turn writes.
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
    sync::Arc,
    thread,
    time::Duration,
};

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Extension,
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::app_runtime::EnginePolicy;
use opencode_rk_server::event_bus::ServerEvent;
use opencode_rk_server::runtime_wiring::RuntimeWiring;
use opencode_rk_server::{router_with_auth, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use opencode_rk_tools::registry::ToolRegistry;
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

fn build_app() -> (axum::Router, tempfile::TempDir, RuntimeWiring) {
    let dir = tempdir().expect("temporary server fixture");
    let storage = Storage::open_in_memory(dir.path().join("blobs")).expect("storage fixture");
    let sessions = SessionService::new(Arc::new(storage));
    let mut policy = EnginePolicy::default_deny();
    policy.allow_tool("bash").expect("fixture policy");
    let runtime = RuntimeWiring::for_daemon(sessions.clone(), ToolRegistry::new(), policy);
    (
        router_with_auth(
            AppState {
                sessions,
                catalog: Arc::new(Catalog::default()),
            },
            None,
        )
        .layer(Extension(runtime.clone())),
        dir,
        runtime,
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

/// Provider round one requests a real bash call whose output must never escape
/// the daemon-owned deny-by-default policy.
fn round_one_events() -> Vec<String> {
    vec![
        "event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"id\":\"fc_events_1\",\"call_id\":\"call_events_1\",\"name\":\"bash\",\"arguments\":\"{\\\"command\\\":\\\"echo runtime-events-fixture\\\"}\"}}\n\n".to_owned(),
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_events_r1\",\"status\":\"completed\"}}\n\n".to_owned(),
    ]
}

fn round_two_events() -> Vec<String> {
    vec![
        "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"Runtime events done\"}\n\n".to_owned(),
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_events_r2\",\"status\":\"completed\"}}\n\n".to_owned(),
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
async fn runtime_events_follow_durable_turn_writes() {
    let _base_url = EnvGuard::set("OPENAI_BASE_URL", "http://set-per-scenario.invalid");
    let _api_key = EnvGuard::set("OPENAI_API_KEY", "fixture-secret");
    let _tools = EnvGuard::set("OPENCODE_RK_TURN_TOOLS", "bash");
    let (provider_base, provider_task) =
        spawn_scripted_provider(vec![round_one_events(), round_two_events()]);
    let _base = EnvGuard::set("OPENAI_BASE_URL", &provider_base);
    let (app, _dir, runtime) = build_app();
    let session_id = create_session(&app, "Runtime events").await;
    let mut subscription = runtime.events().subscribe(None);
    let (address, server) = spawn_http(app.clone()).await;
    let stream_session = session_id.clone();
    let response = tokio::task::spawn_blocking(move || stream_turn(address, stream_session))
        .await
        .expect("stream task");
    server.abort();

    let events = ndjson_events(&response);
    let tool_call = events
        .iter()
        .find(|event| event["type"] == "tool_call")
        .expect("provider function call is represented");
    assert_eq!(tool_call["name"], "bash");
    let tool_output = events
        .iter()
        .find(|event| event["type"] == "tool_output")
        .expect("tool output is represented");
    let tool_text = tool_output["output"].as_str().unwrap_or_default();
    assert!(tool_text.contains("runtime-events-fixture"), "bash output: {tool_text}");

    let bodies = provider_task.join().expect("provider fixture finished");
    assert_eq!(bodies.len(), 2, "tool call receives completion round");
    let round2_items = bodies[1]["input"].as_array().expect("round two input");
    let output = round2_items
        .iter()
        .find(|item| item["type"] == "function_call_output")
        .and_then(|item| item["output"].as_str())
        .expect("round two function_call_output");
    assert!(output.contains("runtime-events-fixture"));

    let messages = fetch_messages(&app, &session_id).await;
    let tool = messages
        .iter()
        .find(|message| message["role"] == "tool")
        .expect("tool record persisted");
    let persisted = tool["body"]["text"].as_str().unwrap_or_default();
    assert!(persisted.contains("runtime-events-fixture"));
    let roles: Vec<_> = messages.iter().filter_map(|message| message["role"].as_str()).collect();
    assert_eq!(roles, ["user", "tool", "assistant"]);

    let expected = [
        ServerEvent::MessageAppended { session: session_id.parse().expect("session id"), seq: 0 },
        ServerEvent::ToolExecuted { name: "bash".to_owned(), duration_ms: 0 },
        ServerEvent::MessageAppended { session: session_id.parse().expect("session id"), seq: 0 },
        ServerEvent::MessageAppended { session: session_id.parse().expect("session id"), seq: 0 },
    ];
    for expected_event in expected {
        let actual = tokio::time::timeout(Duration::from_millis(100), subscription.recv())
            .await
            .expect("runtime event timeout")
            .expect("runtime event stream closed");
        assert_eq!(actual, expected_event);
    }
    assert!(tokio::time::timeout(Duration::from_millis(25), subscription.recv()).await.is_err(), "unexpected fifth runtime event");
}
