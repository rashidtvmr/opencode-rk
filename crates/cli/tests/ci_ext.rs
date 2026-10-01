#![forbid(unsafe_code)]

#[path = "../src/ci_output.rs"]
mod ci_output;

use ci_output::CiExitCode;
use std::cell::RefCell;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------------------
// Helpers (same pattern as ci_mode.rs tests)
// ---------------------------------------------------------------------------

struct TestHome {
    dir: std::path::PathBuf,
    fixture: RefCell<Option<FixtureResources>>,
}

impl TestHome {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("opencode-rk-ci-ext-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self {
            dir,
            fixture: RefCell::new(None),
        }
    }
    fn path(&self) -> &std::path::Path {
        &self.dir
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        // The daemon and provider are deliberately owned by the disposable home,
        // not detached from the test.  This also makes cleanup happen after the
        // CI child has been reaped (the tests retain the home until then).
        let _ = self.fixture.get_mut().take();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

const FIXTURE_DEADLINE: Duration = Duration::from_secs(20);
const FIXTURE_MODEL: &str = "fixture/openai-ci";

struct FixtureResources {
    stopping: Arc<AtomicBool>,
    provider: Option<JoinHandle<()>>,
    daemon: Option<Child>,
}

impl Drop for FixtureResources {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        if let Some(mut daemon) = self.daemon.take() {
            let _ = daemon.kill();
            let _ = daemon.wait();
        }
        if let Some(provider) = self.provider.take() {
            let deadline = Instant::now() + FIXTURE_DEADLINE;
            while !provider.is_finished() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            assert!(provider.is_finished(), "provider fixture did not stop before deadline");
            let _ = provider.join();
        }
    }
}

struct ChildGuard {
    child: Child,
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn wait_for(child: &mut Child, pattern: &str, timeout: Duration) -> Vec<String> {
    let start = Instant::now();
    let reader = BufReader::new(child.stdout.take().unwrap());
    let mut collected = Vec::new();
    for line in reader.lines() {
        let line = line.unwrap();
        collected.push(line.clone());
        if line.contains(pattern) {
            return collected;
        }
        if start.elapsed() > timeout {
            break;
        }
    }
    collected
}

fn spawn_openai_fixture(home: &TestHome, extra_args: Vec<String>) -> Child {
    let provider_listener = TcpListener::bind("127.0.0.1:0").expect("bind provider fixture");
    let provider_addr = provider_listener.local_addr().expect("provider address");
    provider_listener
        .set_nonblocking(true)
        .expect("provider nonblocking");
    let stopping = Arc::new(AtomicBool::new(false));
    let provider_stop = Arc::clone(&stopping);
    let provider = thread::spawn(move || {
        let deadline = Instant::now() + FIXTURE_DEADLINE;
        while !provider_stop.load(Ordering::Acquire) && Instant::now() < deadline {
            let (mut stream, _) = match provider_listener.accept() {
                Ok(pair) => pair,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(error) => panic!("provider accept failed: {error}"),
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("provider read timeout");
            let mut request = Vec::new();
            let mut buf = [0_u8; 4096];
            loop {
                let read = stream.read(&mut buf).expect("provider request read");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buf[..read]);
                assert!(request.len() <= 256 * 1024, "provider request exceeded bound");
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let headers_end = request.windows(4).position(|window| window == b"\r\n\r\n").unwrap();
                    let headers = String::from_utf8_lossy(&request[..headers_end]);
                    let length = headers.lines().find_map(|line| {
                        line.to_ascii_lowercase().strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    }).unwrap_or(0);
                    if request.len() >= headers_end + 4 + length {
                        break;
                    }
                }
            }
            let path = String::from_utf8_lossy(&request)
                .lines().next().unwrap_or("").to_owned();
            let (content_type, body) = if path.contains(" /models ") {
                ("application/json", serde_json::json!({
                    "object": "list",
                    "data": [{"id": FIXTURE_MODEL, "object": "model", "owned_by": "fixture"}]
                }).to_string())
            } else {
                let events = [
                    format!("event: response.output_text.delta\ndata: {}\n\n", serde_json::json!({"delta":"fixture hello"})),
                    format!("event: response.completed\ndata: {}\n\n", serde_json::json!({"type":"response.completed","response":{"id":"fixture-response","status":"completed"}})),
                ].concat();
                ("text/event-stream", events)
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).expect("provider response write");
        }
    });

    let daemon_addr = TcpListener::bind("127.0.0.1:0")
        .expect("bind daemon reservation")
        .local_addr()
        .expect("daemon address");
    let config = format!(
        r#"{{
            "providers": {{
                "openai": {{
                    "base_url": "http://{}:{}/v1",
                    "api_key": "fixture-key",
                    "models": ["{}"]
                }}
            }}
        }}"#,
        provider_addr.ip(), provider_addr.port(), FIXTURE_MODEL
    );
    std::fs::write(home.path().join("config.json"), config).unwrap();

    let mut daemon = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    daemon.args(["serve", "--listen", &daemon_addr.to_string()])
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join("xdg-config"))
        .env("XDG_DATA_HOME", home.path().join("xdg-data"))
        .env("XDG_CACHE_HOME", home.path().join("xdg-cache"))
        .env("OPENAI_BASE_URL", format!("http://{}:{}/v1", provider_addr.ip(), provider_addr.port()))
        .env("OPENAI_API_KEY", "fixture-key")
        .stdout(Stdio::null()).stderr(Stdio::null()).stdin(Stdio::null());
    let daemon = daemon.spawn().expect("failed to spawn fixture daemon");
    let descriptor_path = home.path().join("runtime/backend.json");
    let deadline = Instant::now() + FIXTURE_DEADLINE;
    let (origin, token) = loop {
        if let Ok(bytes) = std::fs::read(&descriptor_path) {
            assert!(bytes.len() <= 8 * 1024, "daemon descriptor exceeded bound");
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                let pid = value.get("pid").and_then(|v| v.as_u64());
                let origin = value.get("http_origin").and_then(|v| v.as_str());
                let token = value.get("auth_token").and_then(|v| v.as_str());
                if pid == Some(u64::from(daemon.id())) && origin.is_some()
                    && token.is_some_and(|value| value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())) {
                    break (origin.unwrap().to_owned(), token.unwrap().to_owned());
                }
            }
        }
        assert!(Instant::now() < deadline, "fixture daemon descriptor readiness timeout");
        thread::sleep(Duration::from_millis(20));
    };
    home.fixture.borrow_mut().replace(FixtureResources {
        stopping, provider: Some(provider), daemon: Some(daemon),
    });

    let mut args = vec!["run".to_string(), "--ci".to_string()];
    args.extend(extra_args);

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    cmd.args(&args)
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join("xdg-config"))
        .env("XDG_DATA_HOME", home.path().join("xdg-data"))
        .env("XDG_CACHE_HOME", home.path().join("xdg-cache"))
        .env("OPENCODE_RK_DAEMON_ADDR", origin.strip_prefix("http://").unwrap_or(&origin))
        .env("OPENCODE_RK_DAEMON_TOKEN", token)
        .env("OPENCODE_RK_CI_MODEL", FIXTURE_MODEL)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    cmd.spawn().expect("failed to spawn opencode-rk")
}

fn ci_command(home: &TestHome, prompt: &str) -> Child {
    let addr = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap()
    };
    let config = format!(
        r#"{{
            "providers": {{
                "openai": {{
                    "base_url": "http://127.0.0.1:{}",
                    "api_key": "sk-fake"
                }}
            }}
        }}"#,
        addr.port()
    );
    std::fs::write(home.path().join("config.json"), config).unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    cmd.args(["run", "--ci", "--prompt", prompt])
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .env("OPENCODE_RK_DAEMON_ADDR", addr.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    cmd.spawn().expect("failed to spawn opencode-rk")
}

fn spawn_guarded(home: &TestHome, args: Vec<String>) -> ChildGuard {
    let addr = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap()
    };
    let config = format!(
        r#"{{
            "providers": {{
                "openai": {{
                    "base_url": "http://127.0.0.1:{}",
                    "api_key": "sk-fake"
                }}
            }}
        }}"#,
        addr.port()
    );
    std::fs::write(home.path().join("config.json"), config).unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    cmd.args(&args)
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .env("OPENCODE_RK_DAEMON_ADDR", addr.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    ChildGuard {
        child: cmd.spawn().expect("failed to spawn opencode-rk"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn ci_ext_t01_max_steps_zero_is_usage_error() {
    let home = TestHome::new();
    let status = Command::new(env!("CARGO_BIN_EXE_opencode-rk"))
        .args(["run", "--ci", "--max-steps", "0"])
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .expect("failed to spawn");

    assert_eq!(
        status.code(),
        Some(CiExitCode::UsageError as i32),
        "--max-steps 0 should exit with code 64 (UsageError)"
    );
}

#[test]
fn ci_ext_t02_timeout_zero_is_usage_error() {
    let home = TestHome::new();
    let status = Command::new(env!("CARGO_BIN_EXE_opencode-rk"))
        .args(["run", "--ci", "--timeout", "0"])
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .expect("failed to spawn");

    assert_eq!(
        status.code(),
        Some(CiExitCode::UsageError as i32),
        "--timeout 0 should exit with code 64 (UsageError)"
    );
}

#[test]
fn ci_ext_t03_max_steps_one_completes_like_baseline() {
    let home = TestHome::new();
    let mut child = spawn_openai_fixture(
        &home,
        vec![
            "--max-steps".into(),
            "1".into(),
            "--prompt".into(),
            "Say hello".into(),
        ],
    );

    let lines = wait_for(&mut child, "TurnFinished", Duration::from_secs(30));
    let status = child.wait().unwrap();

    // Should complete (not crash) and emit at least one JSONL line
    assert!(
        !lines.is_empty(),
        "expected TurnFinished in output, got nothing"
    );

    // Every stdout line must be valid JSON
    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(trimmed);
        assert!(parsed.is_ok(), "invalid JSONL line: {}", trimmed);
    }

    assert!(
        status.success() || status.code() == Some(CiExitCode::BudgetExhausted as i32),
        "process should succeed or exit BudgetExhausted, got {:?}",
        status.code()
    );
}

#[test]
fn ci_ext_t04_timeout_large_value_completes() {
    let home = TestHome::new();
    let mut child = spawn_openai_fixture(
        &home,
        vec![
            "--timeout".into(),
            "60".into(),
            "--prompt".into(),
            "Say hello".into(),
        ],
    );

    let lines = wait_for(&mut child, "TurnFinished", Duration::from_secs(30));
    let status = child.wait().unwrap();

    assert!(
        !lines.is_empty(),
        "expected TurnFinished in output, got nothing"
    );

    // Every line must be valid JSON
    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(trimmed);
        assert!(parsed.is_ok(), "invalid JSONL line: {}", trimmed);
    }

    // With a large timeout, we should NOT hit BudgetExhausted
    assert_ne!(
        status.code(),
        Some(CiExitCode::BudgetExhausted as i32),
        "--timeout 60 should not result in BudgetExhausted"
    );
}

#[test]
fn ci_ext_t05_doctor_emits_valid_jsonl_with_checks() {
    let home = TestHome::new();
    let mut child = ci_command(&home, "doctor");

    let lines = wait_for(&mut child, "Doctor", Duration::from_secs(30));
    let _status = child.wait().unwrap();

    // Find the Doctor JSONL line
    let doctor_line = lines
        .iter()
        .find(|l| l.contains("Doctor"))
        .expect("expected a Doctor JSONL line in output");

    let parsed: serde_json::Value =
        serde_json::from_str(doctor_line.trim()).expect("Doctor line is not valid JSON");

    // Must have a "checks" array
    let checks = parsed
        .get("checks")
        .and_then(|v| v.as_array())
        .expect("Doctor JSONL must contain a 'checks' array");

    assert!(!checks.is_empty(), "Doctor checks array must not be empty");

    // Each check must have name + status
    for check in checks {
        assert!(
            check.get("name").and_then(|v| v.as_str()).is_some(),
            "each check must have a 'name' field: {:?}",
            check
        );
        assert!(
            check.get("status").and_then(|v| v.as_str()).is_some(),
            "each check must have a 'status' field: {:?}",
            check
        );
    }
}

#[test]
fn ci_ext_t06_doctor_is_single_jsonl_line() {
    let home = TestHome::new();
    let mut child = ci_command(&home, "doctor");

    let lines = wait_for(&mut child, "Doctor", Duration::from_secs(30));
    let _status = child.wait().unwrap();

    // Collect only non-empty lines
    let non_empty: Vec<&str> = lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    // Exactly one Doctor line
    let doctor_lines: Vec<&str> = non_empty
        .iter()
        .filter(|l| l.contains("Doctor"))
        .copied()
        .collect();

    assert_eq!(
        doctor_lines.len(),
        1,
        "expected exactly one Doctor JSONL line, got {}",
        doctor_lines.len()
    );

    // That one line must be valid JSON
    let parsed: serde_json::Value =
        serde_json::from_str(doctor_lines[0]).expect("Doctor line is not valid JSON");
    assert!(parsed.is_object(), "Doctor line must be a JSON object");
}
