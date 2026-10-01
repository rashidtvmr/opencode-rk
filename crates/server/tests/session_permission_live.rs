//! LIVE-PERMISSION-CONTRACT: real authenticated session-turn permission lifecycle.
//!
//! The fixture owns one bounded provider, server, client turn, and disposable
//! project. The permission routes follow the pinned OpenCode V2 contract; the
//! tests are intentionally RED against the current server, which does not yet
//! publish or await execution-owned permission requests.
#![forbid(unsafe_code)]

use std::{
    env,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
    time::{Duration, Instant},
};

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_security::{
    Decision, FileAction, OperationIntent, PermissionBroker, PermissionSet, SecurityPolicy,
};
use opencode_rk_server::{daemon_auth::DaemonAuth, router_with_auth, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use opencode_rk_tools::file_ops::{FileOperation, FileTool};
use serde_json::{json, Value};
use tempfile::{tempdir, TempDir};
use tower::ServiceExt;

const MAX_REQUEST_BYTES: usize = 128 * 1024;
const MAX_RESPONSE_BYTES: usize = 256 * 1024;
const MAX_RESPONSE_WIRE_BYTES: usize = MAX_RESPONSE_BYTES + 16 * 1024;
const MAX_CASE_DURATION: Duration = Duration::from_secs(20);
const MAX_SUITE_DURATION: Duration = Duration::from_secs(90);
const MODEL: &str = "openai/gpt-5.6";
const CALL_ID: &str = "call_permission_fixture_1";
const WRITE_CONTENT: &str = "permission-fixture-write-once";
const WIRE_TIMEOUT: Duration = Duration::from_secs(12);
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(8);

static ENV_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
static SUITE_STARTED: OnceLock<Instant> = OnceLock::new();

struct EnvGuard {
    old: Vec<(&'static str, Option<String>)>,
    _home: TempDir,
    _data: TempDir,
}

impl EnvGuard {
    fn install(provider_url: &str, project_dir: &Path) -> Self {
        let home = tempdir().expect("fixture HOME");
        let data = tempdir().expect("fixture XDG_DATA_HOME");
        let values = [
            ("OPENAI_BASE_URL", Some(provider_url.to_owned())),
            ("OPENAI_API_KEY", Some("fixture-only-key".to_owned())),
            ("OPENCODE_RK_TURN_TOOLS", Some("write".to_owned())),
            ("OPENCODE_RK_TURN_MAX_STEPS", Some("2".to_owned())),
            (
                "OPENCODE_PROJECT_DIR",
                Some(project_dir.to_string_lossy().into_owned()),
            ),
            ("HOME", Some(home.path().to_string_lossy().into_owned())),
            (
                "XDG_DATA_HOME",
                Some(data.path().to_string_lossy().into_owned()),
            ),
            ("OPENCODE_AUTH_CONTENT", None),
        ];
        let old = values
            .iter()
            .map(|(key, _)| (*key, env::var(key).ok()))
            .collect();
        for (key, value) in values {
            if let Some(value) = value {
                env::set_var(key, value);
            } else {
                env::remove_var(key);
            }
        }
        Self {
            old,
            _home: home,
            _data: data,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.old {
            if let Some(value) = value {
                env::set_var(key, value);
            } else {
                env::remove_var(key);
            }
        }
    }
}

#[derive(Clone, Debug)]
struct WireResponse {
    status: u16,
    body: Vec<u8>,
}

fn decode_http(wire: &[u8]) -> Result<WireResponse, String> {
    if wire.len() > MAX_RESPONSE_WIRE_BYTES {
        return Err("HTTP response exceeds fixture wire bound".to_owned());
    }
    let end = wire
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "HTTP response header terminator missing".to_owned())?;
    let headers = std::str::from_utf8(&wire[..end]).map_err(|error| error.to_string())?;
    let status = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or_else(|| "HTTP status missing".to_owned())?
        .parse::<u16>()
        .map_err(|error| error.to_string())?;
    let body = &wire[end + 4..];
    if body.len() > MAX_RESPONSE_BYTES {
        return Err("HTTP response body exceeds fixture bounds".to_owned());
    }
    let chunked = headers.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("transfer-encoding")
                && value.trim().eq_ignore_ascii_case("chunked")
        })
    });
    if !chunked {
        return Ok(WireResponse {
            status,
            body: body.to_vec(),
        });
    }
    let mut cursor = 0usize;
    let mut decoded = Vec::new();
    loop {
        let line_end = body
            .get(cursor..)
            .and_then(|remaining| remaining.windows(2).position(|window| window == b"\r\n"))
            .map(|at| at + cursor)
            .ok_or_else(|| "HTTP chunk header missing".to_owned())?;
        let size = usize::from_str_radix(
            std::str::from_utf8(&body[cursor..line_end])
                .map_err(|error| error.to_string())?
                .split(';')
                .next()
                .unwrap_or_default(),
            16,
        )
        .map_err(|error| error.to_string())?;
        cursor = line_end + 2;
        if size == 0 {
            break;
        }
        if size > MAX_RESPONSE_BYTES.saturating_sub(decoded.len())
            || body.get(cursor..cursor.saturating_add(size + 2)).is_none()
        {
            return Err("HTTP chunk exceeds fixture bounds".to_owned());
        }
        decoded.extend_from_slice(&body[cursor..cursor + size]);
        cursor += size;
        if &body[cursor..cursor + 2] != b"\r\n" {
            return Err("HTTP chunk terminator missing".to_owned());
        }
        cursor += 2;
    }
    Ok(WireResponse {
        status,
        body: decoded,
    })
}

fn http_request(
    address: SocketAddr,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<&Value>,
) -> Result<WireResponse, String> {
    // macOS may briefly report ECONNREFUSED while the freshly-bound listener is
    // being handed to the runtime.  Retry only connection establishment, with
    // one absolute bound for the whole operation (not one bound per retry).
    let connect_deadline = Instant::now() + Duration::from_secs(5);
    let mut stream = loop {
        let remaining = connect_deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("HTTP connection absolute deadline".to_owned());
        }
        match TcpStream::connect_timeout(&address, remaining.min(Duration::from_millis(500))) {
            Ok(stream) => break stream,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionRefused
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::Interrupted
                ) =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error.to_string()),
        }
    };
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .map_err(|error| error.to_string())?;
    let payload = body.map(Value::to_string).unwrap_or_default();
    if payload.len() > MAX_REQUEST_BYTES {
        return Err("fixture request exceeds wire bound".to_owned());
    }
    let auth = token
        .map(|token| format!("authorization: Bearer {token}\r\n"))
        .unwrap_or_default();
    let content_type = body
        .map(|_| "content-type: application/json\r\n")
        .unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nhost: {address}\r\n{auth}{content_type}content-length: {}\r\nconnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| error.to_string())?;
    stream.flush().map_err(|error| error.to_string())?;
    let deadline = Instant::now() + WIRE_TIMEOUT;
    let mut wire = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        if Instant::now() >= deadline {
            return Err("HTTP response absolute deadline".to_owned());
        }
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                if count > MAX_RESPONSE_WIRE_BYTES.saturating_sub(wire.len()) {
                    return Err("HTTP response exceeds fixture wire bound".to_owned());
                }
                wire.extend_from_slice(&buffer[..count]);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) =>
            {
                thread::sleep(Duration::from_millis(2));
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    let parsed = decode_http(&wire)?;
    if parsed.body.len() > MAX_RESPONSE_BYTES {
        return Err("HTTP response body exceeds fixture bounds".to_owned());
    }
    Ok(parsed)
}

fn read_provider_request(stream: &mut TcpStream) -> Result<Value, String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut bytes = Vec::new();
    let mut expected = None;
    let mut buffer = [0u8; 4096];
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .map_err(|error| error.to_string())?;
    loop {
        if Instant::now() >= deadline {
            return Err("provider request absolute deadline".to_owned());
        }
        match stream.read(&mut buffer) {
            Ok(0) => return Err("provider request ended before body".to_owned()),
            Ok(count) => {
                if count > MAX_REQUEST_BYTES.saturating_sub(bytes.len()) {
                    return Err("provider request exceeds wire bound".to_owned());
                }
                bytes.extend_from_slice(&buffer[..count]);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) =>
            {
                thread::sleep(Duration::from_millis(2));
                continue;
            }
            Err(error) => return Err(error.to_string()),
        }
        if expected.is_none() {
            if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.split_once(':').and_then(|(name, value)| {
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                    })
                    .ok_or_else(|| "provider Content-Length missing".to_owned())?;
                if length > MAX_REQUEST_BYTES.saturating_sub(end + 4) {
                    return Err("provider content length exceeds wire bound".to_owned());
                }
                expected = Some(end + 4 + length);
            }
        }
        if expected.is_some_and(|length| bytes.len() >= length) {
            let end = bytes
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .ok_or_else(|| "provider HTTP headers missing".to_owned())?;
            return serde_json::from_slice(&bytes[end + 4..end + 4 + expected.unwrap() - end - 4])
                .map_err(|error| format!("provider JSON: {error}"));
        }
    }
}

fn write_sse(stream: &mut TcpStream, first: bool, target: &Path) -> Result<(), String> {
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| error.to_string())?;
    stream
        .write_all(
            b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
        )
        .map_err(|error| error.to_string())?;
    if first {
        let args = json!({"path":target,"content":WRITE_CONTENT}).to_string();
        let call = json!({
            "type":"response.output_item.done",
            "item":{"type":"function_call","id":"fc_permission_fixture_1",
                "call_id":CALL_ID,"name":"write","arguments":args}
        });
        let event = format!(
            "event: response.output_item.done\ndata: {call}\n\nevent: response.completed\ndata: {{\"type\":\"response.completed\",\"response\":{{\"status\":\"completed\"}}}}\n\n"
        );
        stream
            .write_all(event.as_bytes())
            .map_err(|error| error.to_string())?;
    } else {
        stream
            .write_all(b"event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"permission turn settled\"}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n")
            .map_err(|error| error.to_string())?;
    }
    stream.flush().map_err(|error| error.to_string())
}

struct Provider {
    address: SocketAddr,
    requests: Arc<Mutex<Vec<Value>>>,
    target: Arc<Mutex<PathBuf>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Provider {
    fn start(target: &Path) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind provider fixture");
        listener
            .set_nonblocking(true)
            .expect("provider listener nonblocking");
        let address = listener.local_addr().expect("provider fixture address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let worker_requests = Arc::clone(&requests);
        let target = Arc::new(Mutex::new(target.to_path_buf()));
        let worker_target = Arc::clone(&target);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            let deadline = Instant::now() + PROVIDER_TIMEOUT;
            let mut served = 0usize;
            while served < 2 && Instant::now() < deadline && !worker_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        if stream.set_nonblocking(false).is_err() {
                            break;
                        }
                        let parsed = match read_provider_request(&mut stream) {
                            Ok(parsed) => parsed,
                            Err(error) => {
                                worker_requests
                                    .lock()
                                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                                    .push(json!({"_fixture_error":error}));
                                break;
                            }
                        };
                        worker_requests
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .push(parsed);
                        let target = worker_target
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .clone();
                        if write_sse(&mut stream, served == 0, &target).is_err() {
                            break;
                        }
                        served += 1;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            address,
            requests,
            target,
            stop,
            thread: Some(thread),
        }
    }

    fn set_target(&self, target: &Path) {
        *self
            .target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = target.to_path_buf();
    }

    fn base_url(&self) -> String {
        format!("http://{}/v1", self.address)
    }

    fn count(&self) -> usize {
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    fn captured(&self) -> Vec<Value> {
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    async fn wait_for(&self, count: usize) -> bool {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if self.count() >= count {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        self.count() >= count
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Provider {
    fn drop(&mut self) {
        self.stop();
    }
}

struct ServerTask(Option<tokio::task::JoinHandle<()>>);

impl ServerTask {
    async fn stop(&mut self) {
        if let Some(server) = self.0.take() {
            server.abort();
            let _ = server.await;
        }
    }
}

impl Drop for ServerTask {
    fn drop(&mut self) {
        if let Some(server) = self.0.take() {
            server.abort();
        }
    }
}

struct Fixture {
    address: SocketAddr,
    token: String,
    session: String,
    other_session: String,
    app: axum::Router,
    server: ServerTask,
    provider: Provider,
    turn: Option<thread::JoinHandle<Result<WireResponse, String>>>,
    workspace: TempDir,
    target: PathBuf,
    _storage: TempDir,
    _env: EnvGuard,
    _env_lock: tokio::sync::MutexGuard<'static, ()>,
}

impl Fixture {
    async fn start() -> Result<Self, String> {
        let env_lock = ENV_LOCK
            .get_or_init(|| tokio::sync::Mutex::new(()))
            .lock()
            .await;
        let suite_start = *SUITE_STARTED.get_or_init(Instant::now);
        if suite_start.elapsed() > MAX_SUITE_DURATION {
            return Err("permission suite exceeded 90-second bound".to_owned());
        }
        let workspace = tempdir().map_err(|error| error.to_string())?;
        let target = workspace.path().join("approved.txt");
        let provider = Provider::start(&target);
        let env = EnvGuard::install(&provider.base_url(), workspace.path());
        let storage_dir = match tempdir() {
            Ok(directory) => directory,
            Err(error) => {
                drop(env);
                let mut provider = provider;
                provider.stop();
                return Err(error.to_string());
            }
        };
        let storage = match Storage::open_in_memory(storage_dir.path().join("blobs")) {
            Ok(storage) => storage,
            Err(error) => {
                drop(env);
                let mut provider = provider;
                provider.stop();
                return Err(error.to_string());
            }
        };
        let sessions = SessionService::new(Arc::new(storage));
        let auth = match DaemonAuth::mint() {
            Ok(auth) => auth,
            Err(error) => {
                drop(env);
                let mut provider = provider;
                provider.stop();
                return Err(error.to_string());
            }
        };
        let token = auth.token().to_owned();
        let app = router_with_auth(
            AppState {
                sessions,
                catalog: Arc::new(Catalog::default()),
            },
            Some(auth),
        );
        let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(error) => {
                drop(env);
                let mut provider = provider;
                provider.stop();
                return Err(error.to_string());
            }
        };
        let address = match listener.local_addr() {
            Ok(address) => address,
            Err(error) => {
                drop(env);
                let mut provider = provider;
                provider.stop();
                return Err(error.to_string());
            }
        };
        let server_app = app.clone();
        let server = tokio::spawn(async move {
            axum::serve(listener, server_app)
                .await
                .expect("permission fixture server");
        });
        let session = match create_session(&app, &token, "permission fixture").await {
            Ok(session) => session,
            Err(error) => {
                server.abort();
                let _ = server.await;
                drop(env);
                let mut provider = provider;
                provider.stop();
                return Err(error);
            }
        };
        let other_session = match create_session(&app, &token, "other permission fixture").await {
            Ok(session) => session,
            Err(error) => {
                server.abort();
                let _ = server.await;
                drop(env);
                let mut provider = provider;
                provider.stop();
                return Err(error);
            }
        };
        Ok(Self {
            address,
            token,
            session,
            other_session,
            app,
            server: ServerTask(Some(server)),
            provider,
            turn: None,
            workspace,
            target,
            _storage: storage_dir,
            _env: env,
            _env_lock: env_lock,
        })
    }

    fn start_turn(&mut self, target: &Path) {
        self.provider.set_target(target);
        let address = self.address;
        let token = self.token.clone();
        let session = self.session.clone();
        self.turn = Some(thread::spawn(move || {
            http_request(
                address,
                "POST",
                &format!("/api/sessions/{session}/turns/stream"),
                Some(&token),
                Some(&json!({
                    "text":"write the disposable permission fixture",
                    "model":MODEL,
                    "reasoning_effort":"low"
                })),
            )
        }));
    }

    fn turn_finished(&self) -> bool {
        self.turn
            .as_ref()
            .is_some_and(thread::JoinHandle::is_finished)
    }

    fn finish_turn(&mut self) -> Result<WireResponse, String> {
        let Some(turn) = self.turn.take() else {
            return Err("turn client was not started".to_owned());
        };
        turn.join()
            .map_err(|_| "turn client thread panicked".to_owned())?
    }

    async fn request(
        &self,
        method: &str,
        path: String,
        token: Option<String>,
        body: Option<Value>,
    ) -> Result<WireResponse, String> {
        let address = self.address;
        let method = method.to_owned();
        tokio::task::spawn_blocking(move || {
            http_request(address, &method, &path, token.as_deref(), body.as_ref())
        })
        .await
        .map_err(|error| error.to_string())?
    }

    async fn pending(&self) -> Result<(String, Value), String> {
        let deadline = Instant::now() + Duration::from_secs(3);
        let path = format!("/api/session/{}/permission", self.session);
        loop {
            let response = self
                .request("GET", path.clone(), Some(self.token.clone()), None)
                .await?;
            if response.status == 200 {
                let value: Value = serde_json::from_slice(&response.body)
                    .map_err(|error| format!("permission list JSON: {error}"))?;
                let entries = value
                    .as_array()
                    .ok_or_else(|| "session permission list must be a JSON array".to_owned())?;
                if let Some(request) = entries.first() {
                    let id = request["id"]
                        .as_str()
                        .ok_or_else(|| "pending permission ID missing".to_owned())?;
                    return Ok((id.to_owned(), request.clone()));
                }
            } else if response.status != 404 {
                return Err(format!(
                    "session permission list status {}, expected 200 or initial 404",
                    response.status
                ));
            } else {
                return Err("live session permission list route missing (404)".to_owned());
            }
            if Instant::now() >= deadline {
                return Err("permission request did not become pending".to_owned());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    async fn reply(
        &self,
        session: &str,
        request: &str,
        reply: &str,
        token: Option<String>,
    ) -> Result<WireResponse, String> {
        self.request(
            "POST",
            format!("/api/session/{session}/permission/{request}/reply"),
            token,
            Some(json!({"reply":reply})),
        )
        .await
    }

    async fn create_session(&self, title: &str) -> Result<String, String> {
        create_session(&self.app, &self.token, title).await
    }

    async fn finish(mut self) {
        self.server.stop().await;
        if let Some(turn) = self.turn.take() {
            let _ = tokio::task::spawn_blocking(move || turn.join()).await;
        }
        self.provider.stop();
    }

    fn broker_requires_human_for_target(&self, target: &Path) -> Result<(), String> {
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        if target.starts_with(&root) {
            return Err(format!(
                "fixture target {target:?} is inside server project root {root:?}"
            ));
        }
        let broker = PermissionBroker::new(SecurityPolicy::lean_default(root));
        let decision = broker.authorize(&OperationIntent::File {
            action: FileAction::Write,
            path: target.to_path_buf(),
        });
        if matches!(decision, Decision::RequireHuman { .. }) {
            Ok(())
        } else {
            Err(format!(
                "fixture write should require human approval, got {decision:?}"
            ))
        }
    }

    async fn live_once(&mut self) -> Result<(), String> {
        self.broker_requires_human_for_target(&self.target)?;
        if self
            .target
            .starts_with(std::env::current_dir().map_err(|error| error.to_string())?)
        {
            return Err(
                "permission target must be outside the server current-directory root".to_owned(),
            );
        }
        let target = self.target.clone();
        self.start_turn(&target);
        let (id, request) = self.pending().await?;
        if self.target.exists() {
            return Err("pending request already wrote target".to_owned());
        }
        if self.turn_finished() {
            return Err("provider turn continued before reply".to_owned());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        if self.provider.count() != 1 {
            return Err(format!(
                "provider continued while permission was pending: {} rounds",
                self.provider.count()
            ));
        }
        if request["sessionID"] != self.session
            || request["action"] != "write"
            || request["resources"][0] != self.target.to_string_lossy().as_ref()
            || request["source"]["type"] != "tool"
            || request["source"]["callID"] != CALL_ID
        {
            return Err(format!(
                "permission request lost session/action/resource/source: {request}"
            ));
        }
        let approved = self
            .reply(&self.session, &id, "once", Some(self.token.clone()))
            .await?;
        if approved.status != 204 {
            return Err(format!(
                "approval reply status {}, expected 204",
                approved.status
            ));
        }
        let response = self.finish_turn()?;
        if response.status != 201 {
            return Err(format!(
                "turn HTTP status {}, expected 201",
                response.status
            ));
        }
        if std::fs::read_to_string(&self.target).map_err(|error| error.to_string())?
            != WRITE_CONTENT
        {
            return Err("approved write did not execute exactly as requested".to_owned());
        }
        if !self.provider.wait_for(2).await {
            return Err("approved write did not resume provider continuation".to_owned());
        }
        let requests = self.provider.captured();
        let items = requests
            .get(1)
            .and_then(|request| request["input"].as_array())
            .ok_or_else(|| "provider continuation input missing".to_owned())?;
        let call = items
            .iter()
            .find(|item| item["type"] == "function_call" && item["call_id"] == CALL_ID);
        let output = items
            .iter()
            .find(|item| item["type"] == "function_call_output" && item["call_id"] == CALL_ID);
        if call.is_none()
            || !output.is_some_and(|item| item["output"].as_str() == Some("write success"))
        {
            return Err(format!(
                "typed tool call/result missing from continuation: {items:?}"
            ));
        }
        let duplicate = self
            .reply(&self.session, &id, "once", Some(self.token.clone()))
            .await?;
        if duplicate.status != 404 {
            return Err(format!(
                "duplicate reply status {}, expected 404",
                duplicate.status
            ));
        }
        if self.provider.count() != 2
            || std::fs::read_to_string(&self.target).ok().as_deref() != Some(WRITE_CONTENT)
        {
            return Err("duplicate reply replayed a side effect or provider round".to_owned());
        }
        Ok(())
    }

    async fn rejection(&mut self) -> Result<(), String> {
        self.broker_requires_human_for_target(&self.target)?;
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        if self.target.starts_with(root) {
            return Err(
                "permission target must be outside the current server project root".to_owned(),
            );
        }
        let target = self.target.clone();
        self.start_turn(&target);
        let (id, _) = self.pending().await?;
        if self.target.exists() || self.turn_finished() || self.provider.count() != 1 {
            return Err(
                "pending rejection already caused a side effect or continuation".to_owned(),
            );
        }
        let rejected = self
            .reply(&self.session, &id, "reject", Some(self.token.clone()))
            .await?;
        if rejected.status != 204 {
            return Err(format!(
                "rejection reply status {}, expected 204",
                rejected.status
            ));
        }
        let response = self.finish_turn()?;
        let events = ndjson_events(&response.body)?;
        let result = events
            .iter()
            .find(|event| event["type"] == "tool_output" && event["call_id"] == CALL_ID)
            .ok_or_else(|| format!("rejection did not settle the typed tool call: {events:?}"))?;
        let output = result["output"].as_str().unwrap_or_default();
        if !output.to_ascii_lowercase().contains("reject")
            && !output.to_ascii_lowercase().contains("denied")
        {
            return Err(format!(
                "rejection tool result was not a denial: {output:?}"
            ));
        }
        if self.target.exists() || self.provider.count() != 1 {
            return Err("rejected operation wrote a file or resumed the provider".to_owned());
        }
        Ok(())
    }

    async fn auth_failure(&mut self) -> Result<(), String> {
        self.broker_requires_human_for_target(&self.target)?;
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        if self.target.starts_with(root) {
            return Err(
                "permission target must be outside the current server project root".to_owned(),
            );
        }
        let target = self.target.clone();
        self.start_turn(&target);
        let (id, _) = self.pending().await?;
        let missing = self.reply(&self.session, &id, "once", None).await?;
        let wrong = self
            .reply(
                &self.session,
                &id,
                "once",
                Some("not-the-daemon-token".to_owned()),
            )
            .await?;
        if missing.status != 401 || wrong.status != 403 {
            return Err(format!(
                "missing/wrong bearer statuses {}, {}",
                missing.status, wrong.status
            ));
        }
        if self.target.exists() || self.provider.count() != 1 || self.turn_finished() {
            return Err("unauthenticated reply consumed request or caused effects".to_owned());
        }
        let rejected = self
            .reply(&self.session, &id, "reject", Some(self.token.clone()))
            .await?;
        if rejected.status != 204 {
            return Err("valid cleanup reply did not consume pending request".to_owned());
        }
        let _ = self.finish_turn()?;
        if self.target.exists() || self.provider.count() != 1 {
            return Err("unauthenticated reply changed tool/provider state".to_owned());
        }
        Ok(())
    }

    async fn session_and_replay(&mut self) -> Result<(), String> {
        self.broker_requires_human_for_target(&self.target)?;
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        if self.target.starts_with(root) {
            return Err(
                "permission target must be outside the current server project root".to_owned(),
            );
        }
        let target = self.target.clone();
        self.start_turn(&target);
        let (id, _) = self.pending().await?;
        let wrong_session = self
            .reply(&self.other_session, &id, "once", Some(self.token.clone()))
            .await?;
        let stale = self
            .reply(
                &self.session,
                "per_fixture_stale",
                "once",
                Some(self.token.clone()),
            )
            .await?;
        if wrong_session.status != 404 || stale.status != 404 {
            return Err(format!(
                "wrong-session/stale reply statuses {}, {}",
                wrong_session.status, stale.status
            ));
        }
        if self.target.exists() || self.turn_finished() || self.provider.count() != 1 {
            return Err("wrong-session/stale reply consumed or executed request".to_owned());
        }
        let approved = self
            .reply(&self.session, &id, "once", Some(self.token.clone()))
            .await?;
        if approved.status != 204 {
            return Err("valid scoped approval failed".to_owned());
        }
        let _ = self.finish_turn()?;
        let duplicate = self
            .reply(&self.session, &id, "once", Some(self.token.clone()))
            .await?;
        if duplicate.status != 404 || self.provider.count() != 2 {
            return Err("stale duplicate replayed an approved operation".to_owned());
        }
        if std::fs::read_to_string(&self.target).ok().as_deref() != Some(WRITE_CONTENT) {
            return Err("session-scoped approval did not write exact fixture bytes".to_owned());
        }
        Ok(())
    }

    async fn protected_secret(&mut self) -> Result<(), String> {
        let secret = self.workspace.path().join(".env");
        let broker = PermissionBroker::new(SecurityPolicy::lean_default(self.workspace.path()))
            .with_permissions(PermissionSet::star());
        let intent = OperationIntent::File {
            action: FileAction::Write,
            path: secret.clone(),
        };
        if !matches!(broker.authorize(&intent), Decision::Deny { .. }) {
            return Err("wildcard permission bypassed mandatory secret protection".to_owned());
        }
        let tool = FileTool::with_project_root(self.workspace.path());
        let outcome = tool
            .execute_authorized(
                FileOperation::write(&secret, "fixture-secret".to_owned()),
                &broker,
            )
            .map_err(|error| error.to_string())?;
        if outcome.success || secret.exists() {
            return Err("mandatory protected write caused filesystem side effect".to_owned());
        }
        self.start_turn(&secret);
        let response = self.finish_turn()?;
        let events = ndjson_events(&response.body)?;
        if secret.exists() {
            return Err("live protected write created fixture .env".to_owned());
        }
        let second_round = self.provider.wait_for(2).await;
        if !second_round {
            return Err("protected denial did not settle through fixture provider".to_owned());
        }
        let requests = self.provider.captured();
        let items = requests
            .get(1)
            .and_then(|request| request["input"].as_array())
            .ok_or_else(|| "protected denial continuation input missing".to_owned())?;
        if !items.iter().any(|item| {
            item["type"] == "function_call_output"
                && item["call_id"] == CALL_ID
                && item["output"]
                    .as_str()
                    .is_some_and(|value| value.to_ascii_lowercase().contains("denied"))
        }) {
            return Err(format!(
                "mandatory denial result missing from typed continuation: {items:?}"
            ));
        }
        let forged = self
            .reply(
                &self.session,
                "per_fixture_forged",
                "once",
                Some(self.token.clone()),
            )
            .await?;
        if forged.status != 404 {
            return Err(format!(
                "forged permission reply status {}, expected 404",
                forged.status
            ));
        }
        if events.iter().all(|event| event["type"] != "tool_output") {
            return Err(format!(
                "protected operation did not settle a tool result: {events:?}"
            ));
        }
        Ok(())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(turn) = self.turn.take() {
            let _ = turn.join();
        }
        self.provider.stop();
    }
}

async fn create_session(app: &axum::Router, token: &str, title: &str) -> Result<String, String> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"title":title}).to_string()))
                .map_err(|error| error.to_string())?,
        )
        .await
        .map_err(|error| error.to_string())?;
    if response.status() != StatusCode::CREATED {
        return Err(format!("create session status {}", response.status()));
    }
    let bytes = to_bytes(response.into_body(), 16 * 1024)
        .await
        .map_err(|error| error.to_string())?;
    let body: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    body["session"]["id"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "session response missing ID".to_owned())
}

fn ndjson_events(body: &[u8]) -> Result<Vec<Value>, String> {
    std::str::from_utf8(body)
        .map_err(|error| error.to_string())?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(|error| error.to_string()))
        .collect()
}

async fn execute_case(case: impl for<'a> AsyncCase<'a>) {
    let mut fixture = Fixture::start().await.expect("fixture setup");
    let result = tokio::time::timeout(MAX_CASE_DURATION, case.run(&mut fixture))
        .await
        .unwrap_or_else(|_| Err("permission scenario exceeded 20-second bound".to_owned()));
    fixture.finish().await;
    result.unwrap_or_else(|error| panic!("{error}"));
}

trait AsyncCase<'a> {
    fn run(
        self,
        fixture: &'a mut Fixture,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + 'a>>;
}

struct OnceCase;
impl<'a> AsyncCase<'a> for OnceCase {
    fn run(
        self,
        fixture: &'a mut Fixture,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + 'a>> {
        Box::pin(fixture.live_once())
    }
}
struct RejectCase;
impl<'a> AsyncCase<'a> for RejectCase {
    fn run(
        self,
        fixture: &'a mut Fixture,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + 'a>> {
        Box::pin(fixture.rejection())
    }
}
struct AuthCase;
impl<'a> AsyncCase<'a> for AuthCase {
    fn run(
        self,
        fixture: &'a mut Fixture,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + 'a>> {
        Box::pin(fixture.auth_failure())
    }
}
struct ReplayCase;
impl<'a> AsyncCase<'a> for ReplayCase {
    fn run(
        self,
        fixture: &'a mut Fixture,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + 'a>> {
        Box::pin(fixture.session_and_replay())
    }
}
struct ProtectedCase;
impl<'a> AsyncCase<'a> for ProtectedCase {
    fn run(
        self,
        fixture: &'a mut Fixture,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + 'a>> {
        Box::pin(fixture.protected_secret())
    }
}

#[tokio::test]
async fn live_permission_pauses_and_once_resumes_exactly_once() {
    execute_case(OnceCase).await;
}

#[tokio::test]
async fn live_permission_reject_settles_without_side_effect_or_continuation() {
    execute_case(RejectCase).await;
}

#[tokio::test]
async fn live_permission_auth_failures_preserve_pending_request() {
    execute_case(AuthCase).await;
}

#[tokio::test]
async fn live_permission_is_session_bound_and_reply_is_one_shot() {
    execute_case(ReplayCase).await;
}

#[tokio::test]
async fn live_permission_wildcard_and_forged_reply_cannot_approve_secret_write() {
    execute_case(ProtectedCase).await;
}
