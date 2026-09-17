#![forbid(unsafe_code)]
//! TUI live-state lane: composer/status frame bound to real daemon session
//! state over the singleton HTTP API. Binary-driven; spawns a real daemon on
//! a free loopback port with a disposable home.

use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct TestHome(PathBuf);

impl TestHome {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("opencode-rk-tui-live-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
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

/// Spawn a web-owned daemon and return its origin once the page is served.
fn spawn_daemon(home: &TestHome, port: u16) -> (ChildGuard, String) {
    let child = Command::new(env!("CARGO_BIN_EXE_opencode-rk"))
        .env_clear()
        .env("OPENCODE_RK_HOME", home.0.clone())
        .args(["web", "--no-open", "--listen", &format!("127.0.0.1:{port}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn daemon");
    let guard = ChildGuard(child);
    let origin = format!("http://127.0.0.1:{port}");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if http(&origin, "GET", "/health", None).is_some() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "daemon did not start at {origin}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    (guard, origin)
}

fn tui_cmd(home: &TestHome) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    cmd.env_clear()
        .env("OPENCODE_RK_HOME", home.0.clone())
        .env("NO_COLOR", "1");
    cmd
}

/// Minimal bounded HTTP/1.1 client for tests. Returns full body on 2xx.
fn http(origin: &str, method: &str, path: &str, body: Option<&str>) -> Option<String> {
    let mut stream = TcpStream::connect(origin.trim_start_matches("http://")).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(3))).ok()?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .ok()?;
    let payload = body.unwrap_or("");
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {origin}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(req.as_bytes()).ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    let status = response.split_whitespace().nth(1)?;
    if !status.starts_with('2') {
        return None;
    }
    let body_start = response.find("\r\n\r\n")? + 4;
    Some(response[body_start..].to_string())
}

fn create_session(origin: &str, title: &str) -> String {
    let body = http(
        origin,
        "POST",
        "/api/sessions",
        Some(&format!(r#"{{"title":"{title}"}}"#)),
    )
    .expect("create session");
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    value["session"]["id"]
        .as_str()
        .expect("session id")
        .to_string()
}

// Live snapshot: --once with --origin renders the real session title and
// message count from the daemon instead of local placeholders.
#[test]
fn l01_once_with_origin_renders_live_session() {
    let home = TestHome::new();
    let port = free_loopback_port();
    let (_guard, origin) = spawn_daemon(&home, port);

    let id = create_session(&origin, "live probe");
    http(
        &origin,
        "POST",
        &format!("/api/sessions/{id}/messages"),
        Some(r#"{"text":"seed message"}"#),
    )
    .expect("seed message");

    let out = tui_cmd(&home)
        .args(["tui", "--once", "--origin", &origin])
        .output()
        .expect("run tui --once --origin");
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

// Interactive submit posts to the daemon; the message is durably persisted
// and visible in a later snapshot.
#[test]
fn l02_interactive_submit_persists_via_daemon() {
    let home = TestHome::new();
    let port = free_loopback_port();
    let (_guard, origin) = spawn_daemon(&home, port);
    let id = create_session(&origin, "interactive probe");

    let mut child = tui_cmd(&home)
        .args(["tui", "--origin", &origin])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tui");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"hello daemon\n:q\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(stdout.contains("you: hello daemon"), "echo: {stdout}");

    // Durable: a fresh snapshot must show the persisted message.
    let body = http(
        &origin,
        "GET",
        &format!("/api/sessions/{id}/messages?limit=10"),
        None,
    )
    .expect("read messages");
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    let texts: Vec<&str> = value["messages"]
        .as_array()
        .expect("messages array")
        .iter()
        .filter_map(|m| m["body"]["text"].as_str())
        .collect();
    assert!(
        texts.iter().any(|t| t.contains("hello daemon")),
        "submit must persist via daemon: {texts:?}"
    );
}

// Unreachable origin fails closed in --once mode with a typed error naming
// the origin; no fabricated live data.
#[test]
fn l03_once_dead_origin_fails_closed() {
    let home = TestHome::new();
    let port = free_loopback_port(); // nothing listening
    let out = tui_cmd(&home)
        .args([
            "tui",
            "--once",
            "--origin",
            &format!("http://127.0.0.1:{port}"),
        ])
        .output()
        .expect("run tui with dead origin");
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

// Interactive mode with a dead origin degrades explicitly: an offline banner
// is shown, and local composer commands still work; exit is clean.
#[test]
fn l04_interactive_dead_origin_degrades_explicitly() {
    let home = TestHome::new();
    let port = free_loopback_port(); // nothing listening
    let mut child = tui_cmd(&home)
        .args(["tui", "--origin", &format!("http://127.0.0.1:{port}")])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tui offline");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"offline draft\n:q\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        stdout.contains("daemon offline") || stdout.contains("offline"),
        "explicit offline banner: {stdout}{stderr}"
    );
    assert!(
        stdout.contains("you: offline draft"),
        "local composer still works offline: {stdout}{stderr}"
    );
    assert!(out.status.success(), "clean exit offline: {stdout}{stderr}");
}
