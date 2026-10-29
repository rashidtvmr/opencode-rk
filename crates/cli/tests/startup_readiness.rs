#![forbid(unsafe_code)]
//! APP-017: black-box checks of the no-subcommand CLI startup decision.
//!
//! These tests invoke the actual `opencode-rk` binary with stdin/stdout attached
//! to a real PTY.  The loopback listener below speaks HTTP; it is not a mocked
//! CLI lease.  In particular, the tests observe actual requests and bearer
//! headers sent by the binary.  All data lives below a unique disposable root.

use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

// Keep the driver's combined code and transcript below ordinary pipe capacity.
const MAX_CAPTURE: usize = 32 * 1024;
const PYTHON_PATH: &str = "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin";
const TEST_TOKEN: &str = "ab";

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

struct FixtureRoot(PathBuf);

impl FixtureRoot {
    fn new(label: &str) -> Self {
        let id = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "opencode-rk-app017-{label}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create disposable APP-017 root");
        Self(path)
    }

    fn data_dir(&self) -> &Path {
        &self.0
    }

    fn descriptor_path(&self) -> PathBuf {
        self.0.join("runtime/backend.json")
    }

    fn publish(&self, origin: &str, pid: u32, token: &str, schema: u16) {
        let path = self.descriptor_path();
        fs::create_dir_all(path.parent().expect("runtime parent")).expect("create runtime dir");
        let descriptor = serde_json::json!({
            "pid": pid,
            "http_origin": origin,
            "schema_version": schema,
            "auth_token": token,
        });
        fs::write(path, descriptor.to_string()).expect("write synthetic descriptor");
    }
}

impl Drop for FixtureRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Debug)]
struct ObservedRequest {
    method: String,
    path: String,
    authorization: Option<String>,
}

/// A real loopback HTTP peer. `/health` is deliberately public; API requests
/// are recorded exactly as received and only the requested synthetic bearer is
/// accepted. The listener and thread have explicit stop/join ownership.
struct HttpFixture {
    address: String,
    health_status: u16,
    stop: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<ObservedRequest>>>,
    worker: Option<thread::JoinHandle<()>>,
    proof_origin: String,
    proof_pid: u32,
    proof_token: String,
}

impl HttpFixture {
    fn start() -> Self {
        Self::start_with_health(200)
    }

    fn start_with_health(health_status: u16) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback HTTP fixture");
        listener
            .set_nonblocking(true)
            .expect("nonblocking fixture accept");
        let address = listener.local_addr().expect("fixture address").to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let worker_stop = Arc::clone(&stop);
        let worker_requests = Arc::clone(&requests);
        let proof_origin = format!("http://{address}");
        let proof_pid = std::process::id();
        let proof_token = TEST_TOKEN.repeat(32);
        let worker_proof_origin = proof_origin.clone();
        let worker_proof_token = proof_token.clone();
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => serve_one(
                        &mut stream,
                        &worker_requests,
                        health_status,
                        &worker_proof_origin,
                        proof_pid,
                        &worker_proof_token,
                    ),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            address,
            health_status,
            stop,
            requests,
            worker: Some(worker),
            proof_origin,
            proof_pid,
            proof_token,
        }
    }

    fn origin(&self) -> String {
        format!("http://{}", self.address)
    }

    fn requests(&self) -> Vec<ObservedRequest> {
        self.requests.lock().expect("request log lock").clone()
    }

    fn api_requests(&self) -> Vec<ObservedRequest> {
        self.requests()
            .into_iter()
            .filter(|request| request.path.starts_with("/api/"))
            .collect()
    }

    fn health_requests(&self) -> Vec<ObservedRequest> {
        self.requests()
            .into_iter()
            .filter(|request| request.path == "/health")
            .collect()
    }

    fn discovery_requests(&self) -> Vec<ObservedRequest> {
        self.requests()
            .into_iter()
            .filter(|request| request.path.starts_with("/discovery-proof?nonce="))
            .collect()
    }

    fn unexpected_api_requests(&self) -> Vec<ObservedRequest> {
        self.api_requests().into_iter().filter(|request| {
            !matches!(
                (request.method.as_str(), request.path.as_str()),
                ("POST", "/api/sessions")
                    | ("GET", "/api/sessions")
                    | ("GET", "/api/models?provider=openai&limit=500")
                    | ("GET", "/api/providers")
                    | ("GET", "/api/commands")
                    | ("GET", "/api/sessions/550e8400-e29b-41d4-a716-446655440000/history?limit=100")
                    | ("GET", "/api/sessions/550e8400-e29b-41d4-a716-446655440000/mcp")
                    | ("GET", "/api/sessions/550e8400-e29b-41d4-a716-446655440000/todo")
            )
        }).collect()
    }
    fn health_still_responds(&self) -> bool {
        let Ok(mut stream) = TcpStream::connect_timeout(
            &self.address.parse().expect("fixture socket address"),
            Duration::from_millis(300),
        ) else {
            return false;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
        if write!(
            stream,
            "GET /health HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            self.address
        )
        .is_err()
        {
            return false;
        }
        let mut response = Vec::new();
        stream.take(2048).read_to_end(&mut response).is_ok()
            && response.starts_with(format!("HTTP/1.1 {} ", self.health_status).as_bytes())
    }
}

impl Drop for HttpFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Wake the nonblocking listener so shutdown doesn't depend on a sleep.
        let _ = TcpStream::connect(&self.address);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn decode_discovery_nonce(value: &str) -> Option<Vec<u8>> {
    if value.len() != 64 || !value.is_ascii() {
        return None;
    }
    value.as_bytes().chunks_exact(2).map(|pair| {
        let high = match pair[0] {
            b'0'..=b'9' => pair[0] - b'0',
            b'a'..=b'f' => pair[0] - b'a' + 10,
            b'A'..=b'F' => pair[0] - b'A' + 10,
            _ => return None,
        };
        let low = match pair[1] {
            b'0'..=b'9' => pair[1] - b'0',
            b'a'..=b'f' => pair[1] - b'a' + 10,
            b'A'..=b'F' => pair[1] - b'A' + 10,
            _ => return None,
        };
        Some((high << 4) | low)
    }).collect()
}


fn serve_one(
    stream: &mut TcpStream,
    log: &Mutex<Vec<ObservedRequest>>,
    health_status: u16,
    proof_origin: &str,
    proof_pid: u32,
    proof_token: &str,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 2048];
    let header_end = loop {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(count) => {
                bytes.extend_from_slice(&chunk[..count]);
                if bytes.len() > 16 * 1024 {
                    return;
                }
                if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                    break end;
                }
            }
        }
    };
    let headers = String::from_utf8_lossy(&bytes[..header_end]);
    let mut lines = headers.lines();
    let request_line = lines.next().unwrap_or_default();
    let mut fields = request_line.split_whitespace();
    let method = fields.next().unwrap_or_default().to_owned();
    let path = fields.next().unwrap_or_default().to_owned();
    let authorization = lines.find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("authorization")
            .then(|| value.trim().to_owned())
    });
    log.lock().expect("request log lock").push(ObservedRequest {
        method: method.clone(),
        path: path.clone(),
        authorization: authorization.clone(),
    });

    let (status, body) = if path == "/health" && health_status == 200 {
        ("200 OK", r#"{"status":"ok"}"#.to_owned())
    } else if path == "/health" {
        ("503 Service Unavailable", r#"{"status":"unavailable"}"#.to_owned())
    } else if let Some(nonce_hex) = path.strip_prefix("/discovery-proof?nonce=") {
        match decode_discovery_nonce(nonce_hex).and_then(|nonce| {
            opencode_rk_server::discovery_proof::mac(
                proof_token,
                &nonce,
                proof_origin,
                1,
                proof_pid,
            ).map(|mac| serde_json::json!({
                "schema_version": 1,
                "pid": proof_pid,
                "http_origin": proof_origin,
                "nonce": nonce_hex,
                "mac": mac,
            }).to_string())
        }) {
            Some(body) => ("200 OK", body),
            None => ("400 Bad Request", r#"{"message":"invalid discovery nonce"}"#.to_owned()),
        }
    } else if authorization.as_deref()
        == Some("Bearer ".to_owned() + &TEST_TOKEN.repeat(32)).as_deref()
        && path.starts_with("/api/")
    {
        // Explicit startup graph only. Unknown authenticated paths never get
        // a plausible success envelope: they are observable 404s.
        const SESSION_ID: &str = "550e8400-e29b-41d4-a716-446655440000";
        const SESSION_TITLE: &str = "APP017 fixture session";
        let (status, body) = match (method.as_str(), path.as_str()) {
            ("POST", "/api/sessions") => (
                "201 Created",
                serde_json::json!({"session": {"id": SESSION_ID, "title": SESSION_TITLE, "state": "idle", "updated_at": "2026-01-01T00:00:00Z"}}).to_string(),
            ),
            ("GET", "/api/sessions") => (
                "200 OK",
                serde_json::json!({"sessions": [{"id": SESSION_ID, "title": SESSION_TITLE, "state": "idle", "updated_at": "2026-01-01T00:00:00Z", "directory": "/tmp"}]}).to_string(),
            ),
            ("GET", "/api/models?provider=openai&limit=500") => (
                "200 OK",
                serde_json::json!({"schema_version": 1, "source": "models.dev", "count": 0, "models": []}).to_string(),
            ),
            ("GET", "/api/providers") => (
                "200 OK",
                serde_json::json!({"providers": []}).to_string(),
            ),
            ("GET", "/api/commands") => (
                "200 OK",
                serde_json::json!({"agents": [], "commands": []}).to_string(),
            ),
            ("GET", "/api/sessions/550e8400-e29b-41d4-a716-446655440000/history?limit=100") => (
                "200 OK", r#"{"messages":[],"next_before":null}"#.to_owned(),
            ),
            ("GET", "/api/sessions/550e8400-e29b-41d4-a716-446655440000/mcp") => (
                "200 OK", r#"{}"#.to_owned(),
            ),
            ("GET", "/api/sessions/550e8400-e29b-41d4-a716-446655440000/todo") => (
                "200 OK", r#"{"todos":[]}"#.to_owned(),
            ),
            ("POST", "/api/models?provider=openai&limit=500")
            | ("POST", "/api/providers") | ("POST", "/api/commands") => (
                "405 Method Not Allowed", r#"{"message":"method not allowed"}"#.to_owned(),
            ),
            _ => ("404 Not Found", r#"{"message":"not found"}"#.to_owned()),
        };
        (status, body)
    } else {
        ("401 Unauthorized", r#"{"message":"unauthorized"}"#.to_owned())
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    // The accept worker is joined by HttpFixture::Drop; bound both sides of
    // every socket before any response write so a peer that stops reading
    // cannot strand teardown indefinitely.
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// Runs the actual no-subcommand binary in a PTY. Python is only the stdlib
/// PTY adapter; all launch, descriptor reads, health probes, requests and
/// lifecycle ownership under test execute in the compiled Rust CLI binary.
/// The child environment is reconstructed from a tiny explicit allowlist and
/// the PTY transcript is byte-capped. Rust launches the Python supervisor as
/// an owned setsid process-group leader; the native child shares that group.
/// A 25-second Python alarm and independent outer bound govern cleanup.
fn run_default_cli(root: &FixtureRoot, address: &str) -> (i32, String, String) {
    const DRIVER: &str = r#"
import errno, os, pty, select, signal, subprocess, sys, time

os.setsid()
binary, data_dir, home, address = sys.argv[1:]
child = None
master = None
slave = None
captured = bytearray()
ready_marker = b"OpenCode RK"
capture_limit = 32768

def infra(message):
    sys.stderr.write("PTY_INFRA_BLOCKER: " + message + "\n")
    sys.stderr.flush()
    raise SystemExit(90)

def alarm(_signum, _frame):
    infra("PTY driver reached its 25-second hard alarm")

signal.signal(signal.SIGALRM, alarm)
signal.alarm(25)
try:
    try:
        # Rust launches this Python supervisor through setsid, making the
        # supervisor PID the live process-group leader. The native child is
        # deliberately Popen'd without a new session, so it remains in that
        # same owned group; Rust can verify PGID==child.id() before signalling.
        master, slave = pty.openpty()
        env = {
            "HOME": home,
            "OPENCODE_RK_HOME": data_dir,
            "OPENCODE_RK_DAEMON_ADDR": address,
            "PATH": "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin",
            "TMPDIR": home,
        }
        child = subprocess.Popen(
            [binary, "--data-dir", data_dir],
            stdin=slave,
            stdout=slave,
            stderr=slave,
            env=env,
            close_fds=True,
        )
        # Install the supervisor-only TERM policy after spawning. The native
        # child inherited the default disposition and can still terminate;
        # this supervisor ignores the group TERM it sends during final cleanup
        # so it can drain/reap without recursively terminating itself.
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        os.close(slave)
        slave = None
    except BaseException as error:
        infra("PTY open/Popen failed: " + repr(error))

    # Await output from the actual CLI before sending commands. This prevents
    # empty transcripts from satisfying the negative attachment assertions.
    # chat::spawn_daemon has a ten-second readiness bound; allow one extra
    # second for the CLI banner after successful child publication.
    ready_deadline = time.monotonic() + 11.0
    while ready_marker not in captured and time.monotonic() < ready_deadline:
        ready, _, _ = select.select([master], [], [], 0.05)
        if ready:
            try:
                block = os.read(master, 4096)
            except OSError as error:
                if error.errno == errno.EIO:
                    block = b""
                else:
                    infra("PTY readiness read failed: " + repr(error))
            if block:
                if len(captured) + len(block) > capture_limit:
                    infra("PTY transcript exceeded 32768-byte cap before banner")
                captured.extend(block)
        child_status = child.poll()
        if child_status is not None:
            # Preserve the bounded transcript when the executable exits before
            # readiness. This is diagnostic evidence, never acceptance: the
            # caller still raises an infrastructure blocker with the actual
            # exit status and captured native output.
            rendered = bytes(captured[:capture_limit]).decode("utf-8", "replace")
            infra("CLI exited before startup banner; status=" + repr(child_status)
                  + "\nCAPTURED_PTY_TRANSCRIPT_BEGIN\n" + rendered
                  + "\nCAPTURED_PTY_TRANSCRIPT_END")
    if ready_marker not in captured:
        infra("PTY_NOT_READY: CLI startup banner absent within eleven seconds")

    # Submit each native command with the active Enter mapping (CR).  LF is
    # ordinary composer Newline in the native loop, so it must not be used to
    # submit /sessions or the exit route.
    os.write(master, b"/sessions\r")

    # Observe the actual Sessions-page frame before dismissing it.  This keeps
    # the authenticated GET /api/sessions and its rendered result in the test
    # path rather than racing a second command into the composer.  The marker
    # is intentionally broad native page content; the byte/output bounds below
    # remain authoritative if a broken page emits nothing.
    page_deadline = time.monotonic() + 5.0
    while b"Sessions" not in captured and time.monotonic() < page_deadline:
        ready, _, _ = select.select([master], [], [], 0.05)
        if ready:
            try:
                block = os.read(master, 4096)
            except OSError as error:
                if error.errno == errno.EIO:
                    block = b""
                else:
                    infra("PTY Sessions-page read failed: " + repr(error))
            if block:
                if len(captured) + len(block) > capture_limit:
                    infra("PTY transcript exceeded 32768-byte cap while observing Sessions page")
                captured.extend(block)
    if b"Sessions" not in captured:
        infra("PTY_SESSIONS_NOT_OBSERVED: native Sessions page/result absent within five seconds")

    # A standalone ESC is decoded only after the decoder's quiet timeout; wait
    # 50 ms before submitting the process exit command so it cannot be folded
    # into an escape sequence or share the Sessions-page input frame.
    os.write(master, b"\x1b")
    time.sleep(0.05)
    os.write(master, b":q\r")
    deadline = time.monotonic() + 10.5
    status = None
    while time.monotonic() < deadline:
        ready, _, _ = select.select([master], [], [], 0.05)
        if ready:
            try:
                block = os.read(master, 4096)
            except OSError as error:
                if error.errno == errno.EIO:
                    block = b""
                else:
                    raise
            if block:
                if len(captured) + len(block) > capture_limit:
                    infra("PTY transcript exceeded 32768-byte cap")
                captured.extend(block)
        child_status = child.poll()
        if child_status is not None:
            status = child_status
            break
    if status is None:
        infra("CLI did not exit after Sessions-page dismissal and :q within 10.5 seconds")

    # Drain the PTY after waitpid: the child may exit while its final output is
    # still buffered in the PTY master. Stop at EOF/EIO or a bounded quiet read.
    drain_deadline = time.monotonic() + 0.5
    while time.monotonic() < drain_deadline:
        ready, _, _ = select.select([master], [], [], 0.025)
        if not ready:
            continue
        try:
            block = os.read(master, 4096)
        except OSError as error:
            if error.errno == errno.EIO:
                break
            infra("PTY drain failed: " + repr(error))
        if not block:
            break
        if len(captured) + len(block) > capture_limit:
            infra("PTY transcript exceeded 32768-byte cap during drain")
        captured.extend(block)
    code = status
    sys.stdout.buffer.write((str(code) + "\n").encode() + captured)
    sys.stdout.buffer.flush()
finally:
    signal.alarm(0)
    if child is not None:
        try:
            if child.poll() is None:
                child.kill()
            child.wait(timeout=2.0)
        except (ProcessLookupError, subprocess.TimeoutExpired):
            pass
        try:
            # The CLI may have spawned an owned daemon. The supervisor is the
            # live group leader; after the direct child is reaped, terminate
            # remaining group members without self-recursion, then bound the
            # final drain. The real T08 daemon is never in this group.
            own_group = os.getpgrp() == os.getpid() and os.getsid(0) == os.getpid()
            if own_group:
                group = os.getpgrp()
                os.killpg(group, signal.SIGTERM)
                time.sleep(0.05)
                # Never SIGKILL the group leader: it is this live supervisor.
                # Re-enumerate the live group and kill only remaining children
                # by identity, excluding our own PID; no stale PID ledger is
                # retained or trusted.
                listing = subprocess.run(
                    ["ps", "-axo", "pid=,pgid="],
                    capture_output=True,
                    text=True,
                    timeout=1.0,
                ).stdout
                own = os.getpid()
                for line in listing.splitlines():
                    fields = line.split()
                    if len(fields) == 2 and fields[1] == str(group):
                        try:
                            member = int(fields[0])
                        except ValueError:
                            continue
                        if member > 1 and member != own:
                            try:
                                if os.getpgid(member) == group:
                                    os.kill(member, signal.SIGKILL)
                            except ProcessLookupError:
                                pass
                # Recheck live membership after KILL, while this supervisor
                # still anchors the session and excludes itself from cleanup.
                cleanup_deadline = time.monotonic() + 2.0
                while True:
                    listing = subprocess.run(
                        ["/bin/ps", "-axo", "pid=,pgid="],
                        capture_output=True, text=True, timeout=1.0, check=True,
                    ).stdout
                    remaining = []
                    for line in listing.splitlines():
                        fields = line.split()
                        if len(fields) != 2 or fields[1] != str(group):
                            continue
                        member = int(fields[0])
                        if member <= 1 or member == own:
                            continue
                        try:
                            if os.getpgid(member) == group:
                                remaining.append(member)
                        except ProcessLookupError:
                            pass
                    if not remaining:
                        break
                    if time.monotonic() >= cleanup_deadline:
                        infra("owned PTY descendants remain after bounded cleanup")
                    time.sleep(0.025)
        except (ProcessLookupError, subprocess.TimeoutExpired):
            pass
    if slave is not None:
        try:
            os.close(slave)
        except OSError:
            pass
    if master is not None:
        try:
            os.close(master)
        except OSError:
            pass
"#;

    let binary = env!("CARGO_BIN_EXE_opencode-rk");
    let mut driver = Command::new("/usr/bin/python3");
    driver
        .arg("-c")
        .arg(DRIVER)
        .arg(binary)
        .arg(root.data_dir())
        .arg(root.data_dir())
        .arg(address)
        .env_clear()
        .env("PATH", PYTHON_PATH)
        .current_dir(root.data_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = driver.spawn().expect("launch stdlib PTY driver");
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll PTY driver") {
            break status;
        }
        if Instant::now() >= deadline {
            // `setsid` makes this direct Rust child the live group leader;
            // verify that identity from the kernel process table before using
            // a negative-PID group signal. Never trust a descriptor/PID file.
            let group_is_owned = Command::new("ps")
                .args(["-o", "pgid=", "-p", &child.id().to_string()])
                .output()
                .ok()
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .and_then(|value| value.trim().parse::<u32>().ok())
                .is_some_and(|pgid| pgid == child.id())
                && Command::new("/usr/bin/python3")
                    .args(["-c", "import os,sys; print(os.getsid(int(sys.argv[1])))", &child.id().to_string()])
                    .output()
                    .ok()
                    .and_then(|output| String::from_utf8(output.stdout).ok())
                    .and_then(|value| value.trim().parse::<u32>().ok())
                    .is_some_and(|sid| sid == child.id());
            if group_is_owned {
                let _ = Command::new("kill")
                    .args(["-KILL", "--", &format!("-{}", child.id())])
                    .status();
            }
            let _ = child.kill();
            let _ = child.wait();
            panic!("PTY driver exceeded 30-second outer bound");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let output = child
        .wait_with_output()
        .expect("collect bounded PTY driver output");
    if status.code() == Some(90) {
        panic!(
            "infrastructure blocker: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(
        status.success(),
        "PTY driver failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.len() <= MAX_CAPTURE + 16,
        "outer PTY output bound"
    );
    let split = output
        .stdout
        .iter()
        .position(|byte| *byte == b'\n')
        .expect("PTY driver exit code line");
    let code = String::from_utf8_lossy(&output.stdout[..split])
        .trim()
        .parse::<i32>()
        .expect("PTY child exit code");
    let transcript = String::from_utf8_lossy(&output.stdout[split + 1..]).into_owned();
    let driver_stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (code, transcript, driver_stderr)
}

fn assert_no_token_output(transcript: &str) {
    assert!(
        !transcript.contains(&TEST_TOKEN.repeat(32)),
        "descriptor bearer must never appear in PTY output"
    );
}

#[test]
fn app017_t01_matching_descriptor_and_healthy_peer_attach_with_its_bearer() {
    let root = FixtureRoot::new("valid");
    let fixture = HttpFixture::start();
    let origin = fixture.origin();
    root.publish(&origin, std::process::id(), &TEST_TOKEN.repeat(32), 1);

    let (code, transcript, driver_stderr) = run_default_cli(&root, &fixture.address);

    assert_eq!(
        code, 0,
        "real no-subcommand CLI should exit cleanly: {transcript}"
    );
    assert!(
        transcript.contains("Sessions") && transcript.contains("APP017 fixture session"),
        "native authenticated Sessions table must render the fixture row after attach: {transcript}"
    );
    assert!(!fixture.health_requests().is_empty(), "fixture must observe public readiness probe");
    assert!(!fixture.discovery_requests().is_empty(), "fixture must observe signed discovery proof");
    let descriptor: serde_json::Value = serde_json::from_slice(&fs::read(root.descriptor_path()).expect("read attached descriptor")).expect("valid attached descriptor");
    assert_eq!(descriptor["http_origin"].as_str(), Some(origin.as_str()));
    assert_eq!(descriptor["pid"].as_u64(), Some(u64::from(std::process::id())));
    assert_no_token_output(&transcript);
    assert_no_token_output(&driver_stderr);
    let api = fixture.api_requests();
    assert!(
        !api.is_empty(),
        "the actual CLI must reach the fixture API after attach"
    );
    assert!(
        api.iter().all(|request| request.authorization.as_deref()
            == Some(format!("Bearer {}", TEST_TOKEN.repeat(32)).as_str())),
        "every API request must carry only the descriptor bearer: {api:?}"
    );
    assert!(fixture.unexpected_api_requests().is_empty(), "unexpected authenticated startup routes: {:?}", fixture.unexpected_api_requests());
}

#[test]
fn app017_t02_healthy_foreign_listener_without_descriptor_is_not_attached_or_killed() {
    let root = FixtureRoot::new("foreign");
    let fixture = HttpFixture::start();

    let (code, transcript, driver_stderr) = run_default_cli(&root, &fixture.address);

    assert_eq!(
        code, 0,
        "foreign listener is a refusal, not a CLI crash: {transcript}"
    );
    assert!(
        !transcript.contains(&format!("daemon: {}", fixture.origin())),
        "public liveness alone must not yield an attached lease: {transcript}"
    );
    // With no validated descriptor, native bootstrap must not route even a
    // public-health request to this foreign listener. The listener-survival
    // probe below is the explicit non-destructive observation.
    assert!(
        fixture.health_requests().is_empty(),
        "descriptor-less foreign listener must not receive startup probes"
    );
    assert!(
        fixture.api_requests().is_empty(),
        "no descriptor means no API activity"
    );
    assert!(
        fixture.health_still_responds(),
        "refusal must leave the foreign listener alive"
    );
    assert_no_token_output(&driver_stderr);
}

#[test]
fn app017_t03_bad_descriptor_shapes_never_authorize_foreign_api_requests() {
    let cases = [
        "empty-token",
        "malformed-token",
        "malformed-json",
        "oversized",
        "schema-mismatch",
        "origin-mismatch",
        "symlink",
    ];
    for case in cases {
        let root = FixtureRoot::new(case);
        let configured = HttpFixture::start();
        let other = HttpFixture::start();
        let configured_origin = configured.origin();
        match case {
            "empty-token" => root.publish(&configured_origin, std::process::id(), "", 1),
            "malformed-token" => root.publish(&configured_origin, std::process::id(), "not-hex", 1),
            "malformed-json" => {
                let path = root.descriptor_path();
                fs::create_dir_all(path.parent().expect("runtime parent")).expect("runtime dir");
                fs::write(path, b"{ this is not valid JSON").expect("malformed descriptor");
            }
            "oversized" => {
                let path = root.descriptor_path();
                fs::create_dir_all(path.parent().expect("runtime parent")).expect("runtime dir");
                fs::write(path, vec![b'x'; 8 * 1024 + 1]).expect("oversized descriptor");
            }
            "schema-mismatch" => root.publish(
                &configured_origin,
                std::process::id(),
                &TEST_TOKEN.repeat(32),
                77,
            ),
            "origin-mismatch" => root.publish(
                &other.origin(),
                std::process::id(),
                &TEST_TOKEN.repeat(32),
                1,
            ),
            "symlink" => {
                let path = root.descriptor_path();
                fs::create_dir_all(path.parent().expect("runtime parent")).expect("runtime dir");
                let target = root.data_dir().join("descriptor-target.json");
                fs::write(
                    &target,
                    serde_json::json!({
                        "pid": std::process::id(),
                        "http_origin": configured_origin,
                        "schema_version": 1,
                        "auth_token": TEST_TOKEN.repeat(32),
                    })
                    .to_string(),
                )
                .expect("write symlink target");
                #[cfg(unix)]
                std::os::unix::fs::symlink(target, path).expect("create descriptor symlink");
                #[cfg(not(unix))]
                panic!("APP-017 symlink descriptor fixture requires Unix symlink semantics");
            }
            _ => unreachable!("closed fixture case list"),
        }

        let (code, transcript, driver_stderr) = run_default_cli(&root, &configured.address);
        assert_eq!(
            code, 0,
            "invalid descriptor {case} must fail closed: {transcript}"
        );
        assert!(
            !transcript.contains(&format!("daemon: {configured_origin}")),
            "invalid descriptor {case} must not be presented as an attached daemon: {transcript}"
        );
        // Invalid descriptor validation fails closed before native bootstrap
        // acquires an origin, so no health/API request is authorized for this
        // foreign or malformed target. Valid descriptor paths retain the
        // explicit public-health assertions below and above.
        assert!(
            configured.api_requests().is_empty(),
            "invalid descriptor {case} must not authorize API requests"
        );
        assert!(
            other.api_requests().is_empty(),
            "mismatched descriptor origin must never receive API requests"
        );
        assert_no_token_output(&transcript);
        assert_no_token_output(&driver_stderr);
    }
}

#[test]
fn app017_t07_wrong_owner_descriptor_is_refused_or_explicitly_classified_unsupported() {
    let root = FixtureRoot::new("wrong-owner");
    let fixture = HttpFixture::start();
    root.publish(
        &fixture.origin(),
        std::process::id(),
        &TEST_TOKEN.repeat(32),
        1,
    );

    // A real foreign-owner metadata fixture requires CAP_CHOWN/root. Do not
    // fake MetadataExt or silently skip it: perform chown only on this generated
    // descriptor and report the exact unsupported privilege classification.
    let mut owner_setup = Command::new("python3");
    owner_setup
        .arg("-c")
        .arg("import os,sys; p=sys.argv[1]; u=os.geteuid(); os.chown(p, 65534 if u != 65534 else 0, -1)")
        .arg(root.descriptor_path())
        .env_clear()
        .env("PATH", PYTHON_PATH)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let ownership = owner_setup.output().expect("run fixture-only chown helper");
    if !ownership.status.success() {
        let diagnostic = String::from_utf8_lossy(&ownership.stderr);
        assert!(
            diagnostic.contains("Permission denied")
                || diagnostic.contains("Operation not permitted"),
            "unexpected wrong-owner fixture setup failure (not a skip): {diagnostic}"
        );
        eprintln!(
            "APP017 WRONG-OWNER UNSUPPORTED: effective user cannot chown its disposable descriptor; chat uses server::daemon owner gate, unit-covered at daemon.rs::rc02_foreign_owner_refused"
        );
        assert!(
            fixture.health_still_responds(),
            "unsupported fixture leaves listener intact"
        );
        assert!(
            fixture.api_requests().is_empty(),
            "unsupported fixture sends no API requests"
        );
        return;
    }

    let (code, transcript, driver_stderr) = run_default_cli(&root, &fixture.address);
    assert_eq!(
        code, 0,
        "wrong-owner descriptor must fail closed: {transcript}"
    );
    assert!(
        !transcript.contains(&format!("daemon: {}", fixture.origin())),
        "wrong-owner descriptor must not establish an attached lease"
    );
    assert!(
        fixture.health_requests().is_empty(),
        "wrong-owner descriptor must not trigger a health probe to the foreign listener"
    );
    assert!(
        fixture.api_requests().is_empty(),
        "wrong-owner token must never authorize API traffic"
    );
    assert!(
        fixture.health_still_responds(),
        "wrong-owner refusal must not kill its listener"
    );
    assert_no_token_output(&transcript);
    assert_no_token_output(&driver_stderr);
}

#[test]
fn app017_t06_non_200_public_health_never_yields_an_authenticated_lease() {
    let root = FixtureRoot::new("unhealthy");
    let fixture = HttpFixture::start_with_health(503);
    root.publish(
        &fixture.origin(),
        std::process::id(),
        &TEST_TOKEN.repeat(32),
        1,
    );

    let (code, transcript, driver_stderr) = run_default_cli(&root, &fixture.address);

    assert_eq!(
        code, 0,
        "failed health must be handled as startup failure: {transcript}"
    );
    assert!(
        !transcript.contains(&format!("daemon: {}", fixture.origin())),
        "non-200 readiness must not attach an authenticated lease"
    );
    assert!(
        !fixture.health_requests().is_empty(),
        "CLI must receive the non-200 health response"
    );
    assert!(
        fixture.api_requests().is_empty(),
        "health failure must precede any API request"
    );
    assert!(
        fixture.health_still_responds(),
        "the pre-existing 503 listener must remain untouched"
    );
    assert_no_token_output(&transcript);
    assert_no_token_output(&driver_stderr);
}

#[test]
fn app017_t04_actual_spawn_waits_for_published_readiness_and_reaps_only_its_child() {
    let root = FixtureRoot::new("owned-start");
    let address = TcpListener::bind("127.0.0.1:0")
        .expect("reserve then release a unique loopback port")
        .local_addr()
        .expect("reserved address")
        .to_string();
    // The probe race is against the real `serve` child and descriptor publish,
    // not a mocked readiness future. No health server owns the port initially.
    drop(TcpListener::bind(&address).expect("reacquire reserved loopback address"));

    let (code, transcript, driver_stderr) = run_default_cli(&root, &address);

    assert_eq!(code, 0, "real child startup should complete: {transcript}");
    assert!(
        transcript.contains("OpenCode RK"),
        "real chat startup banner observed: {transcript}"
    );
    assert!(
        root.descriptor_path().is_file(),
        "owned daemon published its descriptor"
    );
    let descriptor: serde_json::Value = serde_json::from_slice(
        &fs::read(root.descriptor_path()).expect("read published descriptor"),
    )
    .expect("valid published descriptor");
    let pid = descriptor["pid"].as_u64().expect("published child PID") as u32;
    assert_ne!(pid, 0, "published daemon PID is positive");
    let actual_bearer = descriptor["auth_token"]
        .as_str()
        .expect("serve-minted bearer");
    assert!(
        !transcript.contains(actual_bearer),
        "actual daemon bearer must not appear in PTY output"
    );
    assert!(
        !driver_stderr.contains(actual_bearer),
        "actual daemon bearer must not appear in PTY-driver diagnostics"
    );
    // `chat::run` owns this child and kills/reaps it before it returns. A
    // post-exit health probe must therefore fail while the independent
    // synthetic fixture path above remains untouched.
    let socket = address.parse().expect("child loopback address");
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if TcpStream::connect_timeout(&socket, Duration::from_millis(100)).is_err() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "CLI exit must stop its owned daemon listener"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn app017_t05_valid_existing_service_is_reused_without_descriptor_replacement() {
    let root = FixtureRoot::new("reuse");
    let fixture = HttpFixture::start();
    let origin = fixture.origin();
    root.publish(&origin, std::process::id(), &TEST_TOKEN.repeat(32), 1);
    let before = fs::read(root.descriptor_path()).expect("pre-existing descriptor");

    let (code, transcript, driver_stderr) = run_default_cli(&root, &fixture.address);

    assert_eq!(
        code, 0,
        "existing healthy daemon must be reusable: {transcript}"
    );
    assert!(
        transcript.contains("Sessions") && transcript.contains("APP017 fixture session"),
        "native authenticated Sessions table must render the fixture row for reused service: {transcript}"
    );
    assert!(!fixture.discovery_requests().is_empty(), "reused service must answer signed discovery proof");
    assert!(!fixture.api_requests().is_empty(), "reused service must receive authenticated API traffic");
    assert_eq!(
        fs::read(root.descriptor_path()).expect("descriptor after CLI exit"),
        before,
        "attach must not republish/replace the existing daemon descriptor"
    );
    assert!(
        fixture.health_still_responds(),
        "attaching client must not stop a pre-existing service"
    );
    assert!(
        !fixture.health_requests().is_empty(),
        "fixture observed existing-service health probe"
    );
    assert!(
        !fixture.api_requests().is_empty(),
        "existing valid service saw real API requests"
    );
    assert!(fixture.unexpected_api_requests().is_empty(), "unexpected authenticated reuse routes: {:?}", fixture.unexpected_api_requests());
    assert_no_token_output(&transcript);
    assert_no_token_output(&driver_stderr);
}

/// Exercises two independent no-subcommand clients against one real `serve`
/// process whose lifetime is owned by this test, not either chat client.
/// Source contract: `main.rs::serve` publishes an authenticated descriptor and
/// runs `router_with_auth`; `chat.rs::bind_recent_session` prints the empty
/// sessions marker only after its authenticated GET /api/sessions succeeds.
/// This proves two-client reuse of this daemon/origin, not absence of unrelated
/// process starts elsewhere on the host.
fn authenticated_json_request(
    address: &str,
    token: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> String {
    let mut stream = TcpStream::connect(address).expect("connect owned serve for authenticated fixture request");
    stream.set_read_timeout(Some(Duration::from_secs(1))).expect("bound fixture request read");
    stream.set_write_timeout(Some(Duration::from_secs(1))).expect("bound fixture request write");
    let body = body.unwrap_or("");
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ).expect("write authenticated fixture request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read authenticated fixture response");
    response
}

#[test]
fn app017_t08_two_clients_reuse_one_live_authenticated_serve_daemon() {
    use std::process::Child;

    struct ServeGuard(Child);

    impl ServeGuard {
        fn stop(&mut self) {
            // Drop can run during an assertion unwind; cleanup must not panic
            // and turn a useful assertion into a double-panic abort.
            // On a polling error, still try to kill before the bounded-process
            // owner waits below; never assume an error means the child exited.
            if !matches!(self.0.try_wait(), Ok(Some(_))) {
                let _ = self.0.kill();
            }
            let _ = self.0.wait();
        }
    }

    impl Drop for ServeGuard {
        fn drop(&mut self) {
            self.stop();
        }
    }

    fn healthy(address: &str) -> bool {
        let Ok(socket) = address.parse() else {
            return false;
        };
        let Ok(mut stream) = TcpStream::connect_timeout(&socket, Duration::from_millis(200)) else {
            return false;
        };
        if stream
            .set_read_timeout(Some(Duration::from_millis(300)))
            .is_err()
        {
            return false;
        }
        if write!(
            stream,
            "GET /health HTTP/1.1\r\nhost: {address}\r\nconnection: close\r\n\r\n"
        )
        .is_err()
        {
            return false;
        }
        let mut response = Vec::new();
        stream.take(2048).read_to_end(&mut response).is_ok()
            && response.starts_with(b"HTTP/1.1 200 ")
    }

    let root = FixtureRoot::new("two-client-reuse");
    // Reserve an ephemeral loopback address, then release it for the real
    // server. The unique test data dir and descriptor PID below disambiguate
    // the explicitly owned process from any unrelated service.
    let reservation = TcpListener::bind("127.0.0.1:0").expect("reserve loopback address");
    let address = reservation
        .local_addr()
        .expect("reserved listener address")
        .to_string();
    drop(reservation);
    let origin = format!("http://{address}");

    let serve = Command::new(env!("CARGO_BIN_EXE_opencode-rk"))
        .env_clear()
        .env("HOME", root.data_dir())
        .env("TMPDIR", root.data_dir())
        .env("PATH", PYTHON_PATH)
        .arg("--data-dir")
        .arg(root.data_dir())
        .arg("serve")
        .arg("--listen")
        .arg(&address)
        .current_dir(root.data_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn explicitly-owned real serve process");
    let serve_pid = serve.id();
    let mut serve = ServeGuard(serve);

    // Read only descriptor limit + 1 bytes; reject excess before parsing or
    // retaining an attacker-controlled file of arbitrary size.
    let descriptor_path = root.descriptor_path();
    let deadline = Instant::now() + Duration::from_secs(10);
    let (descriptor_before, descriptor): (Vec<u8>, serde_json::Value) = loop {
        assert!(
            serve
                .0
                .try_wait()
                .expect("poll owned serve process")
                .is_none(),
            "real serve exited before becoming ready"
        );
        if let Ok(file) = fs::File::open(&descriptor_path) {
            let mut bytes = Vec::with_capacity(8 * 1024 + 1);
            file.take((8 * 1024 + 1) as u64)
                .read_to_end(&mut bytes)
                .expect("bounded descriptor read");
            assert!(
                bytes.len() <= 8 * 1024,
                "real serve descriptor exceeded the production 8 KiB bound"
            );
            if healthy(&address) {
                // Publication can become visible just before listener
                // readiness; treat an incomplete transient read as pending
                // rather than misclassifying it as a product failure.
                if let Ok(value) = serde_json::from_slice(&bytes) {
                    break (bytes, value);
                }
            }
        }
        assert!(
            Instant::now() < deadline,
            "real serve readiness exceeded 10 seconds"
        );
        thread::sleep(Duration::from_millis(20));
    };

    assert_eq!(
        descriptor["pid"].as_u64(),
        Some(u64::from(serve_pid)),
        "descriptor PID must identify the live test-owned serve child"
    );
    assert_eq!(
        descriptor["http_origin"].as_str(),
        Some(origin.as_str()),
        "both CLI clients must use the real server's published origin"
    );
    let bearer = descriptor["auth_token"]
        .as_str()
        .expect("real serve must publish its bearer")
        .to_owned();
    assert_eq!(bearer.len(), 64, "real daemon bearer has expected shape");
    assert!(
        bearer.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "real daemon bearer must have the expected hexadecimal encoding"
    );
    // Seed one uniquely named authenticated row through the real API before
    // either client starts. Each native client must render this persisted row
    // in its Sessions page; the row is not fabricated in the PTY transcript.
    let seeded = authenticated_json_request(
        &address,
        &bearer,
        "POST",
        "/api/sessions",
        Some(r#"{"title":"APP017 two-client persisted row"}"#),
    );
    assert!(seeded.starts_with("HTTP/1.1 201 "), "seed authenticated session: {seeded}");

    for client_number in 1..=2 {
        let (code, transcript, driver_stderr) = run_default_cli(&root, &address);
        assert_eq!(
            code, 0,
            "client {client_number} must exit cleanly: {transcript}"
        );
        assert!(
            transcript.contains("OpenCode RK"),
            "client {client_number} must reach real interactive startup: {transcript}"
        );
        // Native rendering does not expose the legacy line-chat attach or
        // empty-list labels. Causal proof is the real descriptor/origin/PID
        // identity above, the authenticated Sessions-page observation in the
        // bounded PTY driver, and the no-401 result below.
        assert!(
            transcript.contains("Sessions") && transcript.contains("APP017 two-client persisted row"),
            "client {client_number} must render the authenticated Sessions row: {transcript}"
        );
        assert!(
            !transcript.contains("session list failed: 401")
                && !transcript.contains("session list failed: 403"),
            "client {client_number} API access must not be rejected: {transcript}"
        );
        assert_no_token_output(&transcript);
        assert!(
            !transcript.contains(&bearer) && !driver_stderr.contains(&bearer),
            "real daemon bearer must not appear in client output or diagnostics"
        );
        assert!(
            serve
                .0
                .try_wait()
                .expect("poll owned serve after client")
                .is_none(),
            "client {client_number} must not stop the separately-owned daemon"
        );
        assert!(
            healthy(&address),
            "the same server origin must remain healthy after client {client_number} exits"
        );
        assert_eq!(
            fs::read(&descriptor_path).expect("read unchanged live descriptor"),
            descriptor_before,
            "client {client_number} must not replace the real owner's descriptor"
        );
    }

    // Explicit test ownership: terminate and reap only the Child we spawned.
    serve.stop();
    assert!(
        !healthy(&address),
        "the test-owned daemon listener must stop after explicit cleanup"
    );
}
