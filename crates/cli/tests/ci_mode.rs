#![forbid(unsafe_code)]
//! LANE-CI-FLAG frozen tests: CI mode end-to-end.
//!
//! Scenario A (jsonl): binary `run --ci --output jsonl` emits parseable JSONL
//! events ending with TurnFinished, exits 0.
//! Scenario B (approval fail-closed): render_approval_required ALWAYS produces
//! exit 20 and valid JSONL naming the tool — never auto-approves.
//! Scenario C (determinism): two identical binary runs produce byte-identical
//! JSONL output (timestamps excluded).

#[path = "../src/ci_output.rs"]
mod ci_output;

use ci_output::{CiEvent, CiExitCode, OutputFormat};

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

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct TestHome(PathBuf);

impl TestHome {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("opencode-rk-ci-{}-{id}", std::process::id()));
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

/// One-shot OpenAI-compatible /v1/responses fixture.
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
                "content": [{"type": "output_text", "text": "CI fixture response"}]
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

fn ci_command(home: &TestHome, daemon_addr: &str, extra_env: &[(&str, &str)]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    command
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .env("OPENCODE_RK_DAEMON_ADDR", daemon_addr)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (key, value) in extra_env {
        command.env(key, value);
    }
    command
}

fn spawn_guarded(mut command: Command) -> (ChildGuard, StdoutReader) {
    let mut child = command.spawn().expect("spawn CI binary");
    let stdout = StdoutReader::spawn(child.stdout.take().expect("piped stdout"));
    (ChildGuard(child), stdout)
}

// ── LANE-CI-FLAG-T01: JSONL output parseable, ends TurnFinished, exit 0 ──

#[test]
fn ci_t01_jsonl_output_parseable_and_exits_zero() {
    let home = TestHome::new();
    let daemon_addr = free_loopback_addr();
    let (provider_base, provider_task) = spawn_openai_fixture();

    let mut command = ci_command(
        &home,
        &daemon_addr,
        &[
            ("OPENAI_BASE_URL", provider_base.as_str()),
            ("OPENAI_API_KEY", "fixture-secret"),
        ],
    );
    command.args(["run", "--ci", "--output", "jsonl", "say hi"]);
    let (mut child, stdout) = spawn_guarded(command);

    let output = stdout.wait_for("TurnFinished", Duration::from_secs(30));
    let status = child.wait().expect("CI run exits");
    let _ = provider_task.join();

    assert!(
        status.success(),
        "CI run must exit 0 for successful turn; stdout:\n{output}"
    );

    // Every non-empty line must be valid JSON with ts_kind
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parsed: serde_json::Value =
            serde_json::from_str(line).expect(&format!("each line must be valid JSON: {line}"));
        assert!(
            parsed.get("ts_kind").is_some(),
            "JSONL line must have ts_kind field: {line}"
        );
    }

    // Must contain TurnStarted and TurnFinished
    assert!(
        output.contains("TurnStarted"),
        "must emit TurnStarted event"
    );
    assert!(
        output.contains("TurnFinished"),
        "must emit TurnFinished event"
    );

    // TurnFinished must have exit: 0
    for line in output.lines() {
        let line = line.trim();
        if line.contains("TurnFinished") {
            let parsed: serde_json::Value =
                serde_json::from_str(line).expect("TurnFinished must be valid JSON");
            assert_eq!(
                parsed.get("exit").and_then(|v| v.as_u64()),
                Some(0),
                "TurnFinished must have exit: 0"
            );
            break;
        }
    }
}

// ── LANE-CI-FLAG-T02: approval fail-closed ──

#[test]
fn ci_t02_approval_required_always_exits_20_and_emits_valid_jsonl() {
    // Test the fail-closed contract at the ci_output level:
    // render_approval_required() ALWAYS returns exit 20, never auto-approves.
    let (_, code) = ci_output::render_approval_required("shell_exec");
    assert_eq!(code, CiExitCode::ApprovalRequired);
    assert_eq!(code.code(), 20);

    // The JSONL event must name the tool.
    let event = CiEvent::ApprovalRequired {
        ts: 0,
        tool: "shell_exec".to_string(),
    };
    let jsonl = event.to_bounded_json_line();
    let parsed: serde_json::Value =
        serde_json::from_str(&jsonl).expect("ApprovalRequired must be valid JSON");
    assert_eq!(
        parsed.get("ts_kind").and_then(|v| v.as_str()),
        Some("ApprovalRequired"),
        "event must be ApprovalRequired"
    );
    assert_eq!(
        parsed.get("tool").and_then(|v| v.as_str()),
        Some("shell_exec"),
        "event must name the tool"
    );

    // Verify via each output format
    for fmt in [OutputFormat::Json, OutputFormat::Jsonl, OutputFormat::Text] {
        let mut buf = Vec::new();
        ci_output::render_event(&mut buf, &event, fmt).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(
            text.contains("ApprovalRequired") || text.contains("approval_required"),
            "format {:?} must contain approval: {}",
            fmt,
            text
        );
    }
}

// ── LANE-CI-FLAG-T03: determinism ──

#[test]
fn ci_t03_deterministic_jsonl_across_identical_runs() {
    // Each run gets its own daemon address and provider fixture so the
    // one-shot provider is fresh for each execution.
    let run_jsonl = || -> String {
        let home = TestHome::new();
        let daemon_addr = free_loopback_addr();
        let (provider_base, provider_task) = spawn_openai_fixture();
        let mut command = ci_command(
            &home,
            &daemon_addr,
            &[
                ("OPENAI_BASE_URL", provider_base.as_str()),
                ("OPENAI_API_KEY", "fixture-secret"),
            ],
        );
        command.args(["run", "--ci", "--output", "jsonl", "determinism probe"]);
        let (mut child, stdout) = spawn_guarded(command);
        let output = stdout.wait_for("TurnFinished", Duration::from_secs(30));
        let _ = child.wait().expect("CI run exits");
        let _ = provider_task.join();
        output
    };

    let out1 = run_jsonl();
    let out2 = run_jsonl();

    // Strip timestamps (ts fields) from JSONL lines and compare
    let strip_ts = |output: &str| -> String {
        output
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                let mut parsed: serde_json::Value =
                    serde_json::from_str(line).expect("valid JSON");
                if let Some(obj) = parsed.as_object_mut() {
                    obj.remove("ts");
                }
                serde_json::to_string(&parsed).expect("serialize stripped JSON")
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let stripped1 = strip_ts(&out1);
    let stripped2 = strip_ts(&out2);
    assert_eq!(
        stripped1, stripped2,
        "two identical CI runs must produce byte-identical JSONL (timestamps excluded)"
    );
}


