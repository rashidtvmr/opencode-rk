//! APP-012 RED: authenticated provider turn dispatches a bounded workspace read.
//!
//! The fixture uses the real router, runtime, permission broker, and turn
//! executor. It must compile before `read` is wired into the live executor.
#![forbid(unsafe_code)]

use std::{
    env,
    ffi::OsString,
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::Duration,
};

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Extension,
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{
    app_runtime::EnginePolicy, daemon_auth::DaemonAuth, router_with_auth,
    runtime_wiring::RuntimeWiring, AppState,
};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{Storage, StoragePaths};
use opencode_rk_tools::registry::ToolRegistry;
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

const MAX_REQUEST_BYTES: usize = 128 * 1024;
const MAX_RESPONSE_BYTES: usize = 256 * 1024;
const READ_LIMIT: usize = 12;
const FILE_CONTENT: &str = "0123456789abcdefTAIL";
const EXPECTED_OUTPUT: &str = "0123456789ab";
const FINAL_TEXT: &str = "safe read complete";

struct EnvGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl AsRef<str>) -> Self {
        let previous = env::var_os(key);
        env::set_var(key, value.as_ref());
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            env::set_var(self.key, previous);
        } else {
            env::remove_var(self.key);
        }
    }
}

struct CurrentDirGuard(PathBuf);

impl CurrentDirGuard {
    fn enter(path: &Path) -> Self {
        let previous = env::current_dir().expect("capture current directory");
        env::set_current_dir(path).expect("enter disposable workspace");
        Self(previous)
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.0);
    }
}

fn read_http_request(stream: &mut TcpStream) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("provider read timeout");
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut expected_len = None;
    loop {
        let count = stream.read(&mut buffer).expect("read provider request");
        assert!(count > 0, "provider request ended early");
        request.extend_from_slice(&buffer[..count]);
        assert!(
            request.len() <= MAX_REQUEST_BYTES,
            "provider request exceeded bound"
        );
        if expected_len.is_none() {
            if let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let length = headers
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
                expected_len = Some(header_end + 4 + length);
            }
        }
        if expected_len.is_some_and(|length| request.len() >= length) {
            return request;
        }
    }
}

fn write_sse(stream: &mut TcpStream, events: &[String]) {
    let mut response =
        b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n".to_vec();
    for event in events {
        response.extend_from_slice(event.as_bytes());
    }
    assert!(
        response.len() <= MAX_REQUEST_BYTES,
        "provider response exceeded bound"
    );
    stream
        .write_all(&response)
        .expect("write provider response");
    stream.flush().expect("flush provider response");
}

fn sse_event(name: &str, data: Value) -> String {
    format!("event: {name}\ndata: {data}\n\n")
}

fn read_call(path: &Path) -> String {
    sse_event(
        "response.output_item.done",
        json!({
            "type": "response.output_item.done",
            "item": {
                "type": "function_call",
                "id": "call-safe-read",
                "call_id": "call-safe-read",
                "name": "read",
                "arguments": json!({
                    "path": path.to_string_lossy(),
                    "limit": READ_LIMIT,
                }).to_string(),
            }
        }),
    )
}

fn completed(id: &str) -> String {
    sse_event(
        "response.completed",
        json!({
            "type": "response.completed",
            "response": {"id": id, "status": "completed"},
        }),
    )
}

fn provider_fixture(path: PathBuf) -> (String, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback provider");
    let address = listener.local_addr().expect("provider address");
    let task = thread::spawn(move || {
        let mut requests = Vec::with_capacity(2);
        let rounds = [
            vec![read_call(&path), completed("response-read-1")],
            vec![
                sse_event(
                    "response.output_text.delta",
                    json!({"type": "response.output_text.delta", "delta": FINAL_TEXT}),
                ),
                completed("response-read-2"),
            ],
        ];
        for events in rounds {
            let (mut stream, _) = listener.accept().expect("accept provider request");
            let request = read_http_request(&mut stream);
            let header_end = request
                .windows(4)
                .position(|part| part == b"\r\n\r\n")
                .expect("provider header terminator");
            requests.push(
                serde_json::from_slice(&request[header_end + 4..]).expect("provider JSON request"),
            );
            write_sse(&mut stream, &events);
        }
        requests
    });
    (format!("http://{address}/v1"), task)
}

fn build_app(root: &Path, auth: DaemonAuth) -> (axum::Router, RuntimeWiring) {
    let storage = Storage::open(StoragePaths::under(root.join("state"))).expect("open fixture DB");
    let sessions = SessionService::new(Arc::new(storage));
    let mut policy = EnginePolicy::default_deny();
    policy.allow_tool("read").expect("allow fixture read");
    let runtime = RuntimeWiring::for_daemon(sessions.clone(), ToolRegistry::new(), policy);
    let app = router_with_auth(
        AppState {
            sessions,
            catalog: Arc::new(Catalog::default()),
        },
        Some(auth),
    )
    .layer(Extension(runtime.clone()));
    (app, runtime)
}

fn auth_request(method: &str, uri: &str, token: &str, body: Option<String>) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"));
    if let Some(body) = body {
        request = request.header("content-type", "application/json");
        request.body(Body::from(body)).expect("JSON request")
    } else {
        request.body(Body::empty()).expect("empty request")
    }
}

async fn create_session(app: &axum::Router, token: &str) -> String {
    let response = app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/sessions",
            token,
            Some(json!({"title": "APP-012 read"}).to_string()),
        ))
        .await
        .expect("create authenticated session");
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(response.into_body(), 16 * 1024)
        .await
        .expect("bounded session body");
    serde_json::from_slice::<Value>(&body).expect("session JSON")["session"]["id"]
        .as_str()
        .expect("session ID")
        .to_owned()
}

async fn spawn_server(app: axum::Router) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind server");
    let address = listener.local_addr().expect("server address");
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve server");
    });
    (address, task)
}

fn stream_turn(address: SocketAddr, session_id: &str, token: &str) -> Vec<u8> {
    let mut stream = TcpStream::connect(address).expect("connect server");
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("stream read timeout");
    let body = json!({
        "text": "Read the bounded fixture.",
        "model": "openai/fixture-model",
        "reasoning_effort": "high",
    })
    .to_string();
    let request = format!(
        "POST /api/sessions/{session_id}/turns/stream HTTP/1.1\r\nhost: {address}\r\nauthorization: Bearer {token}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .expect("write turn request");
    stream.flush().expect("flush turn request");
    let mut response = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let count = stream.read(&mut buffer).expect("read turn response");
        if count == 0 {
            break;
        }
        response.extend_from_slice(&buffer[..count]);
        assert!(
            response.len() <= MAX_RESPONSE_BYTES,
            "stream response exceeded bound"
        );
    }
    response
}

fn stream_events(response: &[u8]) -> Vec<Value> {
    let header_end = response
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .expect("stream response headers");
    std::str::from_utf8(&response[header_end + 4..])
        .expect("stream UTF-8")
        .lines()
        .filter_map(|line| {
            let start = line.find('{')?;
            serde_json::from_str(&line[start..]).ok()
        })
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn app012_authenticated_read_journey_is_compiling_red() {
    let fixture = tempdir().expect("disposable fixture");
    let workspace = fixture.path().join("workspace");
    fs::create_dir_all(&workspace).expect("create workspace");
    let safe_path = workspace.join("safe.txt");
    fs::write(&safe_path, FILE_CONTENT).expect("write fixture file");
    let _current_dir = CurrentDirGuard::enter(&workspace);

    let (provider_base, provider_task) = provider_fixture(safe_path);
    let _base_url = EnvGuard::set("OPENAI_BASE_URL", &provider_base);
    let _api_key = EnvGuard::set("OPENAI_API_KEY", "fixture-key");
    let _timeout = EnvGuard::set("OPENAI_TIMEOUT_SECS", "3");
    let _max_tokens = EnvGuard::set("OPENAI_MAX_TOKENS", "64");
    let _turn_tools = EnvGuard::set("OPENCODE_RK_TURN_TOOLS", "read");
    let _turn_steps = EnvGuard::set("OPENCODE_RK_TURN_MAX_STEPS", "2");

    let auth = DaemonAuth::mint().expect("mint fixture auth");
    let token = auth.token().to_owned();
    let (app, _runtime) = build_app(fixture.path(), auth);
    let session_id = create_session(&app, &token).await;
    let (address, server) = spawn_server(app.clone()).await;
    let stream_session = session_id.clone();
    let stream_token = token.clone();
    let response =
        tokio::task::spawn_blocking(move || stream_turn(address, &stream_session, &stream_token))
            .await
            .expect("stream task");
    server.abort();
    let _ = server.await;

    let events = stream_events(&response);
    let tool_call = events
        .iter()
        .find(|event| event["type"] == "tool_call")
        .expect("read tool call event");
    assert_eq!(tool_call["name"], "read");
    let tool_output = events
        .iter()
        .find(|event| event["type"] == "tool_output")
        .expect("read tool output event");
    assert_eq!(
        tool_output["output"], EXPECTED_OUTPUT,
        "real read dispatch must return bounded file content"
    );
    let final_event = events
        .iter()
        .find(|event| event["type"] == "assistant_message")
        .expect("final assistant event");
    assert_eq!(final_event["message"]["body"]["text"], FINAL_TEXT);

    let requests = provider_task.join().expect("provider fixture joins");
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0]["tools"]
            .as_array()
            .expect("advertised tools")
            .iter()
            .map(|tool| tool["name"].as_str().expect("tool name"))
            .collect::<Vec<_>>(),
        ["read"]
    );
    let round_two_output = requests[1]["input"]
        .as_array()
        .expect("round two input")
        .iter()
        .find(|item| item["type"] == "function_call_output")
        .and_then(|item| item["output"].as_str())
        .expect("round two function_call_output");
    assert_eq!(round_two_output, EXPECTED_OUTPUT);

    let response = app
        .oneshot(auth_request(
            "GET",
            &format!("/api/sessions/{session_id}/messages?limit=50"),
            &token,
            None,
        ))
        .await
        .expect("fetch persisted messages");
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("bounded messages body");
    let messages = serde_json::from_slice::<Value>(&body).expect("messages JSON");
    let messages = messages["messages"].as_array().expect("messages array");
    assert_eq!(
        messages
            .iter()
            .filter_map(|message| message["role"].as_str())
            .collect::<Vec<_>>(),
        ["user", "tool", "assistant"]
    );
    assert_eq!(
        messages
            .iter()
            .find(|message| message["role"] == "tool")
            .expect("persisted tool message")["body"]["text"],
        format!("[read] {EXPECTED_OUTPUT}")
    );
    assert_eq!(
        messages
            .iter()
            .find(|message| message["role"] == "assistant")
            .expect("persisted assistant message")["body"]["text"],
        FINAL_TEXT
    );
}
