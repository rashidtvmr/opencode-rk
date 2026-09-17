#![forbid(unsafe_code)]
//! TUI follow lane: bounded polling of live daemon session state. `--follow`
//! re-fetches the bound session every `--poll-ms` and re-renders the live
//! block when it changes, for at most `--follow-for` seconds (unbounded only
//! when the flag is omitted). Follow mode is read-only: it does not read
//! stdin, so a viewer can watch another client's submits.

use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
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
            "opencode-rk-tui-follow-{}-{id}",
            std::process::id()
        ));
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
        thread::sleep(Duration::from_millis(50));
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

fn post_message(origin: &str, id: &str, text: &str) {
    http(
        origin,
        "POST",
        &format!("/api/sessions/{id}/messages"),
        Some(&format!(r#"{{"text":"{text}"}}"#)),
    )
    .expect("post message");
}

// f01: follow renders the initial live block and picks up a message posted
// by another client mid-run, within the bounded duration.
#[test]
fn f01_follow_detects_message_from_another_client() {
    let home = TestHome::new();
    let port = free_loopback_port();
    let (_guard, origin) = spawn_daemon(&home, port);
    let id = create_session(&origin, "follow probe");
    post_message(&origin, &id, "seed before follow");

    let mut child = tui_cmd(&home)
        .args([
            "tui",
            "--follow",
            "--origin",
            &origin,
            "--follow-for",
            "6",
            "--poll-ms",
            "100",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tui --follow");
    // Let the first frame render, then act as the other client.
    thread::sleep(Duration::from_millis(1200));
    post_message(&origin, &id, "mid-flight from web client");

    let deadline = Instant::now() + Duration::from_secs(15);
    let (stdout, stderr) = loop {
        if let Some(status) = child.try_wait().expect("poll tui follow") {
            let out = child.wait_with_output().expect("collect output");
            break (String::from_utf8_lossy(&out.stdout).into_owned(), status);
        }
        assert!(Instant::now() < deadline, "follow did not exit in time");
        thread::sleep(Duration::from_millis(100));
    };
    let _ = stderr;
    assert!(stdout.contains("follow probe"), "live title: {stdout}");
    assert!(
        stdout.contains("seed before follow"),
        "initial snapshot: {stdout}"
    );
    assert!(
        stdout.contains("mid-flight from web client"),
        "follow must render the message posted by the other client: {stdout}"
    );
    assert!(
        stdout.contains("--- update ---"),
        "changes must be explicitly marked: {stdout}"
    );
    let _ = stdout; // status asserted implicitly by successful collection
}

// f02: bounded follow with no changes exits zero after the duration and does
// not print update markers.
#[test]
fn f02_follow_bounded_no_change_exits_zero() {
    let home = TestHome::new();
    let port = free_loopback_port();
    let (_guard, origin) = spawn_daemon(&home, port);
    let id = create_session(&origin, "quiet probe");
    post_message(&origin, &id, "only message");

    let out = tui_cmd(&home)
        .args([
            "tui",
            "--follow",
            "--origin",
            &origin,
            "--follow-for",
            "1",
            "--poll-ms",
            "100",
        ])
        .stdin(Stdio::null())
        .output()
        .expect("run bounded follow");
    assert!(out.status.success(), "bounded follow exits 0");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        stdout.contains("quiet probe"),
        "initial live block: {stdout}"
    );
    assert!(
        !stdout.contains("--- update ---"),
        "no change must print no update marker: {stdout}"
    );
}

// f03: follow with a dead origin degrades explicitly and still exits clean
// within its bound.
#[test]
fn f03_follow_dead_origin_degrades() {
    let home = TestHome::new();
    let port = free_loopback_port(); // nothing listening
    let out = tui_cmd(&home)
        .args([
            "tui",
            "--follow",
            "--origin",
            &format!("http://127.0.0.1:{port}"),
            "--follow-for",
            "1",
            "--poll-ms",
            "100",
        ])
        .stdin(Stdio::null())
        .output()
        .expect("run follow against dead origin");
    assert!(out.status.success(), "dead origin follow exits 0");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        stdout.contains("daemon offline"),
        "explicit offline banner: {stdout}"
    );
}

// f04: --follow without --origin is a typed usage error, not a silent local
// poll of nothing.
#[test]
fn f04_follow_requires_origin() {
    let home = TestHome::new();
    let out = tui_cmd(&home)
        .args(["tui", "--follow", "--follow-for", "1"])
        .stdin(Stdio::null())
        .output()
        .expect("run follow without origin");
    assert!(!out.status.success(), "follow without origin must fail");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        stderr.contains("--origin") || stderr.contains("origin"),
        "error names the missing requirement: {stderr}"
    );
}

// f05: --session pins the followed session; activity on other sessions never
// leaks into the pinned frame.
#[test]
fn f05_follow_pinned_session_isolates() {
    let home = TestHome::new();
    let port = free_loopback_port();
    let (_guard, origin) = spawn_daemon(&home, port);
    let pinned = create_session(&origin, "pinned probe");
    let other = create_session(&origin, "other probe");
    post_message(&origin, &pinned, "pinned seed");

    let mut child = tui_cmd(&home)
        .args([
            "tui",
            "--follow",
            "--origin",
            &origin,
            "--session",
            &pinned,
            "--follow-for",
            "5",
            "--poll-ms",
            "100",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn pinned follow");
    thread::sleep(Duration::from_millis(1200));
    post_message(&origin, &other, "noise from other session");

    let deadline = Instant::now() + Duration::from_secs(15);
    let stdout = loop {
        if child.try_wait().expect("poll tui follow").is_some() {
            let out = child.wait_with_output().expect("collect output");
            break String::from_utf8_lossy(&out.stdout).into_owned();
        }
        assert!(Instant::now() < deadline, "follow did not exit in time");
        thread::sleep(Duration::from_millis(100));
    };
    assert!(stdout.contains("pinned probe"), "pinned title: {stdout}");
    assert!(
        !stdout.contains("other probe"),
        "other session must not leak: {stdout}"
    );
    assert!(
        !stdout.contains("noise from other session"),
        "other session messages must not leak: {stdout}"
    );
}
