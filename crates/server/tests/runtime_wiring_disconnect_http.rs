//! RED lane: a browser disconnect cancels the live HTTP turn and its tool tree.
//!
//! The fixture uses the real TCP endpoint, provider adapter, RuntimeWiring
//! permit, ToolExecutor and session persistence. It intentionally freezes the
//! missing cancellation behavior: the current stream owns a request-local
//! ToolExecutor whose child process survives response-body drop.
#![forbid(unsafe_code)]

use std::{
    env,
    fs,
    io::{ErrorKind, Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    path::Path,
    process::Command,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Extension,
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::app_runtime::EnginePolicy;
use opencode_rk_server::runtime_wiring::RuntimeWiring;
use opencode_rk_server::{router_with_auth, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use opencode_rk_tools::registry::ToolRegistry;
use serde_json::{json, Value};
use tempfile::tempdir;
use tokio::time::sleep;
use tower::ServiceExt;

const POLL: Duration = Duration::from_millis(20);
const DEADLINE: Duration = Duration::from_secs(4);

fn shell_quote(path: &Path) -> String {
    let value = path.to_str().expect("fixture path is utf-8");
    assert!(!value.contains('\0'), "fixture path contains NUL");
    assert!(value.len() <= 4096, "fixture path exceeds command bound");
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn disconnect_fixture_command(parent_pid: &Path, descendant_pid: &Path, sentinel: &Path) -> String {
    format!(
        "printf '%s\\n' \"$$\" > {}; sleep 5 & child=$!; printf '%s\\n' \"$child\" > {}; wait \"$child\"; printf '%s\\n' delayed-sentinel > {}",
        shell_quote(parent_pid),
        shell_quote(descendant_pid),
        shell_quote(sentinel),
    )
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

async fn create_session(app: &axum::Router) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "title": "disconnect fixture" }).to_string()))
                .expect("session request"),
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
                .expect("messages request"),
        )
        .await
        .expect("fetch messages");
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await["messages"]
        .as_array()
        .expect("messages array")
        .clone()
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
        assert!(request.len() <= 256 * 1024, "provider request exceeded fixture bound");
    }
    request
}

struct ProviderObservation {
    request: Value,
    connection_closed: bool,
}

fn spawn_provider(command: String) -> (String, thread::JoinHandle<ProviderObservation>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback provider");
    let address = listener.local_addr().expect("provider address");
    let task = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept provider request");
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("provider read timeout");
        let request = read_request(&mut stream);
        let header_end = request
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("provider request headers");
        let body: Value = serde_json::from_slice(&request[header_end + 4..])
            .expect("provider request json");
        let arguments = json!({ "command": command }).to_string();
        let function_call = json!({
            "type": "response.output_item.done",
            "item": {
                "type": "function_call",
                "id": "fc_disconnect_1",
                "call_id": "call_disconnect_1",
                "name": "bash",
                "arguments": arguments,
            }
        });
        let completed = json!({
            "type": "response.completed",
            "response": { "id": "resp_disconnect_1", "status": "completed" }
        });
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\r\nevent: response.output_item.done\ndata: {function_call}\n\nevent: response.completed\ndata: {completed}\n\n"
                )
                .as_bytes(),
            )
            .expect("write provider stream");
        stream.flush().expect("flush provider stream");

        let deadline = Instant::now() + DEADLINE;
        let mut byte = [0_u8; 1];
        let connection_closed = loop {
            match stream.read(&mut byte) {
                Ok(0) => break true,
                Ok(_) => break false,
                Err(error)
                    if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock)
                        && Instant::now() < deadline =>
                {
                    continue;
                }
                Err(_) => break false,
            }
        };
        ProviderObservation {
            request: body,
            connection_closed,
        }
    });
    (format!("http://{address}/v1"), task)
}

async fn spawn_http(app: axum::Router) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind server");
    let address = listener.local_addr().expect("server address");
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (address, task)
}

fn stream_until_tool_call(
    address: SocketAddr,
    session_id: String,
    parent_pid: &Path,
    descendant_pid: &Path,
) -> (Vec<u8>, (u32, u32)) {
    let mut stream = TcpStream::connect(address).expect("connect server");
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .expect("client read timeout");
    let body = json!({
        "text": "start the disconnect fixture",
        "model": "openai/gpt-5.6",
        "reasoning_effort": "high"
    })
    .to_string();
    let request = format!(
        "POST /api/sessions/{session_id}/turns/stream HTTP/1.1\r\nhost: localhost\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).expect("write turn request");
    stream.flush().expect("flush turn request");

    let marker = b"\"type\":\"tool_call\"";
    let deadline = Instant::now() + DEADLINE;
    let mut response = Vec::new();
    let mut buffer = [0_u8; 4096];
    while !response.windows(marker.len()).any(|window| window == marker) {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => response.extend_from_slice(&buffer[..read]),
            Err(error)
                if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock)
                    && Instant::now() < deadline =>
            {
                continue;
            }
            Err(error) => panic!("read turn response: {error}"),
        }
        assert!(Instant::now() < deadline, "tool_call event deadline exceeded");
        assert!(response.len() <= 256 * 1024, "turn response exceeded fixture bound");
    }
    assert!(
        response.windows(marker.len()).any(|window| window == marker),
        "real HTTP response omitted tool_call event"
    );
    let pids = wait_for_pids(parent_pid, descendant_pid);
    stream
        .shutdown(Shutdown::Both)
        .expect("disconnect browser-like client");
    (response, pids)
}

fn read_pid(path: &Path) -> Option<u32> {
    let text = fs::read_to_string(path).ok()?;
    let pid = text.trim().parse().ok()?;
    (pid > 1).then_some(pid)
}

fn wait_for_pids(parent_pid: &Path, descendant_pid: &Path) -> (u32, u32) {
    let deadline = Instant::now() + DEADLINE;
    loop {
        if let (Some(parent), Some(descendant)) = (read_pid(parent_pid), read_pid(descendant_pid)) {
            return (parent, descendant);
        }
        assert!(Instant::now() < deadline, "tool fixture PID publication deadline exceeded");
        thread::sleep(POLL);
    }
}

fn process_is_live(pid: u32) -> bool {
    let pid = pid.to_string();
    let alive = Command::new("/bin/kill")
        .args(["-0", &pid])
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if !alive {
        return false;
    }
    Command::new("/bin/ps")
        .args(["-p", &pid, "-o", "stat="])
        .output()
        .map(|output| {
            let stat = String::from_utf8_lossy(&output.stdout);
            !stat.trim_start().starts_with('Z')
        })
        .unwrap_or(false)
}

fn wait_for_fixture_exit(parent: u32, descendant: u32) {
    let deadline = Instant::now() + DEADLINE;
    while (process_is_live(parent) || process_is_live(descendant)) && Instant::now() < deadline {
        thread::sleep(POLL);
    }
    assert!(!process_is_live(parent), "tool parent survived browser disconnect");
    assert!(!process_is_live(descendant), "tool descendant survived browser disconnect");
}

fn terminate_recorded_pid(pid: u32) {
    if pid > 1 && process_is_live(pid) {
        let pid_text = pid.to_string();
        let _ = Command::new("/bin/kill")
            .args(["-TERM", &pid_text])
            .status();
        let deadline = Instant::now() + Duration::from_millis(500);
        while process_is_live(pid) && Instant::now() < deadline {
            thread::sleep(POLL);
        }
        if process_is_live(pid) {
            let _ = Command::new("/bin/kill")
                .args(["-KILL", &pid_text])
                .status();
        }
    }
}

struct FixtureProcesses {
    parent: u32,
    descendant: u32,
}

impl Drop for FixtureProcesses {
    fn drop(&mut self) {
        terminate_recorded_pid(self.parent);
        terminate_recorded_pid(self.descendant);
    }
}

async fn wait_for_permits(runtime: &RuntimeWiring) {
    let deadline = Instant::now() + DEADLINE;
    while runtime.available_permits() != 2 && Instant::now() < deadline {
        sleep(POLL).await;
    }
    assert_eq!(runtime.available_permits(), 2, "turn permit leaked after disconnect");
}

#[tokio::test]
async fn disconnect_cancels_provider_tool_tree_and_durable_turn() {
    let _api_key = EnvGuard::set("OPENAI_API_KEY", "fixture-secret");
    let _tools = EnvGuard::set("OPENCODE_RK_TURN_TOOLS", "bash");
    let fixture = tempdir().expect("tool fixture directory");
    let parent_pid = fixture.path().join("parent.pid");
    let descendant_pid = fixture.path().join("descendant.pid");
    let sentinel = fixture.path().join("delayed-sentinel");
    let command = disconnect_fixture_command(&parent_pid, &descendant_pid, &sentinel);
    let (provider_base, provider_task) = spawn_provider(command);
    let _base_url = EnvGuard::set("OPENAI_BASE_URL", provider_base);

    let (app, _storage_dir, runtime) = build_app();
    assert_eq!(runtime.available_permits(), 2);
    let session_id = create_session(&app).await;
    let (address, server) = spawn_http(app.clone()).await;
    let client_session = session_id.clone();
    let client_parent_pid = parent_pid.clone();
    let client_descendant_pid = descendant_pid.clone();
    let client = tokio::task::spawn_blocking(move || {
        stream_until_tool_call(
            address,
            client_session,
            &client_parent_pid,
            &client_descendant_pid,
        )
    });
    let (response, (parent, descendant)) = client.await.expect("HTTP client task");
    let _fixture_processes = FixtureProcesses { parent, descendant };
    let tool_call_marker = b"\"type\":\"tool_call\"";
    assert!(response
        .windows(tool_call_marker.len())
        .any(|window| window == tool_call_marker));

    wait_for_permits(&runtime).await;
    let provider = provider_task.join().expect("provider fixture task");
    assert!(provider.connection_closed, "provider stream remained live after disconnect");
    assert!(provider.request["tools"].as_array().is_some_and(|tools| {
        tools.iter().any(|tool| tool["name"] == "bash")
    }));

    wait_for_fixture_exit(parent, descendant);
    assert!(!sentinel.exists(), "delayed tool sentinel proves execution continued");

    let messages = fetch_messages(&app, &session_id).await;
    let roles: Vec<_> = messages
        .iter()
        .filter_map(|message| message["role"].as_str())
        .collect();
    assert_eq!(roles, ["user"], "disconnect must not fabricate tool/assistant success");
    assert!(messages.iter().all(|message| message["role"] != "tool"));
    assert!(messages.iter().all(|message| message["role"] != "assistant"));
    server.abort();
}
