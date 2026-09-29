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
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        serve_one(&mut stream, &worker_requests, health_status)
                    }
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

    fn health_still_responds(&self) -> bool {
        let Ok(mut stream) = TcpStream::connect_timeout(
            &self.address.parse().expect("fixture socket address"),
            Duration::from_millis(300),
        ) else {
            return false;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
        if write!(stream, "GET /health HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", self.address).is_err() {
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

fn serve_one(stream: &mut TcpStream, log: &Mutex<Vec<ObservedRequest>>, health_status: u16) {
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
    let _method = fields.next().unwrap_or_default();
    let path = fields.next().unwrap_or_default().to_owned();
    let authorization = lines.find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("authorization")
            .then(|| value.trim().to_owned())
    });
    log.lock()
        .expect("request log lock")
        .push(ObservedRequest {
            path: path.clone(),
            authorization: authorization.clone(),
        });

    let (status, body) = if path == "/health" && health_status == 200 {
        ("200 OK", r#"{"status":"ok"}"#)
    } else if path == "/health" {
        ("503 Service Unavailable", r#"{"status":"unavailable"}"#)
    } else if path.starts_with("/api/") && authorization.as_deref() == Some("Bearer ".to_owned() + &TEST_TOKEN.repeat(32)).as_deref() {
        ("200 OK", r#"{"sessions":[]}"#)
    } else {
        ("401 Unauthorized", r#"{"message":"unauthorized"}"#)
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// Runs the actual no-subcommand binary in a PTY. Python is only the stdlib
/// PTY adapter; all launch, descriptor reads, health probes, requests and
/// lifecycle ownership under test execute in the compiled Rust CLI binary.
/// The child environment is reconstructed from a tiny explicit allowlist and
/// the PTY transcript is byte-capped. A 25-second Python alarm bounds even a
/// stuck CLI and its `finally` block kills/reaps the PTY session process group.
fn run_default_cli(root: &FixtureRoot, address: &str) -> (i32, String, String) {
    const DRIVER: &str = r#"
import errno, os, pty, select, signal, sys, time

binary, data_dir, home, address = sys.argv[1:]
pid = None
pgid = None
master = None
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
        pid, master = pty.fork()
        pgid = pid
    except BaseException as error:
        infra("pty.fork failed: " + repr(error))
    if pid == 0:
        # The driver's hard alarm belongs to the parent supervisor, not the
        # exec'd CLI (which has its own independently asserted deadlines).
        signal.alarm(0)
        env = {
            "HOME": home,
            "OPENCODE_RK_HOME": data_dir,
            "OPENCODE_RK_DAEMON_ADDR": address,
            "PATH": "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin",
            "TMPDIR": home,
        }
        try:
            os.execve(binary, [binary, "--data-dir", data_dir], env)
        except BaseException as error:
            os.write(2, ("PTY_EXEC_FAILED: " + repr(error) + "\n").encode())
            os._exit(90)

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
        waited, child_status = os.waitpid(pid, os.WNOHANG)
        if waited == pid:
            pid = None
            infra("CLI exited before startup banner; status=" + repr(child_status))
    if ready_marker not in captured:
        infra("PTY_NOT_READY: CLI startup banner absent within eleven seconds")

    # Canonical TTY input remains queued until the real chat loop reads it.
    os.write(master, b"/sessions\n/exit\n")
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
        waited, child_status = os.waitpid(pid, os.WNOHANG)
        if waited == pid:
            status = child_status
            pid = None
            break
    if status is None:
        infra("CLI did not exit after /exit within 10.5 seconds")

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
    code = os.waitstatus_to_exitcode(status)
    sys.stdout.buffer.write((str(code) + "\n").encode() + captured)
    sys.stdout.buffer.flush()
finally:
    signal.alarm(0)
    if pgid is not None:
        try:
            os.killpg(pgid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    if pid is not None:
        try:
            os.waitpid(pid, 0)
        except ChildProcessError:
            pass
    if master is not None:
        try:
            os.close(master)
        except OSError:
            pass
"#;

    let binary = env!("CARGO_BIN_EXE_opencode-rk");
    let mut driver = Command::new("python3");
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
            let _ = child.kill();
            let _ = child.wait();
            panic!("PTY driver exceeded 30-second outer bound");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let output = child.wait_with_output().expect("collect bounded PTY driver output");
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
    assert!(output.stdout.len() <= MAX_CAPTURE + 16, "outer PTY output bound");
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

    assert_eq!(code, 0, "real no-subcommand CLI should exit cleanly: {transcript}");
    assert!(transcript.contains(&format!("daemon: {origin}")), "real chat attach banner: {transcript}");
    assert!(!fixture.health_requests().is_empty(), "fixture must observe public readiness probe");
    assert_no_token_output(&transcript);
    assert_no_token_output(&driver_stderr);
    let api = fixture.api_requests();
    assert!(!api.is_empty(), "the actual CLI must reach the fixture API after attach");
    assert!(
        api.iter().all(|request| request.authorization.as_deref()
            == Some(format!("Bearer {}", TEST_TOKEN.repeat(32)).as_str())),
        "every API request must carry only the descriptor bearer: {api:?}"
    );
}

#[test]
fn app017_t02_healthy_foreign_listener_without_descriptor_is_not_attached_or_killed() {
    let root = FixtureRoot::new("foreign");
    let fixture = HttpFixture::start();

    let (code, transcript, driver_stderr) = run_default_cli(&root, &fixture.address);

    assert_eq!(code, 0, "foreign listener is a refusal, not a CLI crash: {transcript}");
    assert!(
        !transcript.contains(&format!("daemon: {}", fixture.origin())),
        "public liveness alone must not yield an attached lease: {transcript}"
    );
    assert!(!fixture.health_requests().is_empty(), "CLI must make a public health probe");
    assert!(fixture.api_requests().is_empty(), "no descriptor means no API activity");
    assert!(fixture.health_still_responds(), "refusal must leave the foreign listener alive");
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
            "schema-mismatch" => root.publish(&configured_origin, std::process::id(), &TEST_TOKEN.repeat(32), 77),
            "origin-mismatch" => root.publish(&other.origin(), std::process::id(), &TEST_TOKEN.repeat(32), 1),
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
        assert_eq!(code, 0, "invalid descriptor {case} must fail closed: {transcript}");
        assert!(
            !transcript.contains(&format!("daemon: {configured_origin}")),
            "invalid descriptor {case} must not be presented as an attached daemon: {transcript}"
        );
        assert!(!configured.health_requests().is_empty(), "CLI must probe configured health origin");
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
        assert!(fixture.health_still_responds(), "unsupported fixture leaves listener intact");
        assert!(fixture.api_requests().is_empty(), "unsupported fixture sends no API requests");
        return;
    }

    let (code, transcript, driver_stderr) = run_default_cli(&root, &fixture.address);
    assert_eq!(code, 0, "wrong-owner descriptor must fail closed: {transcript}");
    assert!(
        !transcript.contains(&format!("daemon: {}", fixture.origin())),
        "wrong-owner descriptor must not establish an attached lease"
    );
    assert!(!fixture.health_requests().is_empty(), "CLI must probe wrong-owner listener");
    assert!(fixture.api_requests().is_empty(), "wrong-owner token must never authorize API traffic");
    assert!(fixture.health_still_responds(), "wrong-owner refusal must not kill its listener");
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

    assert_eq!(code, 0, "failed health must be handled as startup failure: {transcript}");
    assert!(
        !transcript.contains(&format!("daemon: {}", fixture.origin())),
        "non-200 readiness must not attach an authenticated lease"
    );
    assert!(!fixture.health_requests().is_empty(), "CLI must receive the non-200 health response");
    assert!(fixture.api_requests().is_empty(), "health failure must precede any API request");
    assert!(fixture.health_still_responds(), "the pre-existing 503 listener must remain untouched");
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
    assert!(transcript.contains("OpenCode RK"), "real chat startup banner observed: {transcript}");
    assert!(root.descriptor_path().is_file(), "owned daemon published its descriptor");
    let descriptor: serde_json::Value = serde_json::from_slice(
        &fs::read(root.descriptor_path()).expect("read published descriptor"),
    )
    .expect("valid published descriptor");
    let pid = descriptor["pid"].as_u64().expect("published child PID") as u32;
    assert_ne!(pid, 0, "published daemon PID is positive");
    let actual_bearer = descriptor["auth_token"].as_str().expect("serve-minted bearer");
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
        assert!(Instant::now() < deadline, "CLI exit must stop its owned daemon listener");
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

    assert_eq!(code, 0, "existing healthy daemon must be reusable: {transcript}");
    assert!(transcript.contains(&format!("daemon: {origin}")), "existing service attach observed: {transcript}");
    assert_eq!(
        fs::read(root.descriptor_path()).expect("descriptor after CLI exit"),
        before,
        "attach must not republish/replace the existing daemon descriptor"
    );
    assert!(fixture.health_still_responds(), "attaching client must not stop a pre-existing service");
    assert!(!fixture.health_requests().is_empty(), "fixture observed existing-service health probe");
    assert!(!fixture.api_requests().is_empty(), "existing valid service saw real API requests");
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
            serve.0.try_wait().expect("poll owned serve process").is_none(),
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
        assert!(Instant::now() < deadline, "real serve readiness exceeded 10 seconds");
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
        assert!(
            transcript.contains(&format!("daemon: {origin}")),
            "client {client_number} must attach to the same live origin: {transcript}"
        );
        assert!(
            transcript.contains("no sessions yet; type /new to start one"),
            "client {client_number} must receive the authenticated empty-session result: {transcript}"
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
            serve.0.try_wait().expect("poll owned serve after client").is_none(),
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
