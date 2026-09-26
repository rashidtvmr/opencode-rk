//! APP-012-TOOL-INTENT RED: live provider/HTTP tool dispatch must enforce the
//! real broker's denial with zero side effects, even when an ordinary
//! permission wildcard exists.
//!
//! Contract (frozen once RED is observed):
//! - T1 (broker guard): a real `PermissionBroker` holding ordinary wildcard
//!   `*` Allow still denies file-backed tool effects against secret paths
//!   (`File::Write` to `*/.env`) and destructive argv (`rm -rf *`), while an
//!   ordinary project write still Allows. Deny-first explicit rules also deny.
//! - T2 (fake-grant guard): a manufactured grant from a fake grant issuer that
//!   is stale (wrong policy version) cannot release a denied tool effect;
//!   `run_if_allowed` then runs zero effects.
//! - T3 (LIVE RED): a scripted live provider requests the `write` tool with
//!   file-backed args targeting a disposable secret path (`<tmpdir>/.env`)
//!   over the real HTTP turn endpoint (`POST
//!   /api/sessions/{id}/turns/stream`, plain router, `OPENCODE_RK_TURN_TOOLS`
//!   ordinary enablement). The turn must report a broker denial in
//!   `tool_output`, persist the denial (not a success), complete via round
//!   two — and the denied write must cause ZERO file side effects (sentinel
//!   absent) and no process result.
//!
//! Pre-GREEN, T3 fails on the denial-wording assertion: live dispatch
//! authorizes only the vacuous generic `OperationIntent::Tool` (baseline
//! Allow, `crates/security/src/lib.rs:243-254`) and never binds file-backed
//! args to `FileAction::Write` through the real broker, so the output is
//! `Unknown tool: write` instead of a broker denial. T1/T2 pin the broker
//! semantics GREEN must enforce live. Compatible with the immutable
//! `agent_loop_turns.rs` E1 (enabled bash `echo` still executes): this lane
//! never gates bash content, only file-backed secret writes.
//!
//! Bounds: provider requests <= 256 KiB, server raw response <= 512 KiB,
//! oneshot bodies <= 64 KiB, 20 s socket read timeout. No real user DB
//! (in-memory Storage + tempdir), no secret access (disposable fixture
//! content, existence-check only), loopback fixtures only. The single
//! env-mutating test runs under `RUST_TEST_THREADS=1`; broker tests touch no
//! process-global state.
//!
//! RED receipt: T3 compiles and fails on the missing broker-denial behavior;
//! T1/T2 pass as contract pins. Freeze the file hash after the RED run.
#![forbid(unsafe_code)]

use std::{
    env,
    ffi::OsString,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    sync::Arc,
    thread,
    time::{Duration, SystemTime},
};

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::{ApprovalId, SessionId};
use opencode_rk_security::{
    app_policy::{ExpectedScope, Grant, Scope},
    tool_authorize::{argv_intent, digest_of, file_write_intent, run_if_allowed, ToolAuthorizer, ToolGate},
    Decision, FileAction, OperationIntent, PermissionBroker, PermissionRule, PermissionSet,
    RuleEffect, SecurityPolicy,
};
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

const MAX_PROVIDER_REQUEST_BYTES: usize = 256 * 1024;
const MAX_SERVER_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_ONESHOT_BYTES: usize = 64 * 1024;
const SOCKET_READ_TIMEOUT: Duration = Duration::from_secs(20);
const FIXTURE_GRANT_TTL_SECS: u64 = 3600;
const FIXTURE_NOW: u64 = 1_750_000_000;

fn fixture_now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(FIXTURE_NOW)
}

fn broker() -> PermissionBroker {
    PermissionBroker::new(SecurityPolicy::lean_default("/work/project"))
}

fn star() -> PermissionBroker {
    broker().with_permissions(PermissionSet::star())
}

fn deny_dotenv_then_star() -> PermissionBroker {
    broker().with_permissions(PermissionSet::new(vec![
        PermissionRule::new("**/.env", RuleEffect::Deny),
        PermissionRule::new("*", RuleEffect::Allow),
    ]))
}

fn file_write(path: &str) -> OperationIntent {
    OperationIntent::File {
        action: FileAction::Write,
        path: PathBuf::from(path),
    }
}

fn is_denial_text(output: &str) -> bool {
    output.contains("denied") || output.contains("not permitted") || output.contains("requires human approval")
}

/// T1: deny beats the ordinary wildcard for file-backed tool effects.
#[test]
fn broker_deny_beats_wildcard_for_tool_effect_intents() {
    // Ordinary wildcard allows an ordinary project write.
    assert_eq!(
        star().authorize(&file_write("/work/project/notes.txt")),
        Decision::Allow
    );
    // ...but the mandatory secret baseline survives `*` with no explicit rule.
    assert!(matches!(
        star().authorize(&file_write("/work/project/.env")),
        Decision::Deny { .. }
    ));
    // Explicit deny-first rules also deny, even with `*` present.
    let gated = deny_dotenv_then_star();
    assert_eq!(
        gated.authorize(&file_write("/work/project/notes.txt")),
        Decision::Allow
    );
    assert!(matches!(
        gated.authorize(&file_write("/work/project/.env")),
        Decision::Deny { .. }
    ));
    // Destructive process effects are denied under `*` too (no process may
    // start for a denied operation).
    let cwd = PathBuf::from("/work/project");
    let denied = star().authorize(&argv_intent("rm", &["-rf".to_owned(), "*".to_owned()], &cwd));
    assert!(matches!(denied, Decision::Deny { .. }));
    let star_broker = star();
    let mut auth = ToolAuthorizer::new(&star_broker);
    let gate = auth.authorize(&argv_intent("rm", &["-rf".to_owned(), "*".to_owned()], &cwd));
    assert!(
        matches!(gate, ToolGate::Deny { .. }),
        "destructive argv must gate Deny, got {gate:?}"
    );
    let mut spawned = 0u32;
    assert_eq!(
        run_if_allowed(&gate, || {
            spawned += 1;
        }),
        None
    );
    assert_eq!(spawned, 0, "denied process effect ran");
    // file_write_intent helper binds the same File::Write the live path must bind.
    assert!(matches!(
        star().authorize(&file_write_intent(&PathBuf::from("/work/project/.env"))),
        Decision::Deny { .. }
    ));
}

/// T2: a stale manufactured grant (fake grant issuer) cannot release a denied
/// tool effect; the denial runs zero effects.
#[test]
fn stale_grant_cannot_release_denied_tool_effect() {
    let base = broker();
    let intent = file_write("/work/project/.env");
    let scope = Scope::new(
        "/work/project",
        SessionId::new(),
        "local-user",
        fixture_now() + Duration::from_secs(FIXTURE_GRANT_TTL_SECS),
        base.generation(),
    )
    .expect("fixture scope");
    let stale = Grant::new(ApprovalId::new(), digest_of(&intent), scope, false);
    // Policy version bump: the grant is now stale.
    let repoliced = base.with_policy(SecurityPolicy::lean_default("/work/project"));
    let expected = ExpectedScope {
        workspace: PathBuf::from("/work/project"),
        session: stale.scope.session,
        requester: "local-user".to_owned(),
        now: fixture_now(),
        policy_version: repoliced.generation(),
    };
    let mut auth = ToolAuthorizer::new(&repoliced);
    let gate = auth.authorize_with_grant(&intent, &stale, &expected);
    assert!(
        matches!(gate, ToolGate::Deny { .. }),
        "stale grant must deny, got {gate:?}"
    );
    let mut effects = 0u32;
    assert_eq!(
        run_if_allowed(&gate, || {
            effects += 1;
        }),
        None
    );
    assert_eq!(effects, 0, "stale-grant denial ran an effect");
}

// ---------- live HTTP harness (same proven shape as agent_loop_turns.rs) ----------

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

    fn remove(key: &'static str) -> Self {
        let previous = env::var_os(key);
        env::remove_var(key);
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
    let bytes = to_bytes(response.into_body(), MAX_ONESHOT_BYTES)
        .await
        .expect("bounded response body");
    serde_json::from_slice(&bytes).expect("json response")
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
        request.len() <= MAX_PROVIDER_REQUEST_BYTES,
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

fn sse_event(name: &str, data: Value) -> String {
    format!("event: {name}\ndata: {data}\n\n")
}

/// Provider round one: request the `write` tool with file-backed args aimed
/// at the disposable secret sentinel. Round two: final answer.
fn round_one_events(secret_path: &str) -> Vec<String> {
    let args = serde_json::to_string(&json!({
        "path": secret_path,
        "content": "fixture-content",
    }))
    .expect("fixture args");
    vec![
        sse_event(
            "response.output_text.delta",
            json!({"type": "response.output_text.delta", "delta": "Writing…"}),
        ),
        sse_event(
            "response.output_item.done",
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "function_call",
                    "id": "fc_write_1",
                    "call_id": "call_write_1",
                    "name": "write",
                    "arguments": args,
                },
            }),
        ),
        sse_event(
            "response.completed",
            json!({"type": "response.completed", "response": {"id": "resp_w1", "status": "completed"}}),
        ),
    ]
}

fn round_two_events() -> Vec<String> {
    vec![
        sse_event(
            "response.output_text.delta",
            json!({"type": "response.output_text.delta", "delta": "Write denied"}),
        ),
        sse_event(
            "response.completed",
            json!({"type": "response.completed", "response": {"id": "resp_w2", "status": "completed"}}),
        ),
    ]
}

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
        .set_read_timeout(Some(SOCKET_READ_TIMEOUT))
        .expect("read timeout");
    let body = json!({
        "text": "write the workspace secret file",
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
        assert!(
            raw.len() <= MAX_SERVER_RESPONSE_BYTES,
            "server response exceeded fixture bound"
        );
    }
    raw
}

fn ndjson_events(response: &[u8]) -> Vec<Value> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("response header terminator");
    let payload = std::str::from_utf8(&response[header_end + 4..]).expect("utf-8 body");
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

/// T3 (LIVE RED): the denied file-backed write is broker-denied on the live
/// path with zero file/process side effects, even under ordinary enablement.
#[tokio::test]
async fn live_denied_file_write_has_zero_side_effects() {
    let _api_key = EnvGuard::set("OPENAI_API_KEY", "fixture-secret");
    let _tools = EnvGuard::set("OPENCODE_RK_TURN_TOOLS", "write");
    let _cap = EnvGuard::remove("OPENCODE_RK_TURN_MAX_STEPS");
    let (app, dir) = build_app();
    // Disposable secret sentinel: file-backed target the real broker denies.
    // Existence-check only; content is an innocuous fixture string.
    let sentinel = dir.path().join(".env");
    assert!(
        !sentinel.exists(),
        "fixture sentinel must start absent"
    );
    let secret_path = sentinel.to_string_lossy().into_owned();

    let (provider_base, provider_task) =
        spawn_scripted_provider(vec![round_one_events(&secret_path), round_two_events()]);
    let _base = EnvGuard::set("OPENAI_BASE_URL", &provider_base);

    let session_id = create_session(&app, "Tool intent live denial").await;
    let (address, server) = spawn_http(app.clone()).await;
    let stream_session = session_id.clone();
    let response = tokio::task::spawn_blocking(move || stream_turn(address, stream_session))
        .await
        .expect("stream task");
    server.abort();

    // The provider was offered the real `write` tool through the live path.
    let bodies = provider_task.join().expect("provider fixture finished");
    assert_eq!(bodies.len(), 2, "denied call still receives completion round");
    let advertised = bodies[0]["tools"]
        .as_array()
        .expect("round one tool schema")
        .iter()
        .find(|tool| tool["name"] == "write")
        .expect("live dispatch advertised the write tool");

    assert_eq!(advertised["name"], "write");

    // The provider's function call is represented on the stream.
    let events = ndjson_events(&response);
    let tool_call = events
        .iter()
        .find(|event| event["type"] == "tool_call")
        .expect("provider function call is represented");
    assert_eq!(tool_call["name"], "write");

    // RED: the tool output must report a broker denial (not "Unknown tool").
    let tool_output = events
        .iter()
        .find(|event| event["type"] == "tool_output")
        .expect("tool result is represented as tool output");
    let output = tool_output["output"].as_str().unwrap_or_default();
    assert!(
        is_denial_text(output),
        "live dispatch must broker-deny the secret write, got: {output}"
    );

    // Round two carries the denial back to the provider (no process result).
    let round2_items = bodies[1]["input"].as_array().expect("round two input");
    let fed_back = round2_items
        .iter()
        .find(|item| item["type"] == "function_call_output")
        .and_then(|item| item["output"].as_str())
        .expect("round two function_call_output");
    assert!(
        is_denial_text(fed_back),
        "denial must be fed back to the provider, got: {fed_back}"
    );

    // The denial is persisted as a tool transcript record (no success text).
    let messages = fetch_messages(&app, &session_id).await;
    let tool = messages
        .iter()
        .find(|message| message["role"] == "tool")
        .expect("tool record persisted");
    let persisted = tool["body"]["text"].as_str().unwrap_or_default();
    assert!(
        is_denial_text(persisted),
        "persisted tool record must carry the denial, got: {persisted}"
    );

    // The turn still completes.
    let final_event = events
        .iter()
        .find(|event| event["type"] == "assistant_message")
        .expect("turn ends with assistant message");
    assert_eq!(final_event["stop_reason"], "completed");

    // ZERO file side effects: the denied write created nothing.
    assert!(
        !sentinel.exists(),
        "denied live tool call left a file side effect"
    );
}
