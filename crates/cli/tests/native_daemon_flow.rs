#![forbid(unsafe_code)]
//! Native daemon flow tests: fresh-launch daemon ownership (G1), implicit
//! attach/reuse, the redirected-stdio headless refusal, and bound live state.
//!
//! Contract status — repaired after independent review; see
//! `worklog/V2-NATIVE-FIXTURE-MAINTENANCE.md` and
//! `worklog/V2-NATIVE-CONTRACT-EVALUATION.md`. Higher authorities:
//!
//! - frozen local `crates/cli/src/app_start.rs` (`decide_launch_mode:116-124`,
//!   `enters_raw_mode:128-131`, `HEADLESS_EXIT_CODE=2:135`,
//!   `headless_message:140-146`; frozen tests `app001_t4:580-600`,
//!   `none_arm_headless_both_non_tty_carries_no_role_or_view:614-627`);
//! - pinned upstream `anomalyco/opencode@95daf906`
//!   `packages/opencode/src/cli/cmd/run.ts:319-320` (`--mini requires a TTY
//!   stdout`), `run.ts:416` / `cmd/tui.ts:60` (piped stdin is headless text).
//!
//! Which entrypoint owns which behavior (verified from source):
//! - **Implicit discovery / attach / daemon ownership** is the *default
//!   no-subcommand* launch: `main.rs:221-258` (TTY probe → `NativeTui`) →
//!   `chat::run` (`crates/cli/src/chat.rs:48-86`), which probes `/health`,
//!   attaches to a live daemon, else auto-spawns `serve` as an owned child and
//!   kills it on exit (`chat.rs:80-84`). `chat` binds the recent session and
//!   reads `/exit` (`chat.rs:383`).
//! - The `tui` **subcommand** has NO implicit discovery: without `--origin`,
//!   `resolve_origin_bearer(None)` returns `None` and `live` stays `None`
//!   (`tui_entry.rs:512,526-557`). Its bound path is `--origin`
//!   (`docs/USER_GUIDE.md:108`). Native quit is `:q` (`tui_entry.rs:408-409`).
//!
//! PTY transport: an embedded Python stdlib `pty.fork` driver launched as
//! `/usr/bin/python3 -c DRIVER RECORD -- EXE ARGS...`; the repository also has
//! `tests/e2e/native_interactive_pty.py`. No new crate dependency, unsafe, or
//! Cargo.lock change is used here. Every guarded child is spawned with
//! `CommandExt::process_group(0)` (safe std, no libc) so cleanup signals
//! exactly the process group this fixture owns.
//!
//! Resource discipline: `env_clear` plus disposable `HOME`/`XDG_*` (no secret
//! inheritance, no user home); bounded retained stdout/stderr (both drained);
//! deadline `try_wait`; RAII cleanup of the child and of any daemon identified
//! from the daemon's OWN published descriptor (validated origin+token+health
//! before any signal is sent). No forged descriptor. No fabricated model.

use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Mirrors `app_start::HEADLESS_EXIT_CODE` (observable process contract).
const HEADLESS_EXIT_CODE: i32 = 2;
/// Upper bound on bytes retained from any child pipe.
const MAX_RETAINED_BYTES: usize = 256 * 1024;
/// Bound on any child exit wait.
const EXIT_DEADLINE: Duration = Duration::from_secs(30);

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct TestHome(PathBuf);

impl TestHome {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("pp-{}-{id}", std::process::id()));
        let runtime = path.join("runtime");
        let socket = runtime.join("opencode-rk.sock");
        assert!(
            socket.as_os_str().len() <= 100,
            "fixture socket path is too long for sockaddr_un: {socket:?}"
        );
        for sub in ["", "home", "xdg-config", "xdg-data", "xdg-cache"] {
            fs::create_dir_all(path.join(sub)).unwrap();
        }
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
    fn home(&self) -> PathBuf {
        self.0.join("home")
    }
    fn descriptor(&self) -> PathBuf {
        self.0.join("runtime/backend.json")
    }
    fn pty_pid(&self) -> PathBuf {
        self.0.join("pty.pid")
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Bounded, continuously-draining pipe reader.
struct Drain {
    buffer: Arc<Mutex<String>>,
    truncated: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Drain {
    fn spawn(pipe: impl Read + Send + 'static) -> Self {
        let buffer = Arc::new(Mutex::new(String::new()));
        let truncated = Arc::new(AtomicBool::new(false));
        let b = Arc::clone(&buffer);
        let t = Arc::clone(&truncated);
        let handle = thread::spawn(move || {
            let mut pipe = pipe;
            let mut buf = [0_u8; 4096];
            loop {
                match pipe.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let mut guard = b.lock().unwrap();
                        let room = MAX_RETAINED_BYTES.saturating_sub(guard.len());
                        if room == 0 {
                            t.store(true, Ordering::Relaxed);
                            continue; // keep draining; retain nothing more
                        }
                        guard.push_str(&String::from_utf8_lossy(&buf[..n.min(room)]));
                        if n > room {
                            t.store(true, Ordering::Relaxed);
                        }
                    }
                }
            }
        });
        Self {
            buffer,
            truncated,
            handle: Some(handle),
        }
    }
    fn text(&self) -> String {
        self.buffer.lock().unwrap().clone()
    }
    fn is_truncated(&self) -> bool {
        self.truncated.load(Ordering::Relaxed)
    }
    fn join_bounded(&mut self, deadline: Duration) {
        if let Some(handle) = self.handle.take() {
            let end = Instant::now() + deadline;
            while !handle.is_finished() && Instant::now() < end {
                thread::sleep(Duration::from_millis(5));
            }
            assert!(
                handle.is_finished(),
                "drain thread did not terminate within {:?}; refusing an unbounded join",
                deadline
            );
            let _ = handle.join();
        }
    }
    fn wait_for(&self, needle: &str, timeout: Duration) -> String {
        let deadline = Instant::now() + timeout;
        loop {
            let text = self.text();
            if text.contains(needle) {
                return text;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {needle:?}; stdout so far:\n{}",
                self.text()
            );
            thread::sleep(Duration::from_millis(25));
        }
    }
}

impl Drop for Drain {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            if handle.is_finished() {
                let _ = handle.join();
            } else {
                panic!("drain dropped while reader is still blocked");
            }
        }
    }
}

/// Guarded child with bounded stdio and a deadline-bounded exit wait.
struct Proc {
    child: Child,
    stdout: Drain,
    stderr: Drain,
    /// For PTY roots: the driver's exclusive record of the CLI leader pid.
    pty_pid_file: Option<PathBuf>,
    /// Set once cleanup has signalled the owned group; a second `Drop` must
    /// never re-signal a recycled pid.
    cleaned: bool,
}

impl Proc {
    fn spawn(command: Command) -> Self {
        Self::spawn_inner(command, None)
    }
    fn spawn_pty(command: Command, pid_file: PathBuf) -> Self {
        Self::spawn_inner(command, Some(pid_file))
    }
    fn spawn_inner(mut command: Command, pty_pid_file: Option<PathBuf>) -> Self {
        // Safe std API (no unsafe/libc): every guarded child leads its own
        // process group so cleanup signals exactly what this fixture owns.
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command.spawn().expect("spawn binary");
        let stdout = match child.stdout.take() {
            Some(pipe) => Drain::spawn(pipe),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("piped stdout")
            }
        };
        let stderr = match child.stderr.take() {
            Some(pipe) => Drain::spawn(pipe),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("piped stderr")
            }
        };
        Self {
            child,
            stdout,
            stderr,
            pty_pid_file,
            cleaned: false,
        }
    }
    fn wait_deadline(&mut self, deadline: Duration) -> Option<ExitStatus> {
        let end = Instant::now() + deadline;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    // Reap any descendant that inherited a pipe before the
                    // reader joins; otherwise an exited wrapper can leave an
                    // unbounded EOF wait behind.
                    self.terminate_owned_descendants();
                    self.stdout.join_bounded(Duration::from_secs(2));
                    self.stderr.join_bounded(Duration::from_secs(2));
                    return Some(status);
                }
                Ok(None) if Instant::now() < end => thread::sleep(Duration::from_millis(25)),
                Ok(None) => {
                    self.terminate_owned_descendants();
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    self.stdout.join_bounded(Duration::from_secs(2));
                    self.stderr.join_bounded(Duration::from_secs(2));
                    return None;
                }
                Err(_) => {
                    self.terminate_owned_descendants();
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    self.stdout.join_bounded(Duration::from_secs(2));
                    self.stderr.join_bounded(Duration::from_secs(2));
                    return None;
                }
            }
        }
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        self.terminate_owned_descendants();
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.stdout.join_bounded(Duration::from_secs(2));
        self.stderr.join_bounded(Duration::from_secs(2));
    }
}

impl Proc {
    /// Signal and clean exactly the processes this fixture owns, at most once.
    ///
    /// For a PTY root the sequence is: TERM the Python driver so its `finally`
    /// can kill/reap the CLI session group; wait a bounded second; read the
    /// driver's exclusive PID record and kill that known CLI process group
    /// BEFORE force-killing a driver that ignored TERM. A repeated `Drop`
    /// never re-signals a pid this fixture already cleaned.
    fn terminate_owned_descendants(&mut self) {
        if self.cleaned {
            return;
        }
        self.cleaned = true;
        let driver_pid = self.child.id();
        let driver_alive = self.child.try_wait().ok().flatten().is_none();
        if let Some(path) = self.pty_pid_file.clone() {
            if driver_alive {
                // 1. Cooperative shutdown: the driver's `finally` kills and
                //    reaps the whole PTY child session group.
                signal_pid(driver_pid, "-TERM");
                let end = Instant::now() + Duration::from_secs(1);
                while self.child.try_wait().ok().flatten().is_none() && Instant::now() < end {
                    thread::sleep(Duration::from_millis(10));
                }
            }
            // 2. Safety net: kill the known CLI group from the driver's own
            //    exclusive record, so an exited-but-uncleaned or TERM-ignoring
            //    driver cannot orphan a session group.
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Ok(pid) = raw.trim().parse::<u32>() {
                    if pid != 0 && pid != driver_pid && pid_alive(pid) {
                        kill_process_group(pid);
                    }
                }
            }
            // 3. Only now force-kill a driver that outlived the TERM wait.
            if self.child.try_wait().ok().flatten().is_none() {
                signal_pid(driver_pid, "-KILL");
            }
            return;
        }
        if driver_alive {
            kill_process_group(self.child.id());
        }
    }
}

/// Send `signal` to one pid with no shell interpolation.
fn signal_pid(pid: u32, signal: &str) {
    let _ = Command::new("/bin/kill")
        .args([signal, "--", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn kill_process_group(pid: u32) {
    #[cfg(unix)]
    {
        let _ = Command::new("/bin/kill")
            .args(["-TERM", "--", &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        thread::sleep(Duration::from_millis(50));
        let _ = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

impl std::ops::Deref for Proc {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.child
    }
}
impl std::ops::DerefMut for Proc {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.child
    }
}

/// Daemon reaped on drop, but only after re-validating the CURRENT descriptor:
/// origin must match the tracked origin, token must be a 64-hex bearer, and
/// `/health` must answer 200. A stale/foreign pid is never signalled.
struct DaemonGuard {
    pid: u32,
    origin: String,
    token: String,
    descriptor: PathBuf,
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let ok = descriptor_matches(&self.descriptor, self.pid, &self.origin, &self.token)
            && health_ok(&self.origin)
            && pid_alive(self.pid);
        if ok {
            let _ = Command::new("kill")
                .arg(self.pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

fn free_loopback_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    format!("127.0.0.1:{}", listener.local_addr().unwrap().port())
}

fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn descriptor_matches(path: &Path, pid: u32, origin: &str, token: &str) -> bool {
    let Ok(raw) = fs::read(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&raw) else {
        return false;
    };
    value["pid"].as_u64() == Some(pid as u64)
        && value["http_origin"].as_str() == Some(origin)
        && value["auth_token"].as_str() == Some(token)
        && token.len() == 64
        && token.bytes().all(|b| b.is_ascii_hexdigit())
        && origin.starts_with("http://127.0.0.1:")
}

/// Public `/health` liveness probe (`daemon_auth.rs:3`: `/health` is public).
fn health_ok(origin: &str) -> bool {
    let addr = match origin.strip_prefix("http://") {
        Some(a) => a,
        None => return false,
    };
    let Ok(mut stream) = TcpStream::connect(addr) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let req = format!("GET /health HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut buf = Vec::new();
    let _ = stream.take(4096).read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).starts_with("HTTP/1.1 200")
}

/// Disposable-env command: no inherited secrets, disposable HOME + XDG dirs.
fn base_env(cmd: &mut Command, home: &TestHome) {
    let h = home.home();
    cmd.env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("HOME", &h)
        .env("XDG_CONFIG_HOME", home.path().join("xdg-config"))
        .env("XDG_DATA_HOME", home.path().join("xdg-data"))
        .env("XDG_CACHE_HOME", home.path().join("xdg-cache"))
        .env("XDG_RUNTIME_DIR", home.path().join("runtime"))
        .env("TMPDIR", home.path())
        .env("OPENCODE_RK_HOME", home.path());
}

/// Real-PTY launch through an owned portable Python driver. The child's stdio
/// is a tty, so `app_start` routes the native interactive path.
fn pty_command(bin: &str, args: &[&str], home: &TestHome, daemon_addr: &str) -> Command {
    const PTY_DRIVER: &str = r#"
import os, pty, select, signal, sys, time
if len(sys.argv) < 4 or sys.argv[2] != "--":
    sys.stderr.write("pty_driver: usage: -c DRIVER RECORD -- EXE [ARGS...]\n")
    sys.exit(125)
record_path = sys.argv[1]
argv = sys.argv[3:]
cancelled = False
def cancel(signum, frame):
    global cancelled
    cancelled = True
signal.signal(signal.SIGTERM, cancel)
def waitstatus_to_exitcode(status):
    if hasattr(os, "waitstatus_to_exitcode"):
        return os.waitstatus_to_exitcode(status)
    if os.WIFEXITED(status):
        return os.WEXITSTATUS(status)
    if os.WIFSIGNALED(status):
        return -os.WTERMSIG(status)
    return 1
def reap(leader, deadline_s):
    try:
        waited, st = os.waitpid(leader, os.WNOHANG)
    except ChildProcessError:
        return None
    if waited:
        return st
    try:
        os.killpg(leader, signal.SIGTERM)
    except (ProcessLookupError, PermissionError):
        pass
    end = time.monotonic() + deadline_s
    while time.monotonic() < end:
        try:
            waited, st = os.waitpid(leader, os.WNOHANG)
        except ChildProcessError:
            return None
        if waited:
            return st
        time.sleep(0.01)
    try:
        os.killpg(leader, signal.SIGKILL)
    except (ProcessLookupError, PermissionError):
        pass
    try:
        _, st = os.waitpid(leader, 0)
    except ChildProcessError:
        return None
    return st
pid, master = pty.fork()
if pid == 0:
    os.execvp(argv[0], argv)
    os._exit(127)
with open(record_path, "x") as record:
    record.write(str(pid))
status = None
try:
    while not cancelled:
        ready, _, _ = select.select([master, 0], [], [], 0.1)
        if master in ready:
            try: data = os.read(master, 65536)
            except OSError: break
            if not data: break
            os.write(1, data)
        if 0 in ready:
            data = os.read(0, 65536)
            if not data: break
            os.write(master, data)
finally:
    status = reap(pid, 0.5)
    try:
        os.killpg(pid, signal.SIGKILL)
    except (ProcessLookupError, PermissionError):
        pass
if status is None:
    sys.exit(1)
code = waitstatus_to_exitcode(status)
sys.exit(code if code >= 0 else 128 - code)
"#;
    let mut cmd = Command::new("/usr/bin/python3");
    cmd.args([
        "-c",
        PTY_DRIVER,
        home.pty_pid().to_str().unwrap(),
        "--",
        bin,
    ])
    .args(args);
    base_env(&mut cmd, home);
    cmd.current_dir(home.path())
        .env("TERM", "xterm-256color")
        .env("OPENCODE_RK_DAEMON_ADDR", daemon_addr)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

fn plain_command(home: &TestHome, daemon_addr: &str) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    base_env(&mut cmd, home);
    cmd.env("OPENCODE_RK_DAEMON_ADDR", daemon_addr);
    cmd
}

fn send_line(child: &mut Child, line: &str) {
    let stdin = child.stdin.as_mut().expect("piped stdin");
    writeln!(stdin, "{line}").expect("write line");
    stdin.flush().expect("flush line");
}

fn wait_for_descriptor(home: &TestHome, timeout: Duration) {
    let descriptor = home.descriptor();
    let deadline = Instant::now() + timeout;
    while !descriptor.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        descriptor.exists(),
        "daemon did not publish {descriptor:?} within {timeout:?}"
    );
}

/// Read and validate the daemon's own descriptor: 64-hex bearer, loopback
/// origin. Returns (pid, origin, token). Never forges a descriptor.
fn read_real_descriptor(home: &TestHome) -> (u32, String, String) {
    let raw = fs::read(home.descriptor()).expect("read published descriptor");
    let value: serde_json::Value = serde_json::from_slice(&raw).expect("descriptor is JSON");
    let pid = value["pid"].as_u64().expect("descriptor pid") as u32;
    let origin = value["http_origin"]
        .as_str()
        .expect("descriptor http_origin")
        .to_owned();
    let token = value["auth_token"]
        .as_str()
        .expect("descriptor auth_token")
        .to_owned();
    assert_eq!(token.len(), 64, "real daemon bearer must be 64 hex chars");
    assert!(
        token.bytes().all(|b| b.is_ascii_hexdigit()),
        "real daemon bearer must be hex"
    );
    assert!(
        origin.starts_with("http://127.0.0.1:"),
        "descriptor origin must be loopback, got {origin}"
    );
    (pid, origin, token)
}

/// Spawn `serve` under a disposable home and wait for its real descriptor.
fn spawn_serve(home: &TestHome, daemon_addr: &str) -> (Proc, u32, String, String) {
    let mut cmd = plain_command(home, daemon_addr);
    cmd.args(["serve", "--listen", daemon_addr])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let proc = Proc::spawn(cmd);
    wait_for_descriptor(home, Duration::from_secs(20));
    let (pid, origin, token) = read_real_descriptor(home);
    (proc, pid, origin, token)
}

/// Create a session through the daemon's authenticated `POST /api/sessions`.
fn create_session(origin: &str, token: &str, title: &str) {
    let addr = origin
        .strip_prefix("http://")
        .unwrap_or_else(|| panic!("only http origins supported, got {origin:?}"));
    let body = serde_json::json!({ "title": title }).to_string();
    let request = format!(
        "POST /api/sessions HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let mut stream = TcpStream::connect_timeout(
        &addr.parse().expect("daemon address"),
        Duration::from_secs(5),
    )
    .expect("connect daemon api");
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
    stream.write_all(request.as_bytes()).expect("write create");
    let mut response = Vec::new();
    let _ = stream.take(64 * 1024).read_to_end(&mut response);
    let text = String::from_utf8_lossy(&response);
    assert!(
        text.starts_with("HTTP/1.1 201") || text.starts_with("HTTP/1.1 200"),
        "session create must return 2xx; got:\n{text}"
    );
}

fn bounded_output(cmd: Command) -> (ExitStatus, String, String) {
    let mut proc = Proc::spawn(cmd);
    let status = proc
        .wait_deadline(EXIT_DEADLINE)
        .expect("bounded command exit");
    (status, proc.stdout.text(), proc.stderr.text())
}

// ---------------------------------------------------------------------------
// T01: fresh default launch owns + publishes exactly one authenticated daemon.
// ---------------------------------------------------------------------------
#[test]
fn native_daemon_spawns_when_none_running() {
    let home = TestHome::new();
    let daemon_addr = free_loopback_addr();

    // Bare (no subcommand) launch under a real PTY: app_start routes to the
    // interactive owner and `chat::run` auto-spawns the daemon (chat.rs:48-86).
    let cmd = pty_command(env!("CARGO_BIN_EXE_opencode-rk"), &[], &home, &daemon_addr);
    let mut proc = Proc::spawn_pty(cmd, home.pty_pid());

    proc.stdout.wait_for("OpenCode RK", Duration::from_secs(30));
    wait_for_descriptor(&home, Duration::from_secs(20));

    let (pid, origin, token) = read_real_descriptor(&home);
    let _guard = DaemonGuard {
        pid,
        origin: origin.clone(),
        token: token.clone(),
        descriptor: home.descriptor(),
    };

    assert_ne!(
        pid,
        std::process::id(),
        "daemon pid must not be the test process pid"
    );
    assert!(pid_alive(pid), "published daemon pid {pid} must be alive");
    assert!(health_ok(&origin), "/health must answer on {origin}");
    create_session(&origin, &token, "NativeAuthenticatedProbe");

    send_line(&mut *proc, "/exit");
    let status = proc
        .wait_deadline(EXIT_DEADLINE)
        .expect("fresh launch exits within deadline");
    assert!(
        status.success(),
        "fresh launch must exit 0 after /exit; stdout:\n{}",
        proc.stdout.text()
    );

    // Owned lifecycle: `chat::run` kills its spawned daemon on exit
    // (chat.rs:80-84). Allow a short reap window.
    let reap_deadline = Instant::now() + Duration::from_secs(10);
    while pid_alive(pid) && Instant::now() < reap_deadline {
        thread::sleep(Duration::from_millis(100));
    }
    assert!(
        !pid_alive(pid),
        "owned daemon pid {pid} must be reaped after /exit"
    );
}

// ---------------------------------------------------------------------------
// T02: redirected no-subcommand launch is refused headlessly (security).
// ---------------------------------------------------------------------------
#[test]
fn native_no_tty_entry_routes_headless_without_raw_mode_or_daemon() {
    let home = TestHome::new();
    let daemon_addr = free_loopback_addr();

    // Corrected contract (was the inverse): a redirected launch must NOT enter
    // raw mode and must NOT start a daemon. Frozen `app_start` + upstream.
    let mut cmd = plain_command(&home, &daemon_addr);
    cmd.arg("--native")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let (status, stdout, stderr) = bounded_output(cmd);

    assert_eq!(
        status.code(),
        Some(HEADLESS_EXIT_CODE),
        "redirected launch must exit {HEADLESS_EXIT_CODE}; stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(
        stderr.contains("raw mode is refused"),
        "headless message must name the raw-mode refusal; stderr:\n{stderr}"
    );
    assert!(
        !home.descriptor().exists(),
        "a refused redirected launch must not start a daemon or publish {:?}",
        home.descriptor()
    );
}

// ---------------------------------------------------------------------------
// T03: `tui` attaches to a running serve daemon WITHOUT --origin.
// This is intentionally a genuine product RED until the subcommand performs
// the same descriptor discovery as the default native entrypoint.
// ---------------------------------------------------------------------------
#[test]
fn tui_attaches_to_running_serve_daemon_without_origin() {
    let home = TestHome::new();
    let daemon_addr = free_loopback_addr();
    let (_serve, pid, origin, token) = spawn_serve(&home, &daemon_addr);
    let _guard = DaemonGuard {
        pid,
        origin: origin.clone(),
        token: token.clone(),
        descriptor: home.descriptor(),
    };

    create_session(&origin, &token, "NativeTuiLiveProbe");

    // The explicit `tui` entrypoint must discover the real descriptor without
    // an origin argument.  Use a PTY because this is an interactive contract;
    // `:q` is the tui quit token (not chat's `/exit`).
    let cmd = pty_command(
        env!("CARGO_BIN_EXE_opencode-rk"),
        &["tui"],
        &home,
        &daemon_addr,
    );
    let mut proc = Proc::spawn_pty(cmd, home.pty_pid());

    let transcript = proc.stdout.wait_for("OpenCode RK", Duration::from_secs(30));
    assert!(
        !transcript.contains("offline"),
        "tui must attach, not degrade offline; transcript:\n{transcript}"
    );
    assert!(
        transcript.contains("NativeTuiLiveProbe") && transcript.contains("(live)"),
        "tui must render the live bound session state; transcript:\n{transcript}"
    );

    // Real reuse evidence: the SAME daemon descriptor is unchanged and healthy.
    let (pid2, origin2, token2) = read_real_descriptor(&home);
    assert_eq!(pid2, pid, "attach must reuse the existing daemon pid");
    assert_eq!(origin2, origin, "attach must reuse the same origin");
    assert!(
        descriptor_matches(&home.descriptor(), pid, &origin, &token),
        "the same real descriptor bearer must remain published after attach"
    );
    drop(token2); // bearer is compared without ever formatting it in a failure
    assert!(health_ok(&origin), "reused daemon /health must stay 200");

    send_line(&mut *proc, ":q");
    let status = proc
        .wait_deadline(EXIT_DEADLINE)
        .expect("attached launch exits within deadline");
    assert!(
        status.success(),
        "attached launch must exit 0 after /exit; stdout:\n{}",
        proc.stdout.text()
    );
    // The pre-existing daemon is NOT owned, so it stays alive (chat.rs:80-84).
    assert!(pid_alive(pid), "reused daemon must survive the attacher");
}

// ---------------------------------------------------------------------------
// T04: a bound --once frame carries the real daemon's live session state.
// ---------------------------------------------------------------------------
#[test]
fn status_frame_carries_live_daemon_values() {
    let home = TestHome::new();
    let daemon_addr = free_loopback_addr();
    let (_serve, pid, origin, token) = spawn_serve(&home, &daemon_addr);
    let _guard = DaemonGuard {
        pid,
        origin: origin.clone(),
        token: token.clone(),
        descriptor: home.descriptor(),
    };

    let session_title = "NativeLiveProbe";
    create_session(&origin, &token, session_title);

    // Scriptable bound frame (docs/USER_GUIDE.md:108): real descriptor reuse
    // via `--origin` (tui_entry.rs:564-583). No forged descriptor.
    let mut cmd = plain_command(&home, &daemon_addr);
    cmd.args(["tui", "--origin", origin.as_str(), "--once"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let (status, stdout, stderr) = bounded_output(cmd);

    assert!(
        status.success(),
        "bound --once must exit 0; stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains(session_title) && stdout.contains("(live)"),
        "bound frame must carry live session state (render_live, \
         tui_entry.rs:300-311); stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("[context:") && stdout.contains("tokens"),
        "status bar must carry the real context/token line; stdout:\n{stdout}"
    );
    // NOTE (BLOCKED product gap, not a weakened assertion): the status bar's
    // model field is hardcoded `"unset"` at tui_entry.rs:530/545 and there is
    // no product entrypoint that selects a daemon model, so a live-model-value
    // assertion has no configured state to reflect. Recorded as BLOCKED in the
    // worklog rather than asserted against a fabricated value.
}
