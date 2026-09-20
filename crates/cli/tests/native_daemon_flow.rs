#![forbid(unsafe_code)]
//! Native daemon flow tests: --native and tui subcommand with daemon discovery,
//! auto-spawn, and live state binding. Each test pins its own daemon port via
//! OPENCODE_RK_DAEMON_ADDR so tests never cross-attach or leak orphans.
//! Frozen after RED — no test edits allowed; fix implementation only.

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
        let path = std::env::temp_dir()
            .join(format!("opencode-rk-native-daemon-{}-{id}", std::process::id()));
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
        let _ = self.handle.take();
    }
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

fn spawn_guarded(mut command: Command) -> (ChildGuard, StdoutReader) {
    let mut child = command.spawn().expect("spawn binary");
    let stdout = StdoutReader::spawn(child.stdout.take().expect("piped stdout"));
    (ChildGuard(child), stdout)
}

/// Descriptor content for a valid backend.json with Bearer token.
fn descriptor_content() -> serde_json::Value {
    serde_json::json!({
        "pid": std::process::id(),
        "http_origin": "http://127.0.0.1:4096",
        "schema_version": 1,
        "auth_token": "ab".repeat(32),
    })
}

/// Spawn an OpenAI-compatible fixture provider on a random port.
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
                        .expect("valid content length");
                    expected_len = Some(header_end + 4 + content_length?);
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

// ---------------------------------------------------------------------------
// T01: --native spawns daemon when none running
// ---------------------------------------------------------------------------
#[test]
fn native_daemon_spawns_when_none_running() {
    let home = TestHome::new();
    let daemon_addr = free_loopback_addr();
    let (provider_base, provider_task) = spawn_openai_fixture();

    let mut command = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    command
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .env("OPENCODE_RK_DAEMON_ADDR", &daemon_addr)
        .args(["--native", "--once"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = command.output().expect("spawn --native --once");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // The --native --once must exit cleanly (exit 0) after spawning the daemon
    // and rendering a frame. The daemon descriptor file must appear under HOME.
    let descriptor_path = home.path().join("runtime/backend.json");
    let descriptor_exists = descriptor_path.exists();

    // Must exit 0
    assert!(
        output.status.success(),
        "--native --once must exit 0 when daemon spawns successfully; stderr:\n{stderr}"
    );

    // Descriptor file must have been created
    assert!(
        descriptor_exists,
        "--native --once must create descriptor file at {:?}; stderr:\n{stderr}",
        descriptor_path
    );

    let _ = provider_task.join();
}

/// ---------------------------------------------------------------------------
// T02: --native with no TTY still takes the native path (not the headless error)
// ---------------------------------------------------------------------------
#[test]
fn native_no_tty_still_takes_native_path() {
    let home = TestHome::new();
    let daemon_addr = free_loopback_addr();
    let (provider_base, provider_task) = spawn_openai_fixture();

    let mut command = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    command
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .env("OPENCODE_RK_DAEMON_ADDR", &daemon_addr)
        // Pipe stdin/stdout to simulate no TTY, but force --native
        .args(["--native", "--once"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = command.output().expect("spawn --native --once with piped I/O");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must not exit with headless error code (2); --native must take NativeTui path
    assert!(
        output.status.success(),
        "--native with piped I/O must not exit headless (code 2); stderr:\n{stderr}\nstdout:\n{stdout}"
    );

    let _ = provider_task.join();
}

// ---------------------------------------------------------------------------
// T03: tui attaches to a running serve daemon without --origin
// ---------------------------------------------------------------------------
#[test]
fn tui_attaches_to_running_serve_daemon() {
    let home = TestHome::new();
    let daemon_addr = free_loopback_addr();

    // First, start a daemon via serve subcommand
    let mut serve_cmd = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    serve_cmd
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .args(["serve", "--listen", &daemon_addr])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut serve_child = serve_cmd.spawn().expect("spawn serve daemon");
    let _ = std::thread::sleep(Duration::from_secs(2)); // wait for daemon to init

    // Now run `opencode-rk tui` without --origin, should attach to running daemon
    let mut tui_cmd = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    tui_cmd
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .env("OPENCODE_RK_DAEMON_ADDR", &daemon_addr)
        .arg("tui")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let (mut tui, stdout_reader) = spawn_guarded(tui_cmd);
    // Wait for the TUI to start and detect the daemon
    let transcript = stdout_reader.wait_for("OpenCode RK", Duration::from_secs(10));

    // Should not show offline message
    assert!(
        !transcript.contains("[offline]"),
        "tui attaching to running daemon must not show offline message; transcript:\n{transcript}"
    );

    // Exit cleanly
    send_line(&mut tui, "/exit");
    let status = tui.wait().expect("tui exits cleanly");
    assert!(status.success(), "tui must exit 0 after /exit");

    let _ = serve_child.kill();
    let _ = serve_child.wait();
}

// ---------------------------------------------------------------------------
// T04: status frame carries live model/token values from the bound daemon
// ---------------------------------------------------------------------------
#[test]
fn status_frame_carries_live_daemon_values() {
    let home = TestHome::new();
    let daemon_addr = free_loopback_addr();
    let (provider_base, provider_task) = spawn_openai_fixture();

    // Start daemon with descriptor containing auth token
    let mut daemon_cmd = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    daemon_cmd
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .env("OPENCODE_RK_DAEMON_ADDR", &daemon_addr)
        .args(["serve", "--listen", &daemon_addr])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut daemon_child = daemon_cmd.spawn().expect("spawn daemon");

    // Wait for daemon to write descriptor
    std::thread::sleep(Duration::from_secs(2));

    // Write a proper descriptor with auth token under HOME
    let descriptor_path = home.path().join("runtime/backend.json");
    fs::create_dir_all(descriptor_path.parent().unwrap()).expect("runtime dir");
    fs::write(&descriptor_path, serde_json::to_string(&descriptor_content()).unwrap())
        .expect("write descriptor");

    // Run --native --once to render a frame with live binding
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    cmd.env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .env("OPENCODE_RK_DAEMON_ADDR", &daemon_addr)
        .args(["--native", "--once"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output().expect("spawn --native --once");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // The frame must contain live model/token info from the bound daemon,
    // not just "unset" / "0 tokens"
    let has_live_model = stdout.contains("model:") && !stdout.trim().contains("model: unset");
    // Also check for token/credential evidence in the status bar
    let has_token_evidence = stdout.contains("tokens");

    // Must exit 0
    assert!(
        output.status.success(),
        "--native --once must exit 0 with live daemon; stderr:\n{stderr}\nstdout:\n{stdout}"
    );

    // Output must carry live model values (not just "unset")
    assert!(
        has_live_model,
        "--native --once with bound daemon must render live model values, not just 'unset'; stdout:\n{stdout}"
    );

    let _ = provider_task.join();
    let _ = daemon_child.kill();
    let _ = daemon_child.wait();
}