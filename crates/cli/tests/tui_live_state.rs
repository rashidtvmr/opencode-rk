#![forbid(unsafe_code)]
//! TUI live-state lane: composer/status frame bound to real daemon session
//! state over the singleton HTTP API. Binary-driven; spawns a real daemon on
//! a free loopback port with a disposable home.
//!
//! Both real routes are covered:
//!
//! - `--once` is the scriptable snapshot (`docs/USER_GUIDE.md`), which renders
//!   the bounded legacy frame through the native renderer;
//! - the interactive route requires BOTH stdio TTYs (redirected stdio is
//!   refused by the frozen launch contract `crates/cli/src/app_start.rs`), so
//!   it is driven through a real PTY here. The line-mode stdin loop is gone.
//!
//! `/api/*` is authenticated: every request below carries the daemon's OWN
//! published descriptor bearer, read from `<home>/runtime/backend.json`. No
//! token is invented and `/health` stays public.

use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct TestHome(PathBuf);

impl TestHome {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "opencode-rk-tui-live-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn descriptor(&self) -> PathBuf {
        self.0.join("runtime/backend.json")
    }

    /// models.dev-shaped catalogue so the interactive route has a selectable
    /// model; the daemon's catalogue is empty without this file.
    fn models_file(&self) -> PathBuf {
        self.0.join("models.dev.api.json")
    }

    fn pty_record(&self) -> PathBuf {
        self.0.join("pty.pid")
    }

    fn pty_steps(&self) -> PathBuf {
        self.0.join("pty-steps.json")
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_loopback_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// A loopback port that is reserved then released: connecting to it fails
/// immediately, which a provider request treats as an unreachable provider.
fn closed_loopback_origin() -> String {
    let port = free_loopback_port();
    format!("http://127.0.0.1:{port}")
}

fn write_models_fixture(home: &TestHome) {
    let body = r#"{"openai":{"id":"openai","name":"OpenAI","env":["OPENAI_API_KEY"],
        "models":{"gpt-5.6":{"id":"gpt-5.6","name":"GPT-5.6","tool_call":true,"attachment":false}}}}"#;
    fs::write(home.models_file(), body).expect("write models fixture");
}

/// Spawn the singleton daemon on a free port with a disposable home.
fn spawn_daemon(home: &TestHome, port: u16, extra_env: &[(&str, &str)]) -> (ChildGuard, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    cmd.env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("HOME", home.path())
        .env("OPENCODE_RK_HOME", home.path())
        .args([
            "web",
            "--no-open",
            "--listen",
            &format!("127.0.0.1:{port}"),
            "--models-file",
        ])
        .arg(home.models_file())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (key, value) in extra_env {
        cmd.env(key, value);
    }
    let guard = ChildGuard(cmd.spawn().expect("spawn daemon"));
    let origin = format!("http://127.0.0.1:{port}");
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if http(&origin, "GET", "/health", None, None).is_some() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "daemon did not start at {origin}"
        );
        thread::sleep(Duration::from_millis(50));
    }
    (guard, origin)
}

/// Read the daemon's published 64-hex bearer from its OWN descriptor.
fn read_bearer(home: &TestHome) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(raw) = fs::read(home.descriptor()) {
            let value: serde_json::Value = serde_json::from_slice(&raw).expect("descriptor JSON");
            let token = value["auth_token"]
                .as_str()
                .expect("descriptor auth_token")
                .to_owned();
            assert_eq!(token.len(), 64, "descriptor bearer must be 64 hex chars");
            assert!(
                token.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "descriptor bearer must be hex"
            );
            return token;
        }
        assert!(
            Instant::now() < deadline,
            "daemon never published {:?}",
            home.descriptor()
        );
        thread::sleep(Duration::from_millis(50));
    }
}

/// Bounded HTTP/1.1 client. Returns `(status, body)` for any complete
/// response; `None` on transport failure. `/api/*` carries the descriptor
/// bearer.
fn http(
    origin: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
    bearer: Option<&str>,
) -> Option<(u16, String)> {
    let addr: SocketAddr = origin.trim_start_matches("http://").parse().ok()?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(3)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    stream.set_write_timeout(Some(Duration::from_secs(5))).ok()?;
    let payload = body.unwrap_or("");
    let auth = bearer
        .map(|token| format!("Authorization: Bearer {token}\r\n"))
        .unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {origin}\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(request.as_bytes()).ok()?;
    let mut response = Vec::new();
    stream.take(1024 * 1024).read_to_end(&mut response).ok()?;
    let text = String::from_utf8_lossy(&response).into_owned();
    let status = text.split_whitespace().nth(1)?.parse().ok()?;
    let body_start = text.find("\r\n\r\n")? + 4;
    Some((status, text[body_start..].to_owned()))
}

fn create_session(origin: &str, bearer: &str, title: &str) -> String {
    let body = serde_json::json!({ "title": title }).to_string();
    let (status, response) =
        http(origin, "POST", "/api/sessions", Some(&body), Some(bearer)).expect("create session");
    assert!(
        (200..300).contains(&status),
        "session create must return 2xx, got {status}: {response}"
    );
    let value: serde_json::Value = serde_json::from_str(&response).expect("create JSON");
    value["session"]["id"]
        .as_str()
        .expect("session id")
        .to_owned()
}

/// Read the durable user/assistant texts of a session through the same
/// authenticated route the client itself uses.
fn session_texts(origin: &str, bearer: &str, session: &str) -> Vec<String> {
    let (status, body) = http(
        origin,
        "GET",
        &format!("/api/sessions/{session}/history?limit=100"),
        None,
        Some(bearer),
    )
    .expect("read history");
    assert_eq!(status, 200, "history must return 200: {body}");
    let value: serde_json::Value = serde_json::from_str(&body).expect("history JSON");
    value["messages"]
        .as_array()
        .expect("messages array")
        .iter()
        .filter_map(|message| {
            message["body"]["text"]
                .as_str()
                .or_else(|| message["body"]["Inline"]["text"].as_str())
                .map(str::to_owned)
        })
        .collect()
}

/// Wait (bounded) until a durable message containing `needle` exists.
fn wait_for_message(origin: &str, bearer: &str, session: &str, needle: &str) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let texts = session_texts(origin, bearer, session);
        if texts.iter().any(|text| text.contains(needle)) {
            return texts;
        }
        assert!(
            Instant::now() < deadline,
            "prompt {needle:?} never reached the daemon; durable texts: {texts:?}"
        );
        thread::sleep(Duration::from_millis(100));
    }
}

/// Real-PTY driver: a stdlib-only expect/send script so interactive contracts
/// are exercised without inventing a TTY. Steps are JSON:
/// `[{"expect": "text", "timeout": 30}, {"send": "text"}, {"sleep": 1.5}, ...]`.
const PTY_DRIVER: &str = r#"
import fcntl, json, os, pty, select, signal, struct, sys, termios, time

record_path, steps_path = sys.argv[1], sys.argv[2]
if sys.argv[3] != "--" or len(sys.argv) < 5:
    sys.stderr.write("pty_driver: usage: -c DRIVER RECORD STEPS -- EXE [ARGS...]\n")
    sys.exit(125)
argv = sys.argv[4:]
with open(steps_path) as handle:
    steps = json.load(handle)

pid, master = pty.fork()
if pid == 0:
    os.execvp(argv[0], argv)
    os._exit(127)
# An inherited 0x0 winsize (the parent test runner has no terminal) makes the
# native renderer paint a degenerate frame; pin the journey size explicitly.
try:
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 160, 0, 0))
except OSError:
    pass
with open(record_path, "x") as record:
    record.write(str(pid))

buf = b""

def pump(deadline):
    global buf
    while time.monotonic() < deadline:
        ready, _, _ = select.select([master], [], [], 0.05)
        if master not in ready:
            continue
        try:
            data = os.read(master, 65536)
        except OSError:
            return False
        if not data:
            return False
        os.write(1, data)
        buf += data
    return True

failure = None
for step in steps:
    if "expect" in step:
        needle = step["expect"].encode()
        end = time.monotonic() + float(step.get("timeout", 30))
        while needle not in buf and time.monotonic() < end:
            if not pump(time.monotonic() + 0.05):
                break
        if needle not in buf:
            failure = "timed out waiting for %r" % step["expect"]
            break
    elif "send" in step:
        os.write(master, step["send"].encode())
    elif "sleep" in step:
        pump(time.monotonic() + float(step["sleep"]))

status = None
if failure is None:
    end = time.monotonic() + 30
    while time.monotonic() < end:
        try:
            waited, st = os.waitpid(pid, os.WNOHANG)
        except ChildProcessError:
            status = 0
            break
        if waited:
            status = st
            break
        pump(time.monotonic() + 0.05)
    if status is not None:
        pump(time.monotonic() + 0.2)
if status is None:
    try:
        os.killpg(pid, signal.SIGKILL)
    except (ProcessLookupError, PermissionError):
        pass
    try:
        _, status = os.waitpid(pid, 0)
    except ChildProcessError:
        status = 0
if failure is not None:
    sys.stderr.write("pty_driver: %s\n" % failure)
    sys.exit(1)

if hasattr(os, "waitstatus_to_exitcode"):
    code = os.waitstatus_to_exitcode(status)
else:
    code = os.WEXITSTATUS(status) if os.WIFEXITED(status) else 1
sys.exit(code if code >= 0 else 128 - code)
"#;

fn run_pty(home: &TestHome, args: &[&str], steps: &serde_json::Value) -> std::process::Output {
    fs::write(home.pty_steps(), steps.to_string()).expect("write pty steps");
    let _ = fs::remove_file(home.pty_record());
    let mut cmd = Command::new("/usr/bin/python3");
    cmd.arg("-c")
        .arg(PTY_DRIVER)
        .arg(home.pty_record())
        .arg(home.pty_steps())
        .arg("--")
        .arg(env!("CARGO_BIN_EXE_opencode-rk"))
        .args(args);
    cmd.env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("HOME", home.path())
        .env("OPENCODE_RK_HOME", home.path())
        .env("TERM", "xterm-256color")
        .env("NO_COLOR", "1")
        .current_dir(home.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.output().expect("run pty driver")
}

/// Drop terminal control sequences (CSI/OSC/DCS) from a raw PTY capture. The
/// capture is a repaint stream, so cursor moves sit between words.
fn strip_terminal_codes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('[') => {
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if ('\u{40}'..='\u{7e}').contains(&next) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(next) = chars.next() {
                    if next == '\u{7}' {
                        break;
                    }
                    if next == '\u{1b}' {
                        chars.next();
                        break;
                    }
                }
            }
            Some('P') | Some('_') | Some('^') => {
                while let Some(next) = chars.next() {
                    if next == '\u{1b}' {
                        chars.next();
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Collapse runs of whitespace so assertions do not depend on the native
/// renderer's cell painting (it pads words with the remaining row width).
fn squash(text: &str) -> String {
    strip_terminal_codes(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn tui_once(home: &TestHome, origin: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_opencode-rk"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("HOME", home.path())
        .env("OPENCODE_RK_HOME", home.path())
        .env("NO_COLOR", "1")
        .args(["tui", "--once", "--origin", origin])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run tui --once")
}

// Live snapshot: `--once --origin` renders the real session title and message
// count from the daemon instead of local placeholders.
#[test]
fn l01_once_with_origin_renders_live_session() {
    let home = TestHome::new();
    let port = free_loopback_port();
    write_models_fixture(&home);
    let (_guard, origin) = spawn_daemon(&home, port, &[]);
    let bearer = read_bearer(&home);

    let id = create_session(&origin, &bearer, "live probe");
    let seeded = serde_json::json!({ "text": "seed message" }).to_string();
    let (status, body) = http(
        &origin,
        "POST",
        &format!("/api/sessions/{id}/messages"),
        Some(&seeded),
        Some(&bearer),
    )
    .expect("seed message");
    assert_eq!(status, 201, "seed append must return 201: {body}");

    let out = tui_once(&home, &origin);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        stdout.contains("live probe"),
        "live title in frame: {stdout}"
    );
    assert!(
        stdout.contains("(live)"),
        "live data must be tagged: {stdout}"
    );
    assert!(stdout.contains("1 message"), "message count: {stdout}");
}

// Interactive submit posts to the daemon; the prompt is durably persisted and
// visible through the authenticated history route afterwards. The interactive
// route needs a real PTY (redirected stdio is refused before raw mode).
#[test]
fn l02_interactive_submit_persists_via_daemon() {
    let home = TestHome::new();
    let port = free_loopback_port();
    write_models_fixture(&home);
    // A credential must resolve for the daemon to admit the turn; the base URL
    // points at a closed loopback port so the provider call fails fast AFTER
    // the durable user-message append.
    let closed = closed_loopback_origin();
    let (_guard, origin) = spawn_daemon(
        &home,
        port,
        &[
            ("OPENAI_API_KEY", "synthetic-fixture-key"),
            ("OPENAI_BASE_URL", &closed),
        ],
    );
    let bearer = read_bearer(&home);
    let id = create_session(&origin, &bearer, "interactive probe");

    let steps = serde_json::json!([
        { "expect": "interactive probe", "timeout": 45 },
        { "send": "hello daemon\r" },
        { "sleep": 4.0 },
        // A rejected turn preserves the draft (never silently dropped); the
        // composer must be cleared before the quit token can be submitted.
        { "send": "\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}" },
        { "send": ":q\r" },
    ]);
    let out = run_pty(&home, &["tui", "--origin", &origin, "--session", &id], &steps);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        out.status.success(),
        "interactive launch must exit 0 after :q; stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("hello daemon"),
        "the typed prompt must be echoed in the native frame: {stdout}"
    );

    // Durable: the prompt reached the daemon, not just the local composer.
    let texts = wait_for_message(&origin, &bearer, &id, "hello daemon");
    assert!(
        texts.iter().any(|text| text.contains("hello daemon")),
        "submit must persist through the daemon: {texts:?}"
    );
}

// Unreachable origin fails closed in --once mode with a typed error naming
// the origin; no fabricated live data.
#[test]
fn l03_once_dead_origin_fails_closed() {
    let home = TestHome::new();
    let port = free_loopback_port(); // nothing listening
    let out = tui_once(&home, &format!("http://127.0.0.1:{port}"));
    assert!(!out.status.success(), "dead origin must fail --once");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        stderr.contains("127.0.0.1")
            && (stderr.contains("unreachable")
                || stderr.contains("connect")
                || stderr.contains("offline")),
        "typed connection error naming origin: {stderr}"
    );
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains("(live)"),
        "must not render fabricated live data"
    );
}

// Interactive mode with a dead origin degrades explicitly: the offline banner
// names the daemon, local composer input is still accepted, and the launch
// exits cleanly on :q. Interactive needs a real PTY.
#[test]
fn l04_interactive_dead_origin_degrades_explicitly() {
    let home = TestHome::new();
    let port = free_loopback_port(); // nothing listening
    let origin = format!("http://127.0.0.1:{port}");

    let steps = serde_json::json!([
        { "expect": "daemon offline", "timeout": 30 },
        // A bound `--session` opens the session route, whose truthful status is
        // `Unavailable` when the daemon cannot be reached.
        { "expect": "Unavailable", "timeout": 30 },
        { "send": "offline draft\r" },
        // No daemon, so the submit is reported honestly and the draft is kept
        // (never silently treated as persisted). The native renderer pads
        // between words, so the driver waits on a single token and the full
        // sentence is asserted on the whitespace-normalized capture below.
        { "expect": "preserved", "timeout": 30 },
        { "sleep": 0.3 },
        { "send": "\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}" },
        { "send": ":q\r" },
    ]);
    let out = run_pty(
        &home,
        &["tui", "--origin", &origin, "--session", "00000000-0000-0000-0000-000000000000"],
        &steps,
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let frame = squash(&stdout);
    assert!(
        out.status.success(),
        "clean exit offline: {stdout}{stderr}"
    );
    assert!(
        frame.contains("daemon offline"),
        "explicit offline banner: {stdout}{stderr}"
    );
    assert!(
        frame.contains("Unavailable"),
        "offline status must be truthful, never a fabricated live state: {frame}"
    );
    assert!(
        frame.contains("draft preserved (not retried)")
            && frame.contains("127.0.0.1")
            && (frame.contains("offline") || frame.contains("refusing unauthenticated")),
        "offline submit must be reported explicitly with the origin, never persisted silently: {frame}"
    );
}
