//! APP-012 RED: exercise allowed file write from the live streaming route,
//! including provider feedback and durable transcript honesty.
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

/// Round one asks the live turn adapter to write a secret-bearing file.
fn round_one_events(path: &std::path::Path) -> Vec<String> {
    let args = serde_json::to_string(&json!({
        "path": path.to_string_lossy(),
        "content": "allowed fixture content"
    })).expect("write arguments");
    let item = json!({
        "type": "response.output_item.done",
        "item": {
            "type": "function_call",
            "id": "fc_write_1",
            "call_id": "call_write_fixture_1",
            "name": "write",
            "arguments": args
        }
    });
    vec![
        format!("event: response.output_item.done\ndata: {}\n\n", item),
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_write_r1\",\"status\":\"completed\"}}\n\n".to_owned(),
    ]
}

fn round_two_events() -> Vec<String> {
    vec![
        "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"Write succeeded\"}\n\n".to_owned(),
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_write_r2\",\"status\":\"completed\"}}\n\n".to_owned(),
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
async fn live_write_tool_is_allowed_and_persisted() {
    let _api_key = EnvGuard::set("OPENAI_API_KEY", "fixture-secret");
    let _tools = EnvGuard::set("OPENCODE_RK_TURN_TOOLS", "write");
    let (app, dir) = build_app();
    let target = dir.path().join("allowed.txt");
    let (provider_base, provider_task) = spawn_scripted_provider(vec![
        round_one_events(&target),
        round_two_events(),
    ]);
    let _base = EnvGuard::set("OPENAI_BASE_URL", &provider_base);
    let session_id = create_session(&app, "live write denial").await;
    let (address, server) = spawn_http(app.clone()).await;
    let stream_session = session_id.clone();
    let response = tokio::task::spawn_blocking(move || stream_turn(address, stream_session))
        .await
        .expect("stream task");
    server.abort();

    let events = ndjson_events(&response);
    let result = events
        .iter()
        .find(|event| event["type"] == "tool_output")
        .expect("write call reports tool output");
    let output = result["output"].as_str().unwrap_or_default();
    assert!(output.contains("success") || output.contains("written"), "expected successful write, got {output:?}");
    assert!(!output.contains("Unknown tool"), "live caller must dispatch write tool");
    assert_eq!(std::fs::read_to_string(&target).expect("written file"), "allowed fixture content");
    assert!(!events.iter().any(|event| event.to_string().contains("allowed fixture content")), "public tool_call must not leak file content");
    let bodies = provider_task.join().expect("provider fixture finished");
    assert_eq!(bodies.len(), 2, "denial is acknowledged in exactly one bounded second round");
    let round2 = bodies[1]["input"].as_array().expect("round two input");
    let feedback = round2
        .iter()
        .find(|item| item["type"] == "function_call_output")
        .expect("denial fed back to provider");
    let feedback_text = feedback["output"].as_str().unwrap_or_default();
    assert!(feedback_text.contains("success") || feedback_text.contains("written"), "provider feedback must truthfully report success: {feedback_text:?}");
    let messages = fetch_messages(&app, &session_id).await;
    assert!(messages.iter().any(|message| message.to_string().contains("success") || message.to_string().contains("written")), "persisted tool transcript must record success");
    assert!(!messages.iter().any(|message| message.to_string().contains("allowed fixture content")), "transcript must not include file payload");
}
