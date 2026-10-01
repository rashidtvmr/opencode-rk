//! LIVE-INTERRUPT-CONTRACT: authenticated session interrupt via public router.
//!
//! Upstream authority (pinned 95daf90670b7c039c436c85537da5fbfe2205b41):
//!   packages/protocol/src/groups/session.ts:345-356
//!     POST '/api/session/:sessionID/interrupt' singular, returns 204 NoContent.
//!   packages/server/src/handlers/session.ts:366-370
//!     calls session.interrupt(ctx.params.sessionID), returns HttpApiSchema.NoContent.
//!   packages/opencode/src/effect/runner.ts:94-101,108-113
//!     idle -> no-op; active -> stopping=true, pendingWake clear, Fiber.interrupt.
//!
//! Current Rust state: `crates/server/src/lib.rs` registers plural turn routes
//! (`/api/sessions/{id}/turns`, `/api/sessions/{id}/turns/stream`) but has NO
//! singular interrupt route. The `TurnService` in `turn_service.rs:421-434`
//! already models `Turn::interrupt()` -> Cancelled + token fire, but it is not
//! wired to any HTTP endpoint.
//!
//! These are genuine RED contract tests: they compile against the existing
//! public router_with_auth + DaemonAuth + Storage/SessionService/Catalog APIs
//! and fail because the route does not exist yet. They do NOT invent internal
//! cancellation symbols or use isolated dummy HTTP codes.
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

use axum::body::Body;
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{daemon_auth::DaemonAuth, router_with_auth, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tempfile::{tempdir, TempDir};
use tower::ServiceExt;

const PROVIDER_MAX_WIRE: usize = 128 * 1024;
const CLIENT_MAX_WIRE: usize = 256 * 1024;
const MODEL: &str = "openai/gpt-5.6";
const CASE_DEADLINE_SECS: u64 = 20;
const SUITE_DEADLINE_SECS: u64 = 90;

static ENV_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

// ---------------------------------------------------------------------------
// Environment guard (matches session_execution_ownership.rs pattern READONLY)
// ---------------------------------------------------------------------------

struct EnvGuard {
    old_base: Option<String>,
    old_key: Option<String>,
    old_home: Option<String>,
    old_config: Option<String>,
    old_data: Option<String>,
    old_state: Option<String>,
    old_cache: Option<String>,
    old_rk_home: Option<String>,
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
        // Generated fake API key for disposable fixture, never user secrets.
        std::env::set_var("OPENAI_BASE_URL", base);
        std::env::set_var("OPENAI_API_KEY", "fixture-interrupt-key-a1b2c3d4");
        std::env::set_var("HOME", home.path());
        std::env::set_var("XDG_CONFIG_HOME", config.path());
        std::env::set_var("XDG_DATA_HOME", data.path());
        std::env::set_var("XDG_STATE_HOME", state.path());
        std::env::set_var("XDG_CACHE_HOME", cache.path());
        std::env::set_var("OPENCODE_RK_HOME", home.path());
        Self {
            old_base,
            old_key,
            old_home,
            old_config,
            old_data,
            old_state,
            old_cache,
            old_rk_home,
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
        for (key, val) in [
            ("OPENAI_BASE_URL", &self.old_base),
            ("OPENAI_API_KEY", &self.old_key),
            ("HOME", &self.old_home),
            ("XDG_CONFIG_HOME", &self.old_config),
            ("XDG_DATA_HOME", &self.old_data),
            ("XDG_STATE_HOME", &self.old_state),
            ("XDG_CACHE_HOME", &self.old_cache),
            ("OPENCODE_RK_HOME", &self.old_rk_home),
        ] {
            match val {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Controlled streaming provider fixture
// (matches session_execution_ownership.rs Provider pattern READONLY)
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ProviderCounts {
    accepted: usize,
    active: usize,
    disconnected: usize,
}

struct ProviderFixture {
    address: SocketAddr,
    counts: Arc<(Mutex<ProviderCounts>, Condvar)>,
    release: Arc<(Mutex<bool>, Condvar)>,
    stopping: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl ProviderFixture {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("provider bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let address = listener.local_addr().expect("address");
        let counts = Arc::new((Mutex::new(ProviderCounts::default()), Condvar::new()));
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let stopping = Arc::new(AtomicBool::new(false));
        let wc = Arc::clone(&counts);
        let wr = Arc::clone(&release);
        let ws = Arc::clone(&stopping);
        let thread = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(CASE_DEADLINE_SECS);
            let mut workers = Vec::new();
            while Instant::now() < deadline && !ws.load(Ordering::Acquire) && workers.len() < 2 {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let c = Arc::clone(&wc);
                        let r = Arc::clone(&wr);
                        workers.push(thread::spawn(move || handle_provider(stream, c, r)));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(e) => panic!("provider accept: {e}"),
                }
            }
            for w in workers {
                let _ = w.join();
            }
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

    async fn wait_accepted(&self, n: usize) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if self.counts.0.lock().expect("lock").accepted >= n {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    fn release_all(&self) {
        let (lock, wake) = &*self.release;
        *lock.lock().expect("lock") = true;
        wake.notify_all();
    }

    fn accepted_count(&self) -> usize {
        self.counts.0.lock().expect("lock").accepted
    }

    async fn wait_disconnected(&self) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if self.counts.0.lock().expect("lock").disconnected > 0 {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

impl Drop for ProviderFixture {
    fn drop(&mut self) {
        self.release_all();
        self.stopping.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn handle_provider(
    mut stream: TcpStream,
    counts: Arc<(Mutex<ProviderCounts>, Condvar)>,
    release: Arc<(Mutex<bool>, Condvar)>,
) {
    stream
        .set_read_timeout(Some(Duration::from_millis(20)))
        .expect("read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .expect("write timeout");
    let req = read_bounded_request(&mut stream).expect("request");
    let text = String::from_utf8(req).expect("utf8");
    let (headers, body) = text.split_once("\r\n\r\n").expect("body split");
    let parsed: Value = serde_json::from_str(body).expect("json");
    let streaming = parsed["stream"] == true;
    {
        let (lock, wake) = &*counts;
        let mut state = lock.lock().expect("lock");
        state.accepted += 1;
        state.active += 1;
        wake.notify_all();
    }
    if streaming {
        // Emit first delta immediately so the client observes partial output.
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n\
                  event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n",
            )
            .expect("first delta");
        stream.flush().expect("flush delta");
    }
    // Hold until explicitly released, client EOF, or an absolute deadline.
    let (rlock, rwake) = &*release;
    let mut done = rlock.lock().expect("lock");
    let deadline = Instant::now() + Duration::from_secs(CASE_DEADLINE_SECS);
    let mut disconnected = false;
    while !*done && Instant::now() < deadline {
        drop(done);
        let mut probe = [0u8; 1];
        match stream.read(&mut probe) {
            Ok(0) => disconnected = true,
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => disconnected = true,
        }
        if disconnected {
            break;
        }
        done = rlock.lock().expect("lock");
        if !*done {
            let (next, _) = rwake
                .wait_timeout(done, Duration::from_millis(20))
                .expect("wait");
            done = next;
        }
    }
    drop(done);
    if disconnected {
        let (lock, wake) = &*counts;
        let mut state = lock.lock().expect("lock");
        state.disconnected += 1;
        state.active -= 1;
        wake.notify_all();
        return;
    }
    if streaming {
        stream
            .write_all(
                b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n",
            )
            .expect("completion");
    } else {
        let body = json!({"id":"fix","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"done"}]}]}).to_string();
        let resp = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(resp.as_bytes()).expect("response");
    }
    let (lock, _) = &*counts;
    lock.lock().expect("lock").active -= 1;
}

fn read_bounded_request(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buf = [0u8; 4096];
    let mut total = None;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if Instant::now() >= deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "deadline",
            ));
        }
        let n = stream.read(&mut buf)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "eof",
            ));
        }
        if bytes.len() > PROVIDER_MAX_WIRE.saturating_sub(n) {
            return Err(std::io::Error::other("wire bound"));
        }
        bytes.extend_from_slice(&buf[..n]);
        if total.is_none() {
            if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let hdrs = String::from_utf8_lossy(&bytes[..end]);
                let len = hdrs
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .ok_or_else(|| std::io::Error::other("no content-length"))?;
                if len > PROVIDER_MAX_WIRE - end - 4 {
                    return Err(std::io::Error::other("body bound"));
                }
                total = Some(end + 4 + len);
            }
        }
        if total.is_some_and(|t| bytes.len() >= t) {
            break;
        }
    }
    Ok(bytes)
}

// ---------------------------------------------------------------------------
// Raw TCP client (matches session_execution_ownership.rs pattern READONLY)
// ---------------------------------------------------------------------------

fn raw_http(
    address: SocketAddr,
    token: Option<&str>,
    method: &str,
    path: &str,
    body: &Value,
) -> (u16, Vec<u8>) {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(CASE_DEADLINE_SECS)))
        .expect("read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .expect("write timeout");
    let body_str = body.to_string();
    let auth = token
        .map(|t| format!("authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nhost: {address}\r\n{auth}content-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body_str}",
        body_str.len()
    );
    stream.write_all(request.as_bytes()).expect("write");
    let mut response = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if response.len() > CLIENT_MAX_WIRE.saturating_sub(n) {
                    break;
                }
                response.extend_from_slice(&buf[..n]);
            }
        }
    }
    parse_response(&response)
}

fn parse_response(wire: &[u8]) -> (u16, Vec<u8>) {
    let end = wire
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or(wire.len());
    if end == wire.len() {
        return (0, wire.to_vec());
    }
    let headers = std::str::from_utf8(&wire[..end]).unwrap_or("");
    let status = headers
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    let body = &wire[end + 4..];
    let chunked = headers.lines().any(|l| {
        l.split_once(':').is_some_and(|(k, v)| {
            k.eq_ignore_ascii_case("transfer-encoding") && v.trim().eq_ignore_ascii_case("chunked")
        })
    });
    if !chunked {
        return (status, body.to_vec());
    }
    let mut cursor = 0;
    let mut decoded = Vec::new();
    loop {
        let le = body[cursor..]
            .windows(2)
            .position(|w| w == b"\r\n")
            .map(|p| p + cursor);
        let Some(line_end) = le else { break };
        let size_str = std::str::from_utf8(&body[cursor..line_end])
            .unwrap_or("0")
            .split(';')
            .next()
            .unwrap_or("0");
        let size = usize::from_str_radix(size_str, 16).unwrap_or(0);
        cursor = line_end + 2;
        if size == 0 {
            break;
        }
        decoded.extend_from_slice(&body[cursor..cursor + size]);
        cursor += size + 2;
    }
    (status, decoded)
}

// ---------------------------------------------------------------------------
// Server harness (matches session_execution_ownership.rs pattern READONLY)
// ---------------------------------------------------------------------------

struct ServerTask(Option<tokio::task::JoinHandle<()>>);
impl ServerTask {
    async fn stop(mut self) {
        if let Some(s) = self.0.take() {
            s.abort();
            let _ = s.await;
        }
    }
}
impl Drop for ServerTask {
    fn drop(&mut self) {
        if let Some(s) = self.0.take() {
            s.abort();
        }
    }
}

async fn harness() -> (
    SocketAddr,
    String,
    axum::Router,
    ServerTask,
    ProviderFixture,
    EnvGuard,
) {
    let provider = ProviderFixture::start();
    let env = EnvGuard::install(&provider.base_url());
    let dir = tempdir().expect("storage");
    let storage = Storage::open_in_memory(dir.path().join("blobs")).expect("storage");
    let sessions = SessionService::new(Arc::new(storage));
    let auth = DaemonAuth::mint().expect("auth");
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
        .expect("bind");
    let address = listener.local_addr().expect("addr");
    let server_app = app.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, server_app).await.expect("serve");
    });
    (address, token, app, ServerTask(Some(server)), provider, env)
}

async fn create_session(app: &axum::Router, token: &str, title: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"title": title}).to_string()))
                .expect("req"),
        )
        .await
        .expect("resp");
    assert_eq!(response.status(), axum::http::StatusCode::CREATED);
    let body = axum::body::to_bytes(response.into_body(), 16 * 1024)
        .await
        .expect("body");
    serde_json::from_slice::<Value>(&body).expect("json")["session"]["id"]
        .as_str()
        .expect("id")
        .to_owned()
}

async fn fetch_messages(address: SocketAddr, token: &str, session_id: &str) -> Vec<Value> {
    let (status, body) = raw_http(
        address,
        Some(token),
        "GET",
        &format!("/api/sessions/{session_id}/messages?limit=50"),
        &Value::Null,
    );
    assert_eq!(status, 200, "messages fetch failed");
    serde_json::from_slice::<Value>(&body).expect("json")["messages"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Interrupt path constant matching upstream protocol definition.
// Upstream uses SINGULAR /api/session/:sessionID/interrupt.
// Keep the pinned public protocol path here; the current Rust router has no
// interrupt route. The integrator must register this singular endpoint.
// ---------------------------------------------------------------------------
fn interrupt_path(session_id: &str) -> String {
    format!("/api/session/{session_id}/interrupt")
}

// ===========================================================================
// Five parameterized cases exercising the singular interrupt operation on the
// SAME current turn execution through the real public router_with_auth.
// ===========================================================================

/// Case 1: Streaming authorized interrupt returns 204 empty body.
/// After interrupt, the original stream reaches EOF/provider close.
/// Durable user message kept, no successful assistant persisted.
/// Same-session new prompt completes (not 429).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_streaming_authorized_returns_204_and_reclaims_slot() {
    let suite_start = Instant::now();
    let serial = ENV_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let (address, token, app, server, provider, _env) = harness().await;
    let session = create_session(&app, &token, "interrupt-stream").await;

    // Start a streaming turn in background.
    let turn_addr = address;
    let turn_token = token.clone();
    let turn_session = session.clone();
    let turn_handle = tokio::task::spawn_blocking(move || {
        raw_http(
            turn_addr,
            Some(&turn_token),
            "POST",
            &format!("/api/sessions/{turn_session}/turns/stream"),
            &json!({"text": "hello interrupt", "model": MODEL}),
        )
    });

    // Wait for provider to accept the streaming request (first delta observed).
    let started = provider.wait_accepted(1).await;
    assert!(started, "provider must accept before interrupt");
    assert!(
        suite_start.elapsed() < Duration::from_secs(SUITE_DEADLINE_SECS),
        "suite deadline exceeded"
    );

    // Send interrupt with valid bearer.
    let (int_status, int_body) = raw_http(
        address,
        Some(&token),
        "POST",
        &interrupt_path(&session),
        &json!({}),
    );

    // CONTRACT: must be 204 NoContent with empty body.
    // Currently RED: route does not exist, expect 404 or fallback.
    assert_eq!(
        int_status,
        204,
        "interrupt must return 204 NoContent, got {int_status} body={}",
        String::from_utf8_lossy(&int_body)
    );
    assert!(
        int_body.is_empty(),
        "204 response must have empty body, got {} bytes",
        int_body.len()
    );

    let (turn_status, _turn_body) = turn_handle.await.expect("turn join");
    assert!(
        provider.wait_disconnected().await,
        "interrupted provider request must observe client EOF before cleanup"
    );
    // Fallback cleanup remains explicit and idempotent.
    provider.release_all();

    // After interrupt, durable user message must be kept.
    let messages = fetch_messages(address, &token, &session).await;
    assert!(
        messages.iter().any(|m| m["role"] == "user"),
        "user message must be durable after interrupt"
    );
    // No successful assistant message should be persisted from the interrupted turn.
    assert!(
        !messages.iter().any(|m| m["role"] == "assistant"),
        "interrupted turn must not persist assistant message"
    );

    // Same-session new prompt must complete (slot reclaimed, not 429).
    let (new_status, _new_body) = raw_http(
        address,
        Some(&token),
        "POST",
        &format!("/api/sessions/{session}/turns"),
        &json!({"text": "after interrupt", "model": MODEL}),
    );
    assert_ne!(
        new_status, 429,
        "post-interrupt turn must not be rejected as 429; slot must be reclaimed"
    );

    server.stop().await;
    drop(provider);
    drop(_env);
    drop(serial);
    assert!(
        suite_start.elapsed() < Duration::from_secs(SUITE_DEADLINE_SECS),
        "suite deadline exceeded at cleanup"
    );
}

/// Case 2: Missing Bearer on interrupt returns 401 without cancelling the turn.
/// Positive provider completes after release proving interrupt was denied.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_missing_bearer_returns_401_no_cancel() {
    let suite_start = Instant::now();
    let serial = ENV_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let (address, token, app, server, provider, _env) = harness().await;
    let session = create_session(&app, &token, "interrupt-no-auth").await;

    let turn_addr = address;
    let turn_token = token.clone();
    let turn_session = session.clone();
    let turn_handle = tokio::task::spawn_blocking(move || {
        raw_http(
            turn_addr,
            Some(&turn_token),
            "POST",
            &format!("/api/sessions/{turn_session}/turns/stream"),
            &json!({"text": "auth test", "model": MODEL}),
        )
    });

    let started = provider.wait_accepted(1).await;
    assert!(started, "provider must accept");

    // Interrupt WITHOUT Authorization header.
    let (int_status, _) = raw_http(
        address,
        None, // missing bearer
        "POST",
        &interrupt_path(&session),
        &json!({}),
    );
    assert_eq!(
        int_status, 401,
        "missing bearer must return 401, got {int_status}"
    );

    // Provider must still be running (not cancelled by unauthenticated request).
    assert_eq!(provider.accepted_count(), 1, "provider count must remain 1");

    // Release and verify positive completion.
    provider.release_all();
    let (turn_status, _) = turn_handle.await.expect("turn join");
    assert_eq!(
        turn_status, 201,
        "positive provider must complete after denied interrupt"
    );

    server.stop().await;
    drop(provider);
    drop(_env);
    drop(serial);
    assert!(suite_start.elapsed() < Duration::from_secs(SUITE_DEADLINE_SECS));
}

/// Case 3: Wrong Bearer on interrupt returns 403 without cancelling the turn.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_wrong_bearer_returns_403_no_cancel() {
    let suite_start = Instant::now();
    let serial = ENV_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let (address, token, app, server, provider, _env) = harness().await;
    let session = create_session(&app, &token, "interrupt-bad-auth").await;

    let turn_addr = address;
    let turn_token = token.clone();
    let turn_session = session.clone();
    let turn_handle = tokio::task::spawn_blocking(move || {
        raw_http(
            turn_addr,
            Some(&turn_token),
            "POST",
            &format!("/api/sessions/{turn_session}/turns/stream"),
            &json!({"text": "wrong token test", "model": MODEL}),
        )
    });

    let started = provider.wait_accepted(1).await;
    assert!(started);

    // Wrong bearer token.
    let wrong_token = "aa".repeat(32);
    let (int_status, _) = raw_http(
        address,
        Some(&wrong_token),
        "POST",
        &interrupt_path(&session),
        &json!({}),
    );
    assert_eq!(
        int_status, 403,
        "wrong bearer must return 403, got {int_status}"
    );

    provider.release_all();
    let (turn_status, _) = turn_handle.await.expect("turn join");
    assert_eq!(
        turn_status, 201,
        "turn must complete after denied interrupt"
    );

    server.stop().await;
    drop(provider);
    drop(_env);
    drop(serial);
    assert!(suite_start.elapsed() < Duration::from_secs(SUITE_DEADLINE_SECS));
}

/// Case 4: Interrupt on idle session (no active turn) is a no-op returning 204.
/// Repeated interrupts on idle are also no-ops.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_idle_session_is_noop() {
    let suite_start = Instant::now();
    let serial = ENV_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let (address, token, app, server, provider, _env) = harness().await;
    let session = create_session(&app, &token, "interrupt-idle").await;

    // No turn started. Interrupt must be no-op 204.
    let (status1, body1) = raw_http(
        address,
        Some(&token),
        "POST",
        &interrupt_path(&session),
        &json!({}),
    );
    assert_eq!(
        status1, 204,
        "idle interrupt must return 204, got {status1}"
    );
    assert!(body1.is_empty(), "204 must have empty body");

    // Repeat: still no-op.
    let (status2, _) = raw_http(
        address,
        Some(&token),
        "POST",
        &interrupt_path(&session),
        &json!({}),
    );
    assert_eq!(status2, 204, "repeated idle interrupt must be no-op 204");

    // Provider must have received zero requests.
    assert_eq!(
        provider.accepted_count(),
        0,
        "idle interrupt must not contact provider"
    );

    server.stop().await;
    drop(provider);
    drop(_env);
    drop(serial);
    assert!(suite_start.elapsed() < Duration::from_secs(SUITE_DEADLINE_SECS));
}

/// Case 5: Interrupt on OTHER session does not cancel the first session's turn.
/// Non-stream cancel reclaims same-slot.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_other_session_is_noop_for_first() {
    let suite_start = Instant::now();
    let serial = ENV_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let (address, token, app, server, provider, _env) = harness().await;
    let session_a = create_session(&app, &token, "interrupt-other-a").await;
    let session_b = create_session(&app, &token, "interrupt-other-b").await;

    // Start non-stream turn on session A.
    let turn_addr = address;
    let turn_token = token.clone();
    let turn_session = session_a.clone();
    let turn_handle = tokio::task::spawn_blocking(move || {
        raw_http(
            turn_addr,
            Some(&turn_token),
            "POST",
            &format!("/api/sessions/{turn_session}/turns"),
            &json!({"text": "other session test", "model": MODEL}),
        )
    });

    let started = provider.wait_accepted(1).await;
    assert!(started);

    // Interrupt session B (which has no active turn). Must be no-op 204.
    let (int_status, _) = raw_http(
        address,
        Some(&token),
        "POST",
        &interrupt_path(&session_b),
        &json!({}),
    );
    assert_eq!(
        int_status, 204,
        "interrupt on other session must return 204 no-op"
    );

    // Session A's turn must still be running (not cancelled by B's interrupt).
    assert_eq!(provider.accepted_count(), 1);

    // Release and verify A completes.
    provider.release_all();
    let (turn_status, _) = turn_handle.await.expect("turn join");
    assert_eq!(
        turn_status, 201,
        "session A turn must complete after B's no-op interrupt"
    );

    // Verify session A has durable messages.
    let messages = fetch_messages(address, &token, &session_a).await;
    assert!(
        messages.iter().any(|m| m["role"] == "user"),
        "session A must have durable user message"
    );
    assert!(
        messages.iter().any(|m| m["role"] == "assistant"),
        "session A must have completed assistant message"
    );

    server.stop().await;
    drop(provider);
    drop(_env);
    drop(serial);
    assert!(suite_start.elapsed() < Duration::from_secs(SUITE_DEADLINE_SECS));
}
