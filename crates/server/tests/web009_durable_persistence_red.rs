//! WEB-009 RED: real authenticated turn data survives a daemon restart.
//!
//! The public contract exercised here is one assistant activity record containing
//! the provider reasoning summary, tool lifecycle identity/state, and answer
//! references. The current web adapter has no native reference event/persistence
//! seam, so this test must fail until that producer and durable projection exist.
#![forbid(unsafe_code)]

use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{
    daemon_auth::DaemonAuth,
    router_with_auth,
    AppState,
};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{Storage, StoragePaths};
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn app(dir: &TempDir, token: &str) -> axum::Router {
    let storage = Storage::open(StoragePaths::under(dir.path())).expect("storage fixture");
    let sessions = SessionService::new(Arc::new(storage));
    let auth = DaemonAuth::from_published(token).expect("fixture auth token");
    router_with_auth(
        AppState {
            sessions,
            catalog: Arc::new(Catalog::default()),
        },
        Some(auth),
    )
}

struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl AsRef<str>) -> Self {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value.as_ref());
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_ref() {
            std::env::set_var(self.key, previous);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

fn read_request(stream: &mut std::net::TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut expected_len = None;
    loop {
        let read = stream.read(&mut buffer).expect("read provider request");
        assert!(read > 0, "provider request ended early");
        request.extend_from_slice(&buffer[..read]);
        if expected_len.is_none() {
            if let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
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
        if expected_len.is_some_and(|length| request.len() >= length) {
            return request;
        }
        assert!(request.len() <= 256 * 1024, "provider request exceeded fixture bound");
    }
}

fn respond_sse(stream: &mut std::net::TcpStream, events: &[String]) {
    stream
        .write_all(
            b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
        )
        .expect("write provider headers");
    for event in events {
        stream.write_all(event.as_bytes()).expect("write provider event");
    }
    stream.flush().expect("flush provider events");
}

fn accept_with_deadline(listener: &TcpListener) -> (std::net::TcpStream, std::net::SocketAddr) {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        match listener.accept() {
            Ok(connection) => return connection,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "provider fixture accept timed out");
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("accept provider request: {error}"),
        }
    }
}

fn spawn_provider(rounds: Vec<Vec<String>>) -> (String, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind provider fixture");
    listener
        .set_nonblocking(true)
        .expect("nonblocking provider fixture");
    let address = listener.local_addr().expect("provider fixture address");
    let task = thread::spawn(move || {
        let mut bodies = Vec::with_capacity(rounds.len());
        for events in rounds {
            let (mut stream, _) = accept_with_deadline(&listener);
            stream.set_nonblocking(false).expect("blocking provider stream");
            let request = read_request(&mut stream);
            let header_end = request
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .expect("provider request headers");
            bodies.push(
                serde_json::from_slice(&request[header_end + 4..]).expect("provider request JSON"),
            );
            respond_sse(&mut stream, &events);
        }
        bodies
    });
    (format!("http://{address}/v1"), task)
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 512 * 1024)
        .await
        .expect("bounded response body");
    serde_json::from_slice(&bytes).expect("JSON response")
}

fn decode_ndjson(bytes: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("NDJSON event"))
        .collect()
}

async fn create_session(app: &axum::Router, token: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"title":"WEB-009 durable"}).to_string()))
                .expect("create request"),
        )
        .await
        .expect("create response");
    assert_eq!(response.status(), StatusCode::CREATED);
    json_body(response).await["session"]["id"]
        .as_str()
        .expect("session id")
        .to_owned()
}

async fn stream_turn(app: &axum::Router, session_id: &str, token: &str) -> Vec<Value> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{session_id}/turns/stream"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "text":"Run the durable fixture",
                        "model":"openai/gpt-5.6",
                        "reasoning_effort":"high"
                    })
                    .to_string(),
                ))
                .expect("turn request"),
        )
        .await
        .expect("turn response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let bytes = to_bytes(response.into_body(), 512 * 1024)
        .await
        .expect("bounded turn stream");
    decode_ndjson(&bytes)
}

async fn get_activity(app: &axum::Router, session_id: &str, token: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{session_id}/activity?limit=50"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("activity request"),
        )
        .await
        .expect("activity response");
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}

#[tokio::test]
async fn web009_t05_tool_and_reference_fidelity_survives_restart() {
    let _env_lock = env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    const ROUND_ONE: &[&str] = &[
        "event: response.reasoning_summary_text.delta\ndata: {\"type\":\"response.reasoning_summary_text.delta\",\"delta\":\"Checked the tool plan.\"}\n\n",
        "event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"id\":\"fc_durable_1\",\"call_id\":\"call_durable_1\",\"name\":\"bash\",\"arguments\":\"{\\\"command\\\":\\\"echo durable-tool-red\\\"}\"}}\n\n",
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_durable_1\",\"status\":\"completed\"}}\n\n",
    ];
    const ROUND_TWO: &[&str] = &[
        "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"Durable answer\"}\n\n",
        "event: response.output_text.annotation.added\ndata: {\"type\":\"response.output_text.annotation.added\",\"annotation_index\":0,\"annotation\":{\"type\":\"url_citation\",\"url\":\"https://example.test/source\",\"title\":\"Example source\"}}\n\n",
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_durable_2\",\"status\":\"completed\"}}\n\n",
    ];
    let rounds: Vec<Vec<String>> = vec![
        ROUND_ONE.iter().map(|event| (*event).to_owned()).collect(),
        ROUND_TWO.iter().map(|event| (*event).to_owned()).collect(),
    ];
    let (provider_base, provider_task) = spawn_provider(rounds);
    let _base_url = EnvGuard::set("OPENAI_BASE_URL", provider_base);
    let _api_key = EnvGuard::set("OPENAI_API_KEY", "fixture-secret");
    let _tools = EnvGuard::set("OPENCODE_RK_TURN_TOOLS", "bash");

    let dir = tempfile::tempdir().expect("temporary durable fixture");
    let auth = DaemonAuth::mint().expect("fixture auth");
    let token = auth.token().to_owned();
    let initial_app = app(&dir, &token);
    let session_id = create_session(&initial_app, &token).await;
    let events = stream_turn(&initial_app, &session_id, &token).await;
    provider_task.join().expect("provider fixture completed");

    let kinds: Vec<&str> = events
        .iter()
        .filter_map(|event| event["type"].as_str())
        .collect();
    assert!(kinds.contains(&"reasoning_summary_delta"), "missing reasoning event: {kinds:?}");
    assert!(kinds.contains(&"tool_call"), "missing tool call event: {kinds:?}");
    assert!(kinds.contains(&"tool_output"), "missing tool output event: {kinds:?}");
    let final_event = events
        .iter()
        .find(|event| event["type"] == "assistant_message")
        .unwrap_or_else(|| panic!("final assistant event with references; events: {events:?}"));
    let assistant_id = final_event["message"]["id"].as_str().expect("assistant id");
    assert_eq!(final_event["references"], json!([{
        "label":"Example source",
        "url":"https://example.test/source"
    }]));

    drop(initial_app);
    let restarted = app(&dir, &token);
    let activity = get_activity(&restarted, &session_id, &token).await;
    assert_eq!(
        activity["activity"],
        json!([{
            "message_id": assistant_id,
            "reasoning_summary":"Checked the tool plan.",
            "tool_calls":[{
                "call_id":"call_durable_1",
                "name":"bash",
                "state":"completed",
                "ok":true
            }],
            "references":[{
                "label":"Example source",
                "url":"https://example.test/source"
            }]
        }])
    );
}
