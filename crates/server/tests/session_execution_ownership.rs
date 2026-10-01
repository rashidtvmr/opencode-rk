//! G4-SESSION-EXECUTION-SERIALIZATION: live transport contract for session ownership.
//!
//! These tests deliberately exercise the authenticated Axum listener, rather than a
//! router service or a turn-service seam.  They are RED contract tests: the current
//! process-wide two-permit implementation permits two provider calls for one ID.
#![forbid(unsafe_code)]

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex, OnceLock,
    },
    thread,
    time::{Duration, Instant},
};

use axum::{body::Body, http::Request};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{daemon_auth::DaemonAuth, router_with_auth, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tempfile::{tempdir, TempDir};
use tower::ServiceExt;

const MAX_WIRE: usize = 128 * 1024;
const MODEL: &str = "openai/gpt-5.6";

static ENV_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

struct EnvGuard {
    old_base: Option<String>,
    old_key: Option<String>,
    old_home: Option<String>,
    old_config: Option<String>,
    old_data: Option<String>,
    old_state: Option<String>,
    old_cache: Option<String>,
    old_rk_home: Option<String>,
    old_auth_json: Option<String>,
    _home: TempDir,
    _config: TempDir,
    _data: TempDir,
    _state: TempDir,
    _cache: TempDir,
}

impl EnvGuard {
    fn install(base: &str) -> Self {
        let home = tempdir().expect("fixture HOME");
        let config = tempdir().expect("fixture config");
        let data = tempdir().expect("fixture data");
        let state = tempdir().expect("fixture state");
        let cache = tempdir().expect("fixture cache");
        let old_base = std::env::var("OPENAI_BASE_URL").ok();
        let old_key = std::env::var("OPENAI_API_KEY").ok();
        let old_home = std::env::var("HOME").ok();
        let old_config = std::env::var("XDG_CONFIG_HOME").ok();
        let old_data = std::env::var("XDG_DATA_HOME").ok();
        let old_state = std::env::var("XDG_STATE_HOME").ok();
        let old_cache = std::env::var("XDG_CACHE_HOME").ok();
        let old_rk_home = std::env::var("OPENCODE_RK_HOME").ok();
        let old_auth_json = std::env::var("OPENCODE_AUTH_CONTENT").ok();
        std::env::set_var("OPENAI_BASE_URL", base);
        std::env::set_var("OPENAI_API_KEY", "fixture-bearer");
        std::env::set_var("HOME", home.path());
        std::env::set_var("XDG_CONFIG_HOME", config.path());
        std::env::set_var("XDG_DATA_HOME", data.path());
        std::env::set_var("XDG_STATE_HOME", state.path());
        std::env::set_var("XDG_CACHE_HOME", cache.path());
        std::env::set_var("OPENCODE_RK_HOME", home.path());
        std::env::remove_var("OPENCODE_AUTH_CONTENT");
        Self {
            old_base,
            old_key,
            old_home,
            old_config,
            old_data,
            old_state,
            old_cache,
            old_rk_home,
            old_auth_json,
            _home: home,
            _config: config,
            _data: data,
            _state: state,
            _cache: cache,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.old_base {
            Some(value) => std::env::set_var("OPENAI_BASE_URL", value),
            None => std::env::remove_var("OPENAI_BASE_URL"),
        }
        match &self.old_key {
            Some(value) => std::env::set_var("OPENAI_API_KEY", value),
            None => std::env::remove_var("OPENAI_API_KEY"),
        }
        for (key, value) in [
            ("HOME", &self.old_home),
            ("XDG_CONFIG_HOME", &self.old_config),
            ("XDG_DATA_HOME", &self.old_data),
            ("XDG_STATE_HOME", &self.old_state),
            ("XDG_CACHE_HOME", &self.old_cache),
            ("OPENCODE_RK_HOME", &self.old_rk_home),
        ] {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
        match &self.old_auth_json {
            Some(value) => std::env::set_var("OPENCODE_AUTH_CONTENT", value),
            None => std::env::remove_var("OPENCODE_AUTH_CONTENT"),
        }
    }
}

#[derive(Default)]
struct Counts {
    accepted: usize,
    active: usize,
    maximum: usize,
    bad_request: Option<String>,
}

struct Provider {
    address: SocketAddr,
    counts: Arc<(Mutex<Counts>, Condvar)>,
    release: Arc<(Mutex<bool>, Condvar)>,
    stopping: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Provider {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("provider bind");
        listener
            .set_nonblocking(true)
            .expect("provider nonblocking");
        let address = listener.local_addr().expect("provider address");
        let counts = Arc::new((Mutex::new(Counts::default()), Condvar::new()));
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let worker_counts = Arc::clone(&counts);
        let worker_release = Arc::clone(&release);
        let stopping = Arc::new(AtomicBool::new(false));
        let worker_stopping = Arc::clone(&stopping);
        let thread = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(8);
            let mut workers = Vec::new();
            while Instant::now() < deadline
                && !worker_stopping.load(Ordering::Acquire)
                && workers.len() < 2
            {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let c = Arc::clone(&worker_counts);
                        let r = Arc::clone(&worker_release);
                        workers.push(thread::spawn(move || provider_request(stream, c, r)));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("provider accept: {error}"),
                }
            }
            let mut failed = false;
            for worker in workers {
                failed |= worker.join().is_err();
            }
            assert!(
                !failed,
                "provider request worker failed after all workers joined"
            );
        });
        Self {
            address,
            counts,
            release,
            stopping,
            thread: Some(thread),
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}/v1", self.address)
    }

    async fn wait_for(&self, n: usize) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if self.counts.0.lock().expect("counts").accepted >= n {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    fn release(&self) {
        let (lock, wake) = &*self.release;
        *lock.lock().expect("release") = true;
        wake.notify_all();
    }

    fn maximum(&self) -> usize {
        self.counts.0.lock().expect("counts").maximum
    }

    fn bad_request(&self) -> Option<String> {
        self.counts.0.lock().expect("counts").bad_request.clone()
    }

    fn accepted(&self) -> usize {
        self.counts.0.lock().expect("counts").accepted
    }
}

impl Drop for Provider {
    fn drop(&mut self) {
        self.release();
        self.stopping.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            if thread.join().is_err() {
                if std::thread::panicking() {
                    eprintln!("provider fixture cleanup failed");
                } else {
                    panic!("provider server thread failed");
                }
            }
        }
    }
}

fn provider_request(
    mut stream: TcpStream,
    counts: Arc<(Mutex<Counts>, Condvar)>,
    release: Arc<(Mutex<bool>, Condvar)>,
) {
    // Accepted sockets may inherit the listener's nonblocking mode on macOS.
    stream.set_nonblocking(false).expect("provider blocking");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("provider read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .expect("provider write timeout");
    let request = read_request(&mut stream).expect("provider request");
    let text = String::from_utf8(request.clone()).expect("provider utf8");
    let (headers, body) = text.split_once("\r\n\r\n").expect("provider body");
    let parsed: Value = serde_json::from_str(body).expect("provider json");
    let valid = headers.lines().next() == Some("POST /v1/responses HTTP/1.1")
        && headers.lines().any(|line| {
            line.split_once(':').is_some_and(|(name, value)| {
                name.eq_ignore_ascii_case("authorization")
                    && value.trim() == "Bearer fixture-bearer"
            })
        })
        && parsed["model"] == "gpt-5.6"
        && parsed["input"].as_array().is_some_and(|items| {
            items.last().is_some_and(|item| {
                item["role"] == "user"
                    && matches!(
                        item["content"].as_str(),
                        Some("a" | "b" | "good" | "denied")
                    )
            })
        });
    let streaming = parsed["stream"] == true;
    let (lock, wake) = &*counts;
    {
        let mut state = lock.lock().expect("counts");
        state.accepted += 1;
        state.active += 1;
        state.maximum = state.maximum.max(state.active);
        if !valid {
            state.bad_request =
                Some("provider method, bearer, model or final user message mismatch".to_owned());
        }
        wake.notify_all();
    }
    if streaming {
        stream
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\nevent: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"fixture\"}\n\n")
            .expect("provider partial response");
        stream.flush().expect("provider partial flush");
    }
    let (release_lock, release_wake) = &*release;
    let mut done = release_lock.lock().expect("release");
    let deadline = Instant::now() + Duration::from_secs(6);
    while !*done && Instant::now() < deadline {
        let (next, _) = release_wake
            .wait_timeout(done, deadline.saturating_duration_since(Instant::now()))
            .expect("release wait");
        done = next;
    }
    if !*done {
        panic!("provider release deadline");
    }
    drop(done);
    let response = if streaming {
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n".to_owned()
    } else {
        let body = json!({"id":"fixture","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"fixture"}]}]}).to_string();
        format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len())
    };
    stream
        .write_all(response.as_bytes())
        .expect("provider response");
    let (lock, _) = &*counts;
    lock.lock().expect("counts").active -= 1;
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buf = [0_u8; 4096];
    let mut total = None;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "provider request deadline",
            ));
        }
        // Partial reads and transient errors must not restart the absolute deadline.
        stream.set_read_timeout(Some(remaining.min(Duration::from_millis(500))))?;
        let n = match stream.read(&mut buf) {
            Ok(n) => n,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) =>
            {
                thread::sleep(
                    deadline
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_millis(2)),
                );
                continue;
            }
            Err(error) => return Err(error),
        };
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "incomplete provider request",
            ));
        }
        if bytes.len() > MAX_WIRE.saturating_sub(n) {
            return Err(std::io::Error::other("wire bound"));
        }
        bytes.extend_from_slice(&buf[..n]);
        if total.is_none() {
            if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .ok_or_else(|| std::io::Error::other("missing content length"))?;
                if length > MAX_WIRE - end - 4 {
                    return Err(std::io::Error::other("provider content-length bound"));
                }
                total = Some(end + 4 + length);
            }
        }
        if bytes.len() > MAX_WIRE {
            return Err(std::io::Error::other("wire bound"));
        }
        if total.is_some_and(|n| bytes.len() >= n) {
            break;
        }
    }
    Ok(bytes)
}

fn read_response(
    stream: &mut TcpStream,
    mut partial: Option<tokio::sync::oneshot::Sender<()>>,
) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let deadline = Instant::now() + Duration::from_secs(8);
    stream.set_read_timeout(Some(Duration::from_millis(250)))?;
    while Instant::now() < deadline {
        match stream.read(&mut buffer) {
            Ok(0) => {
                if bytes.is_empty() {
                    return Err(std::io::Error::other("empty response"));
                }
                return Ok(bytes);
            }
            Ok(n) => {
                if bytes.len() > MAX_WIRE.saturating_sub(n) {
                    return Err(std::io::Error::other("response wire bound"));
                }
                bytes.extend_from_slice(&buffer[..n]);
                if String::from_utf8_lossy(&bytes).contains("assistant_delta") {
                    if let Some(sender) = partial.take() {
                        let _ = sender.send(());
                    }
                }
                if bytes.len() > MAX_WIRE {
                    return Err(std::io::Error::other("response wire bound"));
                }
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue
            }
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "complete response EOF deadline",
    ))
}

fn client(
    address: SocketAddr,
    token: Option<&str>,
    method: &str,
    path: &str,
    body: &Value,
    partial: Option<tokio::sync::oneshot::Sender<()>>,
) -> Vec<u8> {
    let mut stream =
        TcpStream::connect_timeout(&address, Duration::from_secs(2)).expect("client connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(8)))
        .expect("client timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .expect("client write timeout");
    let body = body.to_string();
    let auth = token
        .map(|t| format!("authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    let request = format!("{method} {path} HTTP/1.1\r\nhost: {address}\r\n{auth}content-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
    stream.write_all(request.as_bytes()).expect("client write");
    read_response(&mut stream, partial).expect("client response")
}

struct ServerTask(Option<tokio::task::JoinHandle<()>>);
impl ServerTask {
    async fn stop(mut self) {
        let server = self.0.take().expect("owned server");
        server.abort();
        assert!(server
            .await
            .expect_err("server must be cancelled")
            .is_cancelled());
    }
}
impl Drop for ServerTask {
    fn drop(&mut self) {
        if let Some(server) = self.0.take() {
            server.abort();
        }
    }
}

fn response_parts(wire: &[u8]) -> (u16, Vec<u8>) {
    let end = wire
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("complete HTTP headers");
    let headers = std::str::from_utf8(&wire[..end]).expect("HTTP header UTF-8");
    let status = headers
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let body = &wire[end + 4..];
    let chunked = headers.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("transfer-encoding")
                && value.trim().eq_ignore_ascii_case("chunked")
        })
    });
    if !chunked {
        return (status, body.to_vec());
    }
    let mut cursor = 0usize;
    let mut decoded = Vec::new();
    loop {
        let line_end = body[cursor..]
            .windows(2)
            .position(|w| w == b"\r\n")
            .expect("chunk header")
            + cursor;
        let size = usize::from_str_radix(
            std::str::from_utf8(&body[cursor..line_end])
                .unwrap()
                .split(';')
                .next()
                .unwrap(),
            16,
        )
        .expect("chunk size");
        cursor = line_end + 2;
        if size == 0 {
            break;
        }
        assert!(
            size <= body.len().saturating_sub(cursor + 2),
            "complete bounded HTTP chunk"
        );
        decoded.extend_from_slice(&body[cursor..cursor + size]);
        cursor += size;
        assert_eq!(&body[cursor..cursor + 2], b"\r\n");
        cursor += 2;
    }
    (status, decoded)
}

fn assert_completed(wire: &[u8], streaming: bool) {
    let (status, body) = response_parts(wire);
    assert_eq!(status, 201, "turn must complete successfully");
    if streaming {
        let events: Vec<Value> = std::str::from_utf8(&body)
            .unwrap()
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_str(line).expect("NDJSON event"))
            .collect();
        assert!(events
            .iter()
            .any(|event| event["type"] == "assistant_delta" && event["delta"] == "fixture"));
        assert!(events
            .iter()
            .any(|event| event["type"] == "assistant_message"
                && event["message"]["body"]["text"] == "fixture"));
        assert!(!events.iter().any(|event| event["type"] == "error"));
    } else {
        let json: Value = serde_json::from_slice(&body).expect("completed turn JSON");
        assert_eq!(json["assistant_message"]["body"]["text"], "fixture");
    }
}

async fn history(address: SocketAddr, token: String, id: String) -> Vec<Value> {
    let wire = tokio::task::spawn_blocking(move || {
        client(
            address,
            Some(&token),
            "GET",
            &format!("/api/sessions/{id}/messages?limit=50"),
            &Value::Null,
            None,
        )
    })
    .await
    .expect("history client");
    let (status, body) = response_parts(&wire);
    assert_eq!(status, 200);
    serde_json::from_slice::<Value>(&body).expect("history JSON")["messages"]
        .as_array()
        .expect("shared messages")
        .clone()
}

async fn harness() -> (
    SocketAddr,
    String,
    axum::Router,
    ServerTask,
    Provider,
    tempfile::TempDir,
    EnvGuard,
) {
    let provider = Provider::start();
    let env = EnvGuard::install(&provider.base_url());
    let dir = tempdir().expect("disposable storage");
    let storage = Storage::open_in_memory(dir.path().join("blobs")).expect("storage");
    let sessions = SessionService::new(Arc::new(storage));
    let auth = DaemonAuth::mint().expect("daemon auth");
    let token = auth.token().to_owned();
    let app = router_with_auth(
        AppState {
            sessions,
            catalog: Arc::new(Catalog::default()),
        },
        Some(auth),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("server bind");
    let address = listener.local_addr().expect("server address");
    let server_app = app.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, server_app).await.expect("server");
    });
    (
        address,
        token,
        app,
        ServerTask(Some(server)),
        provider,
        dir,
        env,
    )
}

async fn make_session(app: &axum::Router, token: &str, title: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"title":title}).to_string()))
                .expect("create request"),
        )
        .await
        .expect("create response");
    assert_eq!(response.status(), axum::http::StatusCode::CREATED);
    let body = axum::body::to_bytes(response.into_body(), 16 * 1024)
        .await
        .expect("create body");
    serde_json::from_slice::<Value>(&body).expect("create json")["session"]["id"]
        .as_str()
        .expect("session id")
        .to_owned()
}

async fn run_pair(stream_a: bool, stream_b: bool, same: bool) {
    let serial = ENV_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let (address, token, app, server, provider, _dir, _env) = harness().await;
    let first = make_session(&app, &token, "ownership-a").await;
    let second = if same {
        first.clone()
    } else {
        make_session(&app, &token, "ownership-b").await
    };
    let path_a = format!(
        "/api/sessions/{first}/turns{}",
        if stream_a { "/stream" } else { "" }
    );
    let path_b = format!(
        "/api/sessions/{second}/turns{}",
        if stream_b { "/stream" } else { "" }
    );
    let a_addr = address;
    let a_token = token.clone();
    let (partial_tx, partial_rx) = tokio::sync::oneshot::channel();
    let a = tokio::task::spawn_blocking(move || {
        client(
            a_addr,
            Some(&a_token),
            "POST",
            &path_a,
            &json!({"text":"a","model":MODEL}),
            stream_a.then_some(partial_tx),
        )
    });
    let first_started = provider.wait_for(1).await;
    let partial_seen = !stream_a
        || tokio::time::timeout(Duration::from_secs(2), partial_rx)
            .await
            .is_ok_and(|result| result.is_ok());
    let b_addr = address;
    let b_token = token.clone();
    let b = tokio::task::spawn_blocking(move || {
        client(
            b_addr,
            Some(&b_token),
            "POST",
            &path_b,
            &json!({"text":"b","model":MODEL}),
            None,
        )
    });
    let _ = provider.wait_for(2).await;
    provider.release();
    let ra = a.await;
    let rb = b.await;
    let messages = history(address, token.clone(), second).await;
    let bad_request = provider.bad_request();
    let maximum = provider.maximum();
    let accepted = provider.accepted();
    server.stop().await;
    drop(provider);
    drop(_env);
    drop(serial);
    let ra = ra.expect("client a");
    let rb = rb.expect("client b");
    assert!(
        first_started && partial_seen,
        "first execution and native partial must be observed before client B"
    );
    assert!(
        bad_request.is_none(),
        "fixture protocol failure: {bad_request:?}"
    );
    assert_completed(&ra, stream_a);
    let (status_b, body_b) = response_parts(&rb);
    if same && status_b == 429 {
        assert_eq!(accepted, 1, "rejected turn reached provider");
        assert!(
            !messages
                .iter()
                .any(|m| m["role"] == "user" && m["body"]["text"] == "b"),
            "rejected turn appended user message"
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&body_b).expect("typed rejection")["code"],
            "too_many_requests"
        );
    } else {
        assert_completed(&rb, stream_b);
        assert_eq!(accepted, 2);
        assert!(messages
            .iter()
            .any(|m| m["role"] == "user" && m["body"]["text"] == "b"));
    }
    if same {
        assert!(maximum <= 1, "same session provider overlap: {maximum}");
    } else {
        assert_eq!(maximum, 2, "different sessions must use both global slots");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn same_session_two_streaming_turns_are_serial() {
    run_pair(true, true, true).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn same_session_nonstream_then_stream_is_serial() {
    run_pair(false, true, true).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn same_session_stream_then_nonstream_is_serial() {
    run_pair(true, false, true).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn different_sessions_can_use_two_provider_slots() {
    run_pair(false, false, false).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unauthenticated_second_client_has_no_provider_side_effect() {
    let serial = ENV_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let (address, token, app, server, provider, _dir, _env) = harness().await;
    let session = make_session(&app, &token, "auth ownership").await;
    let good_path = format!("/api/sessions/{session}/turns");
    let first_addr = address;
    let first_token = token.clone();
    let first = tokio::task::spawn_blocking(move || {
        client(
            first_addr,
            Some(&first_token),
            "POST",
            &good_path,
            &json!({"text":"good","model":MODEL}),
            None,
        )
    });
    let first_started = provider.wait_for(1).await;
    let denied_path = format!("/api/sessions/{session}/turns");
    let denied_addr = address;
    let denied = tokio::task::spawn_blocking(move || {
        client(
            denied_addr,
            None,
            "POST",
            &denied_path,
            &json!({"text":"denied","model":MODEL}),
            None,
        )
    });
    let denied_response = denied.await;
    provider.release();
    let first_response = first.await;
    let messages = history(address, token, session).await;
    let maximum = provider.maximum();
    let accepted = provider.accepted();
    let bad_request = provider.bad_request();
    server.stop().await;
    drop(provider);
    drop(_env);
    drop(serial);
    assert!(first_started);
    assert_eq!(
        response_parts(&denied_response.expect("denied client")).0,
        401
    );
    assert_completed(&first_response.expect("first client"), false);
    assert_eq!(accepted, 1, "denied client reached provider");
    assert_eq!(maximum, 1);
    assert!(
        bad_request.is_none(),
        "fixture protocol failure: {bad_request:?}"
    );
    assert_eq!(messages.len(), 2);
    assert!(
        !messages
            .iter()
            .any(|message| message["role"] == "user" && message["body"]["text"] == "denied"),
        "unauthorized client appended a user message"
    );
}
