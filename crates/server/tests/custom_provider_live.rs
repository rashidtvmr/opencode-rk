//! CUSTOM-PROVIDER-LIVE-W1 — RED: a custom (non-openai) provider turns through
//! the real HTTP API against a loopback OpenAI-compatible fixture.
//!
//! # Wiring gap (documented, not closable from a test-only file)
//!
//! `create_turn` (`crates/server/src/lib.rs:920-924`) hard-rejects every
//! provider id other than `"openai"`:
//!
//! ```text
//! if provider_id != "openai" {
//!     return Err(ApiFailure::bad_request(format!(
//!         "provider '{provider_id}' does not have a native turn adapter yet"
//!     )));
//! }
//! ```
//!
//! `OpenAiResponsesClient::from_env()` (`crates/providers/src/responses.rs:455-456`)
//! hardcodes `ProviderConfig::from_env("openai")`, so even after the guard is
//! removed the client would resolve `OPENAI_BASE_URL` / `OPENAI_API_KEY`
//! instead of `FREE_BASE_URL` / `FREE_API_KEY`.
//!
//! These tests assert the *target* contract and are expected to be RED on the
//! base revision: scenario 1 currently returns 400 before any socket is opened,
//! so the fixture records zero upstream requests.
//!
//! # Scenarios
//!
//! 1. `free/glm-5.3-flash` reaches the adapter, returns 201 with a non-empty
//!    assistant message, persists both messages, and hits the fixture once.
//! 2. A 402 from upstream produces exactly one request — no replay — and never
//!    persists an assistant message.
//! 3. An absent `FREE_API_KEY` fails closed with 503 and zero upstream
//!    requests, without leaking credential material.
//!
//! The three scenarios mutate process-global environment variables, so they
//! are serialized through [`ENV_LOCK`] rather than allowed to race.
#![forbid(unsafe_code)]

use std::{
    env,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
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

/// The custom `provider/model` reference under test. The provider half (`free`)
/// is deliberately *not* `openai`.
const CUSTOM_MODEL: &str = "free/glm-5.3-flash";

/// Serializes the env-sensitive scenarios so `FREE_*` mutations never race.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Hard cap on one fixture request body.
const MAX_FIXTURE_REQUEST_BYTES: usize = 256 * 1024;

// ---------------------------------------------------------------------------
// Server fixture (pattern from session_turn_api.rs)
// ---------------------------------------------------------------------------

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
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
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

async fn post_turn(app: &axum::Router, session_id: &str, model: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{session_id}/turns"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "text": "hello custom provider",
                        "model": model,
                        "reasoning_effort": "high"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("turn request");
    let status = response.status();
    (status, json_body(response).await)
}

async fn list_messages(app: &axum::Router, session_id: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{session_id}/messages?limit=50"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("list messages");
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}

// ---------------------------------------------------------------------------
// Environment guard (pattern from session_turn_api.rs)
// ---------------------------------------------------------------------------

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
        match self.previous.as_ref() {
            Some(previous) => env::set_var(self.key, previous),
            None => env::remove_var(self.key),
        }
    }
}

// ---------------------------------------------------------------------------
// Loopback OpenAI-compatible provider fixture
// ---------------------------------------------------------------------------

/// One scripted upstream response.
struct FakeResponse {
    status: u16,
    reason: &'static str,
    content_type: &'static str,
    body: String,
}

/// One captured upstream request.
struct RecordedRequest {
    request_line: String,
    headers: String,
    body: Vec<u8>,
}

/// A bounded loopback provider. It accepts at most `responses.len()` requests
/// and never blocks longer than `accept_window`, so a RED scenario that opens
/// no socket still returns promptly.
struct FakeProvider {
    base_url: String,
    handle: thread::JoinHandle<Vec<RecordedRequest>>,
}

impl FakeProvider {
    fn finish(self) -> Vec<RecordedRequest> {
        self.handle.join().expect("fake provider thread")
    }
}

fn ok_completion_response(text: &str) -> FakeResponse {
    FakeResponse {
        status: 200,
        reason: "OK",
        content_type: "application/json",
        body: json!({
            "id": "resp_custom_fixture",
            "status": "completed",
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{ "type": "output_text", "text": text }]
            }]
        })
        .to_string(),
    }
}

fn payment_required_response() -> FakeResponse {
    FakeResponse {
        status: 402,
        reason: "Payment Required",
        content_type: "application/json",
        body: json!({ "error": { "message": "insufficient quota" } }).to_string(),
    }
}

fn read_http_request(stream: &mut TcpStream) -> RecordedRequest {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("fixture read timeout");
    let mut raw = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut expected_len: Option<usize> = None;
    loop {
        let read = match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(_) => break,
        };
        raw.extend_from_slice(&buffer[..read]);
        assert!(
            raw.len() <= MAX_FIXTURE_REQUEST_BYTES,
            "fixture request exceeded bound"
        );
        if expected_len.is_none() {
            if let Some(header_end) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&raw[..header_end]);
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
        if expected_len.is_some_and(|len| raw.len() >= len) {
            break;
        }
    }

    let (head, body) = match raw.windows(4).position(|window| window == b"\r\n\r\n") {
        Some(position) => (raw[..position].to_vec(), raw[position + 4..].to_vec()),
        None => (raw.clone(), Vec::new()),
    };
    let head = String::from_utf8_lossy(&head).into_owned();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default().to_owned();
    let headers = lines.collect::<Vec<_>>().join("\n");
    RecordedRequest {
        request_line,
        headers,
        body,
    }
}

fn write_response(stream: &mut TcpStream, response: &FakeResponse) {
    let wire = format!(
        "HTTP/1.1 {} {}\r\ncontent-type: {}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        response.status,
        response.reason,
        response.content_type,
        response.body.len(),
        response.body
    );
    let _ = stream.write_all(wire.as_bytes());
    let _ = stream.flush();
    let _ = stream.shutdown(std::net::Shutdown::Write);
}

fn spawn_fake_provider(responses: Vec<FakeResponse>, accept_window: Duration) -> FakeProvider {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake provider");
    let address = listener.local_addr().expect("fake provider address");
    listener
        .set_nonblocking(true)
        .expect("non-blocking fake provider");
    let expected = responses.len();
    let handle = thread::spawn(move || {
        let deadline = Instant::now() + accept_window;
        let mut recorded: Vec<RecordedRequest> = Vec::new();
        while recorded.len() < expected && Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let request = read_http_request(&mut stream);
                    recorded.push(request);
                    let index = recorded.len() - 1;
                    let response = &responses[index.min(responses.len() - 1)];
                    write_response(&mut stream, response);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("fake provider accept failed: {error}"),
            }
        }
        recorded
    });
    FakeProvider {
        base_url: format!("http://{address}/v1"),
        handle,
    }
}

// ---------------------------------------------------------------------------
// Scenario 1: custom provider reaches the adapter and persists
// ---------------------------------------------------------------------------

#[test]
fn custom_provider_free_model_reaches_adapter_and_persists() {
    let _serial = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let provider = spawn_fake_provider(
        vec![ok_completion_response("Custom provider live reply")],
        Duration::from_secs(3),
    );
    let _base = EnvGuard::set("FREE_BASE_URL", &provider.base_url);
    let _key = EnvGuard::set("FREE_API_KEY", "free-fixture-secret");
    let _key_env = EnvGuard::set("FREE_API_KEY_ENV", "FREE_API_KEY");
    let _timeout = EnvGuard::set("FREE_TIMEOUT_SECS", "5");

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let (status, turn, messages) = runtime.block_on(async {
        let (app, _dir) = build_app();
        let session_id = create_session(&app, "custom provider live").await;
        let (status, turn) = post_turn(&app, &session_id, CUSTOM_MODEL).await;
        let messages = list_messages(&app, &session_id).await;
        (status, turn, messages)
    });

    let recorded = provider.finish();
    assert_eq!(
        status,
        StatusCode::CREATED,
        "custom provider turn must reach the adapter (upstream requests seen: {}); body: {turn}",
        recorded.len()
    );
    assert_eq!(turn["user_message"]["role"], "user");
    assert_eq!(turn["assistant_message"]["role"], "assistant");
    let assistant = turn["assistant_message"]["body"]["text"]
        .as_str()
        .expect("assistant text");
    assert!(
        !assistant.trim().is_empty(),
        "assistant message must not be empty"
    );
    assert_eq!(assistant, "Custom provider live reply");
    assert!(
        !turn.to_string().contains("free-fixture-secret"),
        "response must not echo the credential: {turn}"
    );

    let persisted = messages["messages"].as_array().expect("messages array");
    assert_eq!(persisted.len(), 2, "user + assistant persisted");
    assert_eq!(persisted[0]["role"], "user");
    assert_eq!(persisted[1]["role"], "assistant");
    assert_eq!(persisted[1]["body"]["text"], "Custom provider live reply");

    assert_eq!(recorded.len(), 1, "exactly one upstream request");
    let request = &recorded[0];
    assert_eq!(request.request_line, "POST /v1/responses HTTP/1.1");
    assert!(
        request
            .headers
            .to_ascii_lowercase()
            .contains("authorization: bearer free-fixture-secret"),
        "fixture must receive the FREE_API_KEY credential; headers: {}",
        request.headers
    );
    let body: Value = serde_json::from_slice(&request.body).expect("provider json");
    assert_eq!(
        body["model"], "glm-5.3-flash",
        "model id excludes the provider prefix"
    );
    assert_eq!(body["input"][0]["role"], "user");
    assert_eq!(body["input"][0]["content"], "hello custom provider");
}

// ---------------------------------------------------------------------------
// Scenario 2: upstream 402 is attempted exactly once, no replay
// ---------------------------------------------------------------------------

#[test]
fn custom_provider_upstream_402_is_single_request_without_replay() {
    let _serial = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let provider = spawn_fake_provider(vec![payment_required_response()], Duration::from_secs(3));
    let _base = EnvGuard::set("FREE_BASE_URL", &provider.base_url);
    let _key = EnvGuard::set("FREE_API_KEY", "free-fixture-secret");
    let _key_env = EnvGuard::set("FREE_API_KEY_ENV", "FREE_API_KEY");
    let _timeout = EnvGuard::set("FREE_TIMEOUT_SECS", "5");

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let (status, turn, messages) = runtime.block_on(async {
        let (app, _dir) = build_app();
        let session_id = create_session(&app, "custom provider 402").await;
        let (status, turn) = post_turn(&app, &session_id, CUSTOM_MODEL).await;
        let messages = list_messages(&app, &session_id).await;
        (status, turn, messages)
    });

    let recorded = provider.finish();
    assert_eq!(
        recorded.len(),
        1,
        "a 402 from upstream must be attempted exactly once (no replay); saw {}",
        recorded.len()
    );
    assert_ne!(
        status,
        StatusCode::BAD_REQUEST,
        "the hardcoded non-openai rejection must be gone: {turn}"
    );
    assert_ne!(
        status,
        StatusCode::CREATED,
        "an upstream 402 must not surface as success: {turn}"
    );
    assert!(
        !status.is_success(),
        "an upstream 402 must surface as an error status, got {status}"
    );

    let persisted = messages["messages"].as_array().expect("messages array");
    assert!(
        !persisted.iter().any(|message| message["role"] == "assistant"),
        "a 402 must never persist an assistant message: {messages}"
    );
    assert!(
        !turn.to_string().contains("free-fixture-secret"),
        "response must not echo the credential: {turn}"
    );
}

// ---------------------------------------------------------------------------
// Scenario 3: absent credential fails closed, zero upstream, no leak
// ---------------------------------------------------------------------------

#[test]
fn custom_provider_absent_credential_fails_closed_without_upstream() {
    let _serial = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let provider = spawn_fake_provider(
        vec![ok_completion_response("must never be reached")],
        Duration::from_secs(1),
    );
    let _base = EnvGuard::set("FREE_BASE_URL", &provider.base_url);
    let _key = EnvGuard::unset("FREE_API_KEY");
    let _key_env = EnvGuard::set("FREE_API_KEY_ENV", "FREE_API_KEY");
    let _timeout = EnvGuard::set("FREE_TIMEOUT_SECS", "5");

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let (status, turn, messages) = runtime.block_on(async {
        let (app, _dir) = build_app();
        let session_id = create_session(&app, "custom provider no credential").await;
        let (status, turn) = post_turn(&app, &session_id, CUSTOM_MODEL).await;
        let messages = list_messages(&app, &session_id).await;
        (status, turn, messages)
    });

    let recorded = provider.finish();
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "an absent credential must fail closed as service-unavailable; body: {turn}"
    );
    assert!(
        recorded.is_empty(),
        "no upstream request may be attempted without a credential; saw {}",
        recorded.len()
    );
    let persisted = messages["messages"].as_array().expect("messages array");
    assert!(
        !persisted.iter().any(|message| message["role"] == "assistant"),
        "no assistant message may persist without a credential: {messages}"
    );
    assert!(
        !turn.to_string().contains("free-fixture-secret"),
        "response must not leak credential material: {turn}"
    );
}