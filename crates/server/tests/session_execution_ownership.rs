//! G4-SESSION-EXECUTION-SERIALIZATION: live transport contract for session ownership.
//!
//! These tests deliberately exercise the authenticated Axum listener, rather than a
//! router service or a turn-service seam.  They are RED contract tests: the current
//! process-wide two-permit implementation permits two provider calls for one ID.
#![forbid(unsafe_code)]

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{Arc, Condvar, Mutex, OnceLock},
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
const MODEL: &str = "gpt-5.6";

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
        let old_auth_json = std::env::var("OPENAI_AUTH_JSON").ok();
        std::env::set_var("OPENAI_BASE_URL", base);
        std::env::set_var("OPENAI_API_KEY", "fixture-bearer");
        std::env::set_var("HOME", home.path());
        std::env::set_var("XDG_CONFIG_HOME", config.path());
        std::env::set_var("XDG_DATA_HOME", data.path());
        std::env::set_var("XDG_STATE_HOME", state.path());
        std::env::set_var("XDG_CACHE_HOME", cache.path());
        std::env::set_var("OPENCODE_RK_HOME", home.path());
        std::env::remove_var("OPENAI_AUTH_JSON");
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
            Some(value) => std::env::set_var("OPENAI_AUTH_JSON", value),
            None => std::env::remove_var("OPENAI_AUTH_JSON"),
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
        let thread = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(8);
            let mut workers = Vec::new();
            while Instant::now() < deadline {
                if worker_counts.0.lock().expect("counts").accepted >= 2 {
                    break;
                }
                match listener.accept() {
                    Ok((stream, _)) => {
                        if worker_counts.0.lock().expect("counts").accepted >= 2 {
                            break;
                        }
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
            for worker in workers {
                worker.join().expect("provider request worker");
            }
        });
        Self {
            address,
            counts,
            release,
            thread: Some(thread),
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}/v1", self.address)
    }

    fn wait_for(&self, n: usize) {
        let (lock, wake) = &*self.counts;
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut counts = lock.lock().expect("counts");
        while counts.accepted < n && Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let (next, _) = wake.wait_timeout(counts, remaining).expect("count wait");
            counts = next;
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
}

impl Drop for Provider {
    fn drop(&mut self) {
        self.release();
        if let Some(thread) = self.thread.take() {
            thread.join().expect("provider server thread");
        }
    }
}

fn provider_request(
    mut stream: TcpStream,
    counts: Arc<(Mutex<Counts>, Condvar)>,
    release: Arc<(Mutex<bool>, Condvar)>,
) {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("provider read timeout");
    let request = read_request(&mut stream).expect("provider request");
    let text = String::from_utf8(request.clone()).expect("provider utf8");
    let (headers, body) = text.split_once("\r\n\r\n").expect("provider body");
    let parsed: Value = serde_json::from_str(body).expect("provider json");
    let valid = headers.starts_with("POST /v1/responses HTTP/1.1")
        && headers
            .to_ascii_lowercase()
            .contains("authorization: bearer fixture-bearer\r\n")
        && parsed["model"] == MODEL
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
            state.bad_request = Some(text.chars().take(256).collect());
        }
        wake.notify_all();
    }
    let streaming = streaming;
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
    loop {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            break;
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
                            .and_then(|v| v.trim().parse().ok())
                    })
                    .ok_or_else(|| std::io::Error::other("missing content length"))?;
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

fn read_response(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let deadline = Instant::now() + Duration::from_secs(8);
    stream.set_read_timeout(Some(Duration::from_millis(250)))?;
    while Instant::now() < deadline {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                bytes.extend_from_slice(&buffer[..n]);
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
    if bytes.is_empty() {
        return Err(std::io::Error::other("empty response"));
    }
    Ok(bytes)
}

fn client(address: SocketAddr, token: Option<&str>, path: &str, body: &Value) -> Vec<u8> {
    let mut stream =
        TcpStream::connect_timeout(&address, Duration::from_secs(2)).expect("client connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(8)))
        .expect("client timeout");
    let body = body.to_string();
    let auth = token
        .map(|t| format!("authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    let request = format!("POST {path} HTTP/1.1\r\nhost: {address}\r\n{auth}content-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
    stream.write_all(request.as_bytes()).expect("client write");
    read_response(&mut stream).expect("client response")
}

async fn harness() -> (
    SocketAddr,
    String,
    axum::Router,
    tokio::task::JoinHandle<()>,
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
    (address, token, app, server, provider, dir, env)
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
    let a = tokio::task::spawn_blocking(move || {
        client(
            a_addr,
            Some(&a_token),
            &path_a,
            &json!({"text":"a","model":MODEL}),
        )
    });
    provider.wait_for(1);
    let b_addr = address;
    let b_token = token.clone();
    let b = tokio::task::spawn_blocking(move || {
        client(
            b_addr,
            Some(&b_token),
            &path_b,
            &json!({"text":"b","model":MODEL}),
        )
    });
    provider.wait_for(2);
    provider.release();
    let ra = a.await.expect("client a");
    let rb = b.await.expect("client b");
    assert!(
        String::from_utf8_lossy(&ra).starts_with("HTTP/1.1 201"),
        "first turn failed: {}",
        String::from_utf8_lossy(&ra)
    );
    if !same {
        assert!(
            String::from_utf8_lossy(&rb).starts_with("HTTP/1.1 201"),
            "second session failed: {}",
            String::from_utf8_lossy(&rb)
        );
    }
    assert!(
        provider.bad_request().is_none(),
        "fixture rejected request: {:?}",
        provider.bad_request()
    );
    let maximum = provider.maximum();
    server.abort();
    server.await.expect_err("server must be aborted");
    drop(provider);
    drop(serial);
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
            &good_path,
            &json!({"text":"good","model":MODEL}),
        )
    });
    provider.wait_for(1);
    let denied_path = format!("/api/sessions/{session}/turns");
    let denied_addr = address;
    let denied = tokio::task::spawn_blocking(move || {
        client(
            denied_addr,
            None,
            &denied_path,
            &json!({"text":"denied","model":MODEL}),
        )
    });
    let denied_response = denied.await.expect("denied client");
    assert!(String::from_utf8_lossy(&denied_response).starts_with("HTTP/1.1 401"));
    provider.release();
    let first_response = first.await.expect("first client");
    assert!(String::from_utf8_lossy(&first_response).starts_with("HTTP/1.1 201"));
    assert_eq!(provider.maximum(), 1, "denied client reached provider");
    server.abort();
    server.await.expect_err("server must be aborted");
    drop(provider);
    drop(serial);
}
