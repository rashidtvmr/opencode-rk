#![forbid(unsafe_code)]
//! Default-entrypoint chat lane: `opencode-rk` with no subcommand must open an
//! interactive chat TUI that auto-attaches (or auto-spawns) the singleton
//! daemon, executes real provider turns, and persists sessions. Binary-driven
//! with a disposable home and a fake OpenAI-compatible provider fixture.
//! Each test pins its own daemon port via OPENCODE_RK_DAEMON_ADDR so tests
//! never cross-attach or leak orphans.

use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
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
        let path =
            std::env::temp_dir().join(format!("opencode-rk-default-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
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

impl std::ops::Deref for ChildGuard {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}

impl std::ops::DerefMut for ChildGuard {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

fn free_loopback_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    format!("127.0.0.1:{}", listener.local_addr().unwrap().port())
}

/// Background reader accumulating child stdout so tests can wait for markers
/// without blocking on a pipe read.
struct StdoutReader {
    buffer: std::sync::Arc<std::sync::Mutex<String>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl StdoutReader {
    fn spawn(pipe: impl Read + Send + 'static) -> Self {
        let buffer = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let shared = std::sync::Arc::clone(&buffer);
        let handle = thread::spawn(move || {
            let mut pipe = pipe;
            let mut buf = [0_u8; 4096];
            loop {
                match pipe.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        let mut guard = shared.lock().unwrap();
                        guard.push_str(&String::from_utf8_lossy(&buf[..read]));
                    }
                }
            }
        });
        Self {
            buffer,
            handle: Some(handle),
        }
    }

    fn text(&self) -> String {
        self.buffer.lock().unwrap().clone()
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

impl Drop for StdoutReader {
    fn drop(&mut self) {
        // Deliberately detached: joining here can block forever when the
        // child outlives the test (e.g. a panic while the chat loop waits on
        // stdin). Leaking the thread is bounded and keeps teardown live.
        let _ = self.handle.take();
    }
}

/// One-shot OpenAI-compatible /v1/responses fixture: reads exactly one
/// content-length framed request, replies with a fixed assistant message
/// (same seam as the server's own turn tests).
fn spawn_openai_fixture() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind provider fixture");
    let address = listener.local_addr().expect("fixture address");
    let task = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept provider request");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("fixture read timeout");
        let mut request = Vec::new();
        let mut expected_len = None;
        loop {
            let mut chunk = [0_u8; 4096];
            let read = stream.read(&mut chunk).expect("read provider request");
            assert!(read > 0, "provider request ended before the full body");
            request.extend_from_slice(&chunk[..read]);
            if expected_len.is_none() {
                if let Some(header_end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(str::trim)
                                .map(str::parse::<usize>)
                        })
                        .transpose()
                        .expect("valid content length")
                        .unwrap_or(0);
                    expected_len = Some(header_end + 4 + content_length);
                }
            }
            if expected_len.is_some_and(|len| request.len() >= len) {
                break;
            }
            assert!(request.len() <= 128 * 1024, "fixture request bound");
        }
        let body = serde_json::json!({
            "id": "resp_fixture",
            "status": "completed",
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "Fixture assistant reply"}]
            }]
        })
        .to_string();
        let wire = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(wire.as_bytes());
        let _ = stream.flush();
    });
    (format!("http://{address}/v1"), task)
}

fn chat_command(home: &TestHome, daemon_addr: &str, extra_env: &[(&str, &str)]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    command
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .env("OPENCODE_RK_DAEMON_ADDR", daemon_addr)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (key, value) in extra_env {
        command.env(key, value);
    }
    command
}

fn send_line(child: &mut Child, line: &str) {
    let stdin = child.stdin.as_mut().expect("piped stdin");
    writeln!(stdin, "{line}").expect("write chat line");
    stdin.flush().expect("flush chat line");
}

/// Ensure the chat child never outlives a panicking test: a leaked child
/// holding the stdout pipe would hang the test harness at teardown.
fn spawn_guarded(mut command: Command) -> (ChildGuard, StdoutReader) {
    let mut child = command.spawn().expect("spawn chat TUI");
    let stdout = StdoutReader::spawn(child.stdout.take().expect("piped stdout"));
    (ChildGuard(child), stdout)
}

#[test]
fn bare_launch_opens_chat_tui_and_completes_a_provider_turn() {
    let home = TestHome::new();
    let daemon_addr = free_loopback_addr();
    let (provider_base, provider_task) = spawn_openai_fixture();

    let command = chat_command(
        &home,
        &daemon_addr,
        &[
            ("OPENAI_BASE_URL", provider_base.as_str()),
            ("OPENAI_API_KEY", "fixture-secret"),
        ],
    );
    let (mut chat, stdout) = spawn_guarded(command);

    stdout.wait_for("OpenCode RK", Duration::from_secs(20));
    stdout.wait_for("model:", Duration::from_secs(5));

    send_line(&mut chat, "/new Probe");
    stdout.wait_for("created:", Duration::from_secs(10));

    send_line(&mut chat, "Hello agent");
    stdout.wait_for("Fixture assistant reply", Duration::from_secs(60));

    send_line(&mut chat, "/exit");
    let status = chat.wait().expect("chat exits cleanly after /exit");
    let _ = provider_task.join();
    assert!(
        status.success(),
        "chat TUI must exit 0 after /exit; stdout:\n{}",
        stdout.text()
    );
    let transcript = stdout.text();
    assert!(
        transcript.contains("you: Hello agent"),
        "echoed user turn missing:\n{transcript}"
    );
    assert!(
        transcript.contains("assistant:"),
        "assistant reply rendering missing:\n{transcript}"
    );
}

#[test]
fn chat_tui_reports_missing_provider_auth_as_turn_error() {
    let home = TestHome::new();
    let daemon_addr = free_loopback_addr();

    let command = chat_command(&home, &daemon_addr, &[]);
    let (mut chat, stdout) = spawn_guarded(command);

    stdout.wait_for("OpenCode RK", Duration::from_secs(20));

    send_line(&mut chat, "/new AuthProbe");
    stdout.wait_for("created:", Duration::from_secs(10));

    send_line(&mut chat, "hi");
    let transcript = stdout.wait_for("[error", Duration::from_secs(60));
    assert!(
        transcript.to_lowercase().contains("provider"),
        "turn error must name the provider failure:\n{transcript}"
    );

    send_line(&mut chat, "/exit");
    let status = chat.wait().expect("chat exits cleanly");
    assert!(status.success());
}

#[test]
fn chat_tui_auto_spawns_daemon_and_sessions_persist_after_exit() {
    let home = TestHome::new();
    let daemon_addr = free_loopback_addr();

    let command = chat_command(&home, &daemon_addr, &[]);
    let (mut chat, stdout) = spawn_guarded(command);

    stdout.wait_for("OpenCode RK", Duration::from_secs(20));

    send_line(&mut chat, "/new Saved Chat");
    stdout.wait_for("created:", Duration::from_secs(10));

    send_line(&mut chat, "/exit");
    let status = chat.wait().expect("chat exits cleanly");
    assert!(status.success());

    // The auto-spawned daemon must persist the session for later processes.
    let listing = Command::new(env!("CARGO_BIN_EXE_opencode-rk"))
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .args(["session", "list"])
        .output()
        .expect("list sessions after chat exit");
    assert!(listing.status.success());
    let list = String::from_utf8_lossy(&listing.stdout);
    assert!(
        list.contains("Saved Chat"),
        "session created in the chat TUI must persist: {list}"
    );
}

#[test]
fn chat_tui_offline_hint_when_daemon_cannot_start() {
    let home = TestHome::new();
    // Occupy the default daemon port so the in-process auto-spawn cannot bind.
    let blocker = TcpListener::bind("127.0.0.1:4096")
        .expect("bind default daemon port to force offline degradation");

    let command = chat_command(&home, "127.0.0.1:4096", &[]);
    let (mut chat, stdout) = spawn_guarded(command);

    let transcript = stdout.wait_for("[offline", Duration::from_secs(30));
    assert!(
        transcript.contains("serve"),
        "offline hint must tell the user how to start the daemon:\n{transcript}"
    );

    send_line(&mut chat, "/exit");
    let status = chat.wait().expect("chat exits cleanly");
    assert!(status.success());
    drop(blocker);
}

/// E2E NATIVE TUI: native OpenTUI bridge render_once produces non-empty snapshot
/// with frame content (OpenCode RK TUI, status bar, composer). Tests the
/// real Rust caller for opentui_bridge without requiring the .so library in CI.
#[test]
fn native_render_once_snapshot_contains_frame_content() {
    // Native feature is optional; skip if not compiled with --features native
    if std::env::var_os("CARGO_FEATURE_NATIVE").is_none() {
        eprintln!("skipping: native feature not enabled");
        return;
    }

    // Use render_once directly to verify frame content
    use opencode_rk_opentui_bridge::Renderer;
    let frame_lines = vec![
        "OpenCode RK TUI".to_string(),
        "status: ready".to_string(),
        "> ".to_string(),
    ];
    let snapshot = Renderer::render_once(80, 24, &frame_lines);
    assert!(
        snapshot.is_ok(),
        "render_once should succeed with valid input"
    );
    let output = snapshot.unwrap();
    assert!(
        !output.trim().is_empty(),
        "render_once snapshot must be non-empty"
    );
    assert!(
        output.contains("OpenCode RK"),
        "render_once output must contain frame text: {output}"
    );
}

/// E2E: --once path with Bearer auth renders native snapshot on success.
#[test]
fn once_mode_with_bearer_auth_shows_native_or_fallback() {
    if std::env::var_os("CARGO_FEATURE_NATIVE").is_none() {
        eprintln!("skipping: native feature not enabled");
        return;
    }

    // Start daemon with test bearer
    let home = TestHome::new();
    let port = free_loopback_addr();

    // Spawn daemon with a test descriptor
    let mut daemon = Command::new(env!("CARGO_BIN_EXE_opencode-rk"))
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .env("OPENCODE_RK_DAEMON_ADDR", &port)
        .args(["serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn daemon");

    // Wait for daemon to initialize (write descriptor file)
    thread::sleep(Duration::from_secs(2));

    // Create a test descriptor file (64-hex bearer per is_wellformed_token)
    let descriptor_path = home.path().join("runtime/backend.json");
    fs::create_dir_all(descriptor_path.parent().unwrap()).expect("runtime dir");
    let token: String = "ab".repeat(32);
    let descriptor_content = serde_json::json!({
        "pid": daemon.id(),
        "http_origin": format!("http://127.0.0.1:{}", port),
        "schema_version": 1,
        "auth_token": token
    });
    fs::write(
        &descriptor_path,
        serde_json::to_string(&descriptor_content).unwrap(),
    )
    .expect("write descriptor");

    // Run tui --once via the subcommand path and verify frame content
    let result = Command::new(env!("CARGO_BIN_EXE_opencode-rk"))
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .args(["tui", "--once"])
        .output()
        .expect("run tui --once");

    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(
        result.status.success(),
        "tui --once should exit successfully: stderr={stderr}"
    );
    assert!(
        stdout.contains("OpenCode RK TUI"),
        "tui --once must render frame content: stdout={stdout} stderr={stderr}"
    );

    // Cleanup: kill daemon
    let _ = daemon.kill();
    let _ = daemon.wait();
}
