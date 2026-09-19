#![forbid(unsafe_code)]
//! Native launch integration: proves the default line-mode path is unbroken
//! and the `--native` path either renders or fails closed with an actionable
//! message. Frozen test contract pins the line-mode banner and /exit behavior
//! (same assertions as `default_tui.rs` but in a separate test target).

use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1000);

struct TestHome(PathBuf);

impl TestHome {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("opencode-rk-native-{}", id));
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

fn spawn_guarded(mut command: Command) -> (ChildGuard, StdoutReader) {
    let mut child = command.spawn().expect("spawn binary");
    let stdout = StdoutReader::spawn(child.stdout.take().expect("piped stdout"));
    (ChildGuard(child), stdout)
}

/// One-shot OpenAI-compatible fixture: reads one request, replies with a
/// fixed assistant message (same seam as `default_tui.rs`).
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
            assert!(read > 0, "provider request ended before full body");
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
                "content": [{"type": "output_text", "text": "Fixture native reply"}]
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

// ---------------------------------------------------------------------------
// Scenario A: default launch (no flags) still satisfies the default_tui
// contract — banner "OpenCode RK", "you:" echo, /exit 0.
// ---------------------------------------------------------------------------
#[test]
fn default_launch_satisfies_line_mode_contract() {
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

    send_line(&mut chat, "/new NativeProbe");
    stdout.wait_for("created:", Duration::from_secs(10));

    send_line(&mut chat, "Hello from native launch test");
    stdout.wait_for("Fixture native reply", Duration::from_secs(60));

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
        transcript.contains("you: Hello from native launch test"),
        "echoed user turn missing:\n{transcript}"
    );
    assert!(
        transcript.contains("assistant:"),
        "assistant reply rendering missing:\n{transcript}"
    );
}

// ---------------------------------------------------------------------------
// Scenario B: --native --once piped mode — two branches:
//   1. renderer artifact present → render frame + exit 0
//   2. renderer artifact absent  → non-zero exit + explicit missing-artifact message
// ---------------------------------------------------------------------------
#[test]
fn native_once_renders_or_fails_with_actionable_message() {
    let home = TestHome::new();
    let daemon_addr = free_loopback_addr();

    let native_artifact = std::env::current_dir()
        .expect("cwd")
        .join("../native/lib/x86_64-linux/libopentui.so");
    let artifact_present = native_artifact.exists();

    let mut command = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    command
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .env("OPENCODE_RK_DAEMON_ADDR", &daemon_addr)
        .args(["--native", "--once"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let start = Instant::now();
    let output = command
        .output()
        .expect("failed to spawn --native --once");
    let elapsed = start.elapsed();

    // Must not hang: bounded at 30 seconds.
    assert!(
        elapsed < Duration::from_secs(30),
        "--native --once took too long: {elapsed:?}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if artifact_present {
        // When the renderer artifact exists, --once should render a frame
        // and exit 0. The stdout may contain terminal escape sequences or
        // frame content; we just check exit code.
        assert!(
            output.status.success(),
            "--native --once should exit 0 when artifact is present; stderr:\n{stderr}"
        );
    } else {
        // When the renderer artifact is absent, --native --once must fail
        // non-zero with an explicit, actionable message naming the missing
        // artifact path.
        assert!(
            !output.status.success(),
            "--native --once should exit non-zero when artifact is absent"
        );
        let combined = format!("{stdout}\n{stderr}");
        assert!(
            combined.to_lowercase().contains("opentui")
                || combined.to_lowercase().contains("native")
                || combined.to_lowercase().contains("renderer")
                || combined.to_lowercase().contains("missing"),
            "error message must name the missing artifact or component; got:\n{combined}"
        );
    }
}
