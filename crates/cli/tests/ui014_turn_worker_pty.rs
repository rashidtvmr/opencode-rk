#![forbid(unsafe_code)]
//! UI-014 real-binary RED: an owned turn worker must keep the PTY responsive.
//!
//! This test deliberately uses the shipped `oc2` executable and a disposable
//! authenticated daemon. The provider fixture closes the first request without
//! a response, making the POST outcome ambiguous. A correct caller processes
//! the second prompt and `:i` while the first request is in flight, then
//! joins/cancels that request without replaying either side effect.
//!
//! RED at the current revision: `tui_entry::interactive_loop` performs the
//! provider request synchronously, so the second prompt and interrupt remain
//! unread until the fixture is released. The later request then demonstrates
//! the missing queue/interrupt ownership.

use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const MAX_REQUEST_BYTES: usize = 128 * 1024;
const MAX_CAPTURE_BYTES: usize = 64 * 1024;
const MAX_DIAGNOSTIC_BYTES: usize = 4096;
const MAX_HANDLERS: usize = 8;
const READY_TIMEOUT: Duration = Duration::from_secs(10);
const INPUT_TIMEOUT: Duration = Duration::from_secs(2);
const EXIT_TIMEOUT: Duration = Duration::from_secs(8);
static NEXT_HOME: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
struct RequestRecord {
    ordinal: usize,
    prompt_digest: String,
    request_bytes: usize,
}

#[derive(Clone, Default)]
struct FixtureState {
    records: Vec<RequestRecord>,
    errors: Vec<String>,
    active: usize,
    max_active: usize,
    release_first: bool,
    shutdown: bool,
    closed_without_response: usize,
}

struct FixtureShared {
    state: Mutex<FixtureState>,
    changed: Condvar,
}

impl FixtureShared {
    fn record_error(&self, error: String) {
        let mut state = self.state.lock().expect("fixture state poisoned");
        if state.errors.len() < MAX_HANDLERS {
            state.errors.push(error);
        }
        self.changed.notify_all();
    }

    fn wait_for_requests(&self, count: usize, timeout: Duration) -> Result<Vec<RequestRecord>, String> {
        let deadline = Instant::now() + timeout;
        let mut state = self.state.lock().expect("fixture state poisoned");
        loop {
            if state.records.len() >= count {
                return Ok(state.records.clone());
            }
            if !state.errors.is_empty() {
                return Err(state.errors.join("; "));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!(
                    "provider fixture saw {} request(s), expected {count}",
                    state.records.len()
                ));
            }
            let (next, result) = self.changed.wait_timeout(state, remaining).expect("fixture wait poisoned");
            state = next;
            if result.timed_out() && state.records.len() < count {
                return Err(format!(
                    "provider fixture saw {} request(s), expected {count}",
                    state.records.len()
                ));
            }
        }
    }

}

struct ProviderFixture {
    base_url: String,
    shared: Arc<FixtureShared>,
    listener: Option<thread::JoinHandle<()>>,
}

impl ProviderFixture {
    fn start() -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|error| format!("bind provider fixture: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("set provider fixture nonblocking: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("provider fixture address: {error}"))?;
        let shared = Arc::new(FixtureShared {
            state: Mutex::new(FixtureState::default()),
            changed: Condvar::new(),
        });
        let worker_shared = Arc::clone(&shared);
        let listener_thread = thread::spawn(move || {
            let mut handlers = Vec::new();
            loop {
                let shutdown = worker_shared
                    .state
                    .lock()
                    .expect("fixture state poisoned")
                    .shutdown;
                if shutdown {
                    break;
                }
                match listener.accept() {
                    Ok((stream, _)) => {
                        if handlers.len() >= MAX_HANDLERS {
                            worker_shared.record_error("provider fixture handler bound exceeded".to_owned());
                            break;
                        }
                        let handler_shared = Arc::clone(&worker_shared);
                        handlers.push(thread::spawn(move || {
                            let result = handle_provider_request(stream, Arc::clone(&handler_shared));
                            if let Err(error) = result {
                                // EOF after the test-owned close is expected only after recording.
                                if !error.contains("provider fixture request ended") {
                                    handler_shared.record_error(error);
                                }
                            }
                        }));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => {
                        worker_shared.record_error(format!("provider fixture accept: {error}"));
                        break;
                    }
                }
            }
            for handler in handlers {
                let _ = handler.join();
            }
        });
        Ok(Self {
            base_url: format!("http://{address}/v1"),
            shared,
            listener: Some(listener_thread),
        })
    }

    fn wait_for_request(&self, count: usize) -> Result<Vec<RequestRecord>, String> {
        self.shared.wait_for_requests(count, INPUT_TIMEOUT)
    }

    fn release_first(&self) {
        let mut state = self.shared.state.lock().expect("fixture state poisoned");
        state.release_first = true;
        self.shared.changed.notify_all();
    }

    fn records(&self) -> Vec<RequestRecord> {
        self.shared
            .state
            .lock()
            .expect("fixture state poisoned")
            .records
            .clone()
    }

    fn state(&self) -> FixtureState {
        self.shared
            .state
            .lock()
            .expect("fixture state poisoned")
            .clone()
    }

    fn shutdown(&self) {
        let mut state = self.shared.state.lock().expect("fixture state poisoned");
        state.shutdown = true;
        state.release_first = true;
        self.shared.changed.notify_all();
    }
}

impl Drop for ProviderFixture {
    fn drop(&mut self) {
        self.shutdown();
        if let Some(listener) = self.listener.take() {
            let _ = listener.join();
        }
    }
}

fn handle_provider_request(mut stream: TcpStream, shared: Arc<FixtureShared>) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("provider fixture read timeout: {error}"))?;
    let body = read_http_body(&mut stream)?;
    let prompt = latest_user_prompt(&body)?;
    let ordinal = {
        let mut state = shared.state.lock().expect("fixture state poisoned");
        state.active += 1;
        state.max_active = state.max_active.max(state.active);
        let ordinal = state.records.len() + 1;
        state.records.push(RequestRecord {
            ordinal,
            prompt_digest: digest(prompt.as_bytes()),
            request_bytes: body.len(),
        });
        let ordinal = state.records.len();
        shared.changed.notify_all();
        ordinal
    };

    if ordinal == 1 {
        let deadline = Instant::now() + EXIT_TIMEOUT;
        let mut state = shared.state.lock().expect("fixture state poisoned");
        while !state.release_first && !state.shutdown {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let (next, _) = shared
                .changed
                .wait_timeout(state, remaining)
                .expect("fixture wait poisoned");
            state = next;
        }
    }

    // No response: the client cannot know whether the side effect committed.
    drop(stream);
    let mut state = shared.state.lock().expect("fixture state poisoned");
    state.active = state.active.saturating_sub(1);
    state.closed_without_response += 1;
    shared.changed.notify_all();
    Ok(())
}

fn read_http_body(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    let mut raw = Vec::new();
    let mut chunk = [0_u8; 4096];
    let (header_end, content_length) = loop {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("provider fixture read: {error}"))?;
        if read == 0 {
            return Err("provider fixture request ended before headers".to_owned());
        }
        if raw.len() + read > MAX_REQUEST_BYTES {
            return Err("provider fixture request exceeded byte bound".to_owned());
        }
        raw.extend_from_slice(&chunk[..read]);
        if let Some(header_end) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&raw[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.trim()
                        .eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .ok_or_else(|| "provider fixture request lacked content-length".to_owned())?;
            if header_end + 4 + content_length > MAX_REQUEST_BYTES {
                return Err("provider fixture body exceeded byte bound".to_owned());
            }
            break (header_end, content_length);
        }
    };
    while raw.len() < header_end + 4 + content_length {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("provider fixture body read: {error}"))?;
        if read == 0 {
            return Err("provider fixture request ended before body".to_owned());
        }
        if raw.len() + read > MAX_REQUEST_BYTES {
            return Err("provider fixture request exceeded byte bound".to_owned());
        }
        raw.extend_from_slice(&chunk[..read]);
    }
    Ok(raw[header_end + 4..header_end + 4 + content_length].to_vec())
}

fn latest_user_prompt(body: &[u8]) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_slice(body)
        .map_err(|error| format!("provider request JSON: {error}"))?;
    let input = value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "provider request lacked input array".to_owned())?;
    input
        .iter()
        .rev()
        .find_map(|item| {
            (item.get("role").and_then(serde_json::Value::as_str) == Some("user"))
                .then(|| item.get("content").and_then(serde_json::Value::as_str))
                .flatten()
                .map(ToOwned::to_owned)
        })
        .ok_or_else(|| "provider request lacked user prompt".to_owned())
}

fn digest(bytes: &[u8]) -> String {
    // Bounded, deterministic fixture identity. Raw request bytes are never retained or logged.
    let mut first = 0xcbf29ce484222325_u64;
    let mut second = 0x84222325cbf29ce4_u64;
    for byte in bytes {
        first ^= u64::from(*byte);
        first = first.wrapping_mul(0x100000001b3);
        second ^= u64::from(*byte).rotate_left(7);
        second = second.wrapping_mul(0x100000001b3);
    }
    format!("{first:016x}{second:016x}")
}

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let id = NEXT_HOME.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "opencode-rk-ui014-pty-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(path.join("home")).expect("create disposable UI-014 home");
        fs::create_dir_all(path.join("data")).expect("create disposable UI-014 data");
        Self(path)
    }

    fn home(&self) -> PathBuf {
        self.0.join("home")
    }

    fn data(&self) -> PathBuf {
        self.0.join("data")
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Default)]
struct CaptureState {
    bytes: Vec<u8>,
    truncated: bool,
}

struct Capture {
    shared: Arc<Mutex<CaptureState>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Capture {
    fn spawn(mut pipe: impl Read + Send + 'static) -> Self {
        let shared = Arc::new(Mutex::new(CaptureState::default()));
        let reader_shared = Arc::clone(&shared);
        let handle = thread::spawn(move || {
            let mut chunk = [0_u8; 4096];
            loop {
                let read = match pipe.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => read,
                };
                let mut state = match reader_shared.lock() {
                    Ok(state) => state,
                    Err(_) => break,
                };
                if state.bytes.len() + read > MAX_CAPTURE_BYTES {
                    let keep = MAX_CAPTURE_BYTES.saturating_sub(read.min(MAX_CAPTURE_BYTES));
                    let remove = state.bytes.len().saturating_sub(keep);
                    state.bytes.drain(..remove);
                    state.truncated = true;
                }
                state.bytes.extend_from_slice(&chunk[..read]);
            }
        });
        Self {
            shared,
            handle: Some(handle),
        }
    }

    fn text(&self) -> String {
        let state = match self.shared.lock() {
            Ok(state) => state,
            Err(_) => return "<capture unavailable>".to_owned(),
        };
        let mut text = String::from_utf8_lossy(&state.bytes).into_owned();
        text = redacted(&text);
        if state.truncated {
            text.insert_str(0, "[capture truncated]\n");
        }
        text
    }

    fn join_bounded(&mut self) {
        let Some(handle) = self.handle.take() else {
            return;
        };
        let deadline = Instant::now() + Duration::from_millis(250);
        while !handle.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        if handle.is_finished() {
            let _ = handle.join();
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.join_bounded();
    }
}

fn redacted(text: &str) -> String {
    text.replace("fixture-secret", "<redacted>")
        .chars()
        .take(MAX_DIAGNOSTIC_BYTES)
        .collect()
}

struct PtyProcess {
    child: Child,
    stdin: std::process::ChildStdin,
    output: Capture,
    stderr: Capture,
}

impl PtyProcess {
    fn spawn(binary: &Path, args: &[String], envs: &[(&str, String)]) -> Result<Self, String> {
        if !Path::new("/usr/bin/script").is_file() {
            return Err("UI-014 setup blocker: /usr/bin/script is unavailable".to_owned());
        }
        let mut command = Command::new("/usr/bin/script");
        command.args(["-q", "/dev/null"]).arg(binary).args(args);
        command.env_clear();
        for (key, value) in envs {
            command.env(key, value);
        }
        command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|error| format!("UI-014 setup blocker: spawn /usr/bin/script: {error}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "UI-014 setup blocker: PTY stdin unavailable".to_owned())?;
        let output = Capture::spawn(
            child
                .stdout
                .take()
                .ok_or_else(|| "UI-014 setup blocker: PTY stdout unavailable".to_owned())?,
        );
        let stderr = Capture::spawn(
            child
                .stderr
                .take()
                .ok_or_else(|| "UI-014 setup blocker: PTY stderr unavailable".to_owned())?,
        );
        Ok(Self {
            child,
            stdin,
            output,
            stderr,
        })
    }

    fn send(&mut self, text: &str) -> Result<(), String> {
        self.stdin
            .write_all(text.as_bytes())
            .map_err(|error| format!("write PTY input: {error}"))?;
        self.stdin
            .flush()
            .map_err(|error| format!("flush PTY input: {error}"))
    }

    fn wait_for(&self, needle: &str, timeout: Duration) -> Result<String, String> {
        let deadline = Instant::now() + timeout;
        loop {
            let output = self.output.text();
            if output.contains(needle) {
                return Ok(output);
            }
            if Instant::now() >= deadline {
                return Err(format!("PTY output did not contain {needle:?} within {timeout:?}"));
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> Result<std::process::ExitStatus, String> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) => {}
                Err(error) => return Err(format!("PTY try_wait: {error}")),
            }
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                let _ = self.child.wait();
                return Err("PTY child did not exit within timeout".to_owned());
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn diagnostics(&self) -> String {
        format!(
            "stdout:\n{}\nstderr:\n{}",
            self.output.text(),
            self.stderr.text()
        )
    }
}

impl Drop for PtyProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.output.join_bounded();
        self.stderr.join_bounded();
    }
}

struct DaemonProcess {
    child: Child,
    stderr: Capture,
}

impl DaemonProcess {
    fn spawn(binary: &Path, data: &Path, home: &Path, address: &str, provider: &str) -> Result<Self, String> {
        let mut command = Command::new(binary);
        command
            .args(["--data-dir", data.to_str().unwrap_or_default(), "serve", "--listen", address])
            .env_clear()
            .env("HOME", home)
            .env("OPENCODE_RK_HOME", home)
            .env("OPENAI_BASE_URL", provider)
            .env("OPENAI_API_KEY", "fixture-secret")
            .env("OPENAI_TIMEOUT_SECS", "5")
            .env("OPENAI_MAX_TOKENS", "64")
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("RUST_LOG", "error")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|error| format!("UI-014 setup blocker: spawn oc2 serve: {error}"))?;
        let stderr = Capture::spawn(
            child
                .stderr
                .take()
                .ok_or_else(|| "UI-014 setup blocker: daemon stderr unavailable".to_owned())?,
        );
        Ok(Self { child, stderr })
    }

    fn wait_ready(&mut self, address: &str, timeout: Duration) -> Result<(), String> {
        let socket: SocketAddr = address
            .parse()
            .map_err(|error| format!("UI-014 setup blocker: daemon address: {error}"))?;
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Ok(mut stream) = TcpStream::connect_timeout(&socket, Duration::from_millis(200)) {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                if stream
                    .write_all(b"GET /health HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n")
                    .is_ok()
                {
                    let mut response = [0_u8; 1024];
                    if let Ok(read) = stream.read(&mut response) {
                        if response[..read].starts_with(b"HTTP/1.1 200") {
                            return Ok(());
                        }
                    }
                }
            }
            thread::sleep(Duration::from_millis(25));
        }
        Err(format!(
            "UI-014 setup blocker: oc2 serve did not become healthy; stderr={}",
            self.stderr.text()
        ))
    }
}

impl Drop for DaemonProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.stderr.join_bounded();
    }
}

fn oc2_binary() -> PathBuf {
    std::env::var_os("OC2_E2E_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_oc2")))
}

fn session_id(binary: &Path, data: &Path, home: &Path) -> Result<String, String> {
    let mut command = Command::new(binary);
    command
        .args(["--data-dir", data.to_str().unwrap_or_default(), "session", "create", "UI014 PTY"])
        .env_clear()
        .env("HOME", home)
        .env("OPENCODE_RK_HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|error| format!("UI-014 setup blocker: spawn oc2 session create: {error}"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "UI-014 setup blocker: session stdout unavailable".to_owned())?;
    let mut bytes = Vec::new();
    stdout
        .by_ref()
        .take((MAX_CAPTURE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("UI-014 setup blocker: read session output: {error}"))?;
    let status = child
        .wait()
        .map_err(|error| format!("UI-014 setup blocker: wait session create: {error}"))?;
    if !status.success() || bytes.len() > MAX_CAPTURE_BYTES {
        return Err("UI-014 setup blocker: oc2 session create failed".to_owned());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("UI-014 setup blocker: session JSON: {error}"))?;
    value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "UI-014 setup blocker: session create returned no id".to_owned())
}

fn loopback_address() -> Result<String, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("UI-014 setup blocker: reserve daemon address: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("UI-014 setup blocker: daemon address: {error}"))?;
    Ok(address.to_string())
}

#[test]
fn ui014_turn_worker_pty_keeps_input_live_and_does_not_replay_ambiguous_turn() {
    let binary = oc2_binary();
    let root = TestRoot::new();
    let provider = ProviderFixture::start().expect("UI-014 setup blocker: provider fixture");
    let address = loopback_address().expect("UI-014 setup blocker: loopback address");
    let session = session_id(&binary, &root.data(), &root.home()).expect("UI-014 setup blocker: session");
    let mut daemon = DaemonProcess::spawn(
        &binary,
        &root.data(),
        &root.home(),
        &address,
        &provider.base_url,
    )
    .expect("UI-014 setup blocker: daemon");
    daemon
        .wait_ready(&address, READY_TIMEOUT)
        .expect("UI-014 setup blocker: daemon readiness");

    let origin = format!("http://{address}");
    let args = vec![
        "--data-dir".to_owned(),
        root.data().to_string_lossy().into_owned(),
        "tui".to_owned(),
        "--origin".to_owned(),
        origin,
        "--session".to_owned(),
        session,
        "--model".to_owned(),
        "openai/gpt-5.6".to_owned(),
    ];
    let envs = vec![
        ("HOME", root.home().to_string_lossy().into_owned()),
        ("OPENCODE_RK_HOME", root.home().to_string_lossy().into_owned()),
        ("OPENCODE_RK_DAEMON_ADDR", address.clone()),
        ("TERM", "xterm-256color".to_owned()),
        ("PATH", "/usr/bin:/bin".to_owned()),
        ("LANG", "C".to_owned()),
        ("LC_ALL", "C".to_owned()),
        ("RUST_LOG", "error".to_owned()),
    ];
    let mut pty = PtyProcess::spawn(&binary, &args, &envs)
        .expect("UI-014 setup blocker: PTY process");
    pty.wait_for("OpenCode RK TUI", READY_TIMEOUT)
        .expect("UI-014 setup blocker: real oc2 PTY did not render");

    pty.send("first-prompt\n")
        .expect("write first prompt");
    let first = provider
        .wait_for_request(1)
        .unwrap_or_else(|error| panic!("UI-014 setup/caller did not reach delayed provider POST: {error}"));
    assert_eq!(first.len(), 1, "first provider request count");
    assert_eq!(first[0].prompt_digest, digest(b"first-prompt"));
    assert!(first[0].request_bytes <= MAX_REQUEST_BYTES);

    // These bytes must be consumed while request one is still unresolved.
    pty.send("second-prompt\n")
        .expect("write second prompt");
    pty.send(":i\n").expect("write interrupt");
    pty.send(":q\n").expect("write exit");
    let responsive_output = pty.wait_for("second-prompt", INPUT_TIMEOUT).unwrap_or_default();
    let processed_before_release = responsive_output.contains("[queued] second-prompt")
        || responsive_output.contains("[interrupted");
    let request_count_before_release = provider.records().len();

    provider.release_first();
    let status = pty
        .wait_for_exit(EXIT_TIMEOUT)
        .unwrap_or_else(|error| panic!("UI-014 caller cleanup failure: {error}\n{}", pty.diagnostics()));
    let records = provider.records();
    let state = provider.state();
    let diagnostics = pty.diagnostics();

    assert!(status.success(), "oc2 PTY exit status: {status}\n{diagnostics}");
    assert!(
        processed_before_release,
        "UI-014 RED: real oc2 caller did not process queued input/interrupt before delayed POST completed; before={request_count_before_release}, records={records:?}, max_active={}, closed_without_response={}\n{diagnostics}",
        state.max_active,
        state.closed_without_response
    );
    assert_eq!(
        request_count_before_release, 1,
        "UI-014 RED: caller started a parallel request before the first completed\n{diagnostics}"
    );
    assert_eq!(
        records.len(),
        1,
        "UI-014 RED: ambiguous first POST was replayed; records={records:?}\n{diagnostics}"
    );
    assert_eq!(records[0].prompt_digest, digest(b"first-prompt"));
    assert!(state.closed_without_response >= 1);
    assert!(state.max_active <= 1, "provider observed parallel requests");
    assert!(state.errors.is_empty(), "fixture errors: {:?}", state.errors);
}
