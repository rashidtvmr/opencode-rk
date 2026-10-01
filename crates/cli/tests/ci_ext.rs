#![forbid(unsafe_code)]

#[path = "../src/ci_output.rs"]
mod ci_output;

use ci_output::CiExitCode;
use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
const FIXTURE_MODEL: &str = "openai/gpt-5.6";
const MAX_FIXTURE_BYTES: usize = 256 * 1024;

struct FixtureResources {
    stopping: Arc<AtomicBool>,
    provider: Option<JoinHandle<()>>,
    daemon: Option<Child>,
}

impl Drop for FixtureResources {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        let mut errors = Vec::with_capacity(2);
        if let Some(mut daemon) = self.daemon.take() {
            if stop_child(&mut daemon).is_err() {
                errors.push("fixture daemon cleanup failed");
            }
        }
        if let Some(provider) = self.provider.take() {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !provider.is_finished() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            if !provider.is_finished() {
                errors.push("provider fixture did not stop before deadline");
            } else if provider.join().is_err() {
                errors.push("provider fixture panicked");
            }
        }
        if !errors.is_empty() {
            if thread::panicking() {
                eprintln!("CI fixture failure cleanup: {errors:?}");
            } else {
                panic!("CI fixture cleanup failed: {errors:?}");
            }
        }
    }
}

fn stop_child(child: &mut Child) -> std::io::Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    child.kill()?;
    let deadline = Instant::now() + Duration::from_secs(2);
    while child.try_wait()?.is_none() {
        if Instant::now() >= deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "owned child cleanup deadline",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

struct CiChildOwner<'a> {
    child: &'a mut Child,
}

impl Drop for CiChildOwner<'_> {
    fn drop(&mut self) {
        if let Err(error) = stop_child(self.child) {
            if thread::panicking() {
                eprintln!("CI child failure cleanup: {error}");
            } else {
                panic!("CI child cleanup failed: {error}");
            }
        }
    }
}

fn drain_capped(mut pipe: impl Read) -> std::io::Result<(Vec<u8>, bool)> {
    let mut kept = Vec::with_capacity(4096);
    let mut overflow = false;
    let mut buffer = [0_u8; 4096];
    loop {
        let read = pipe.read(&mut buffer)?;
        if read == 0 {
            return Ok((kept, overflow));
        }
        let retained = read.min(MAX_FIXTURE_BYTES.saturating_sub(kept.len()));
        kept.extend_from_slice(&buffer[..retained]);
        overflow |= retained < read;
    }
}

fn wait_for(child: &mut Child, pattern: &str, timeout: Duration) -> Vec<String> {
    let owner = CiChildOwner { child };
    let stdout = owner.child.stdout.take().expect("piped stdout");
    let stderr = owner.child.stderr.take().expect("piped stderr");
    let stdout_reader = thread::spawn(move || drain_capped(stdout));
    let stderr_reader = thread::spawn(move || drain_capped(stderr));
    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    while owner.child.try_wait().expect("poll CI child").is_none() {
        if Instant::now() >= deadline {
            timed_out = true;
            stop_child(owner.child).expect("reclaim timed-out CI child");
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let drain_deadline = Instant::now() + Duration::from_secs(2);
    while !stdout_reader.is_finished() || !stderr_reader.is_finished() {
        assert!(Instant::now() < drain_deadline, "CI pipe drain deadline");
        thread::sleep(Duration::from_millis(10));
    }
    let (stdout, stdout_overflow) = stdout_reader
        .join()
        .expect("CI stdout reader panicked")
        .expect("CI stdout read failed");
    let (stderr, stderr_overflow) = stderr_reader
        .join()
        .expect("CI stderr reader panicked")
        .expect("CI stderr read failed");
    assert!(
        !stdout_overflow && !stderr_overflow,
        "CI output exceeded bound"
    );
    let stdout = String::from_utf8(stdout).expect("CI stdout UTF-8");
    assert!(
        !timed_out,
        "CI exit deadline; stderr: {}",
        String::from_utf8_lossy(&stderr)
    );
    let collected: Vec<String> = stdout.lines().take(1025).map(str::to_owned).collect();
    assert!(collected.len() <= 1024, "CI stdout line-count bound");
    assert!(
        collected.iter().any(|line| line.contains(pattern)),
        "expected {pattern:?} in bounded CI output: {collected:?}"
    );
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
        let mut models_requests = 0usize;
        let mut response_requests = 0usize;
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
                .set_read_timeout(Some(Duration::from_secs(1)))
                .expect("provider read timeout");
            stream
                .set_write_timeout(Some(Duration::from_secs(1)))
                .expect("provider write timeout");
            let mut request = Vec::new();
            let mut buf = [0_u8; 4096];
            let request_deadline = Instant::now() + Duration::from_secs(2);
            loop {
                assert!(
                    Instant::now() < request_deadline,
                    "provider request deadline"
                );
                let read = stream.read(&mut buf).expect("provider request read");
                if read == 0 {
                    break;
                }
                assert!(
                    request.len() <= MAX_FIXTURE_BYTES.saturating_sub(read),
                    "provider request exceeded bound"
                );
                request.extend_from_slice(&buf[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let headers_end = request
                        .windows(4)
                        .position(|window| window == b"\r\n\r\n")
                        .unwrap();
                    let headers = String::from_utf8_lossy(&request[..headers_end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|value| value.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    assert!(
                        length <= MAX_FIXTURE_BYTES - headers_end - 4,
                        "provider content-length exceeded bound"
                    );
                    if request.len() >= headers_end + 4 + length {
                        break;
                    }
                }
            }
            let request_text = String::from_utf8_lossy(&request);
            let request_line = request_text.lines().next().unwrap_or("");
            let mut fields = request_line.split_whitespace();
            let method = fields.next().expect("provider method");
            let path = fields.next().expect("provider path");
            assert_eq!(fields.next(), Some("HTTP/1.1"));
            assert!(fields.next().is_none(), "provider request-line fields");
            let header_end = request
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .expect("provider headers");
            let headers = request_text[..header_end].to_ascii_lowercase();
            assert!(
                headers.contains("authorization: bearer fixture-key"),
                "provider bearer mismatch"
            );
            let (content_type, response_body) = if method == "GET" && path == "/v1/models" {
                models_requests += 1;
                assert_eq!(models_requests, 1, "provider /v1/models request count");
                (
                    "application/json",
                    serde_json::json!({
                        "object": "list",
                        "data": [{"id": "gpt-5.6", "object": "model", "owned_by": "openai"}]
                    })
                    .to_string(),
                )
            } else if method == "POST" && path == "/v1/responses" {
                response_requests += 1;
                assert_eq!(response_requests, 1, "provider /v1/responses request count");
                let body = &request[header_end + 4..];
                let json: serde_json::Value =
                    serde_json::from_slice(body).expect("provider JSON body");
                assert_eq!(
                    json.get("model").and_then(|value| value.as_str()),
                    Some("gpt-5.6")
                );
                assert_eq!(json.get("stream"), Some(&serde_json::json!(false)));
                assert!(
                    json.to_string().contains("Say hello"),
                    "provider prompt mismatch"
                );
                (
                    "application/json",
                    serde_json::json!({
                        "id": "fixture-response",
                        "status": "completed",
                        "output": [{
                            "type": "message",
                            "role": "assistant",
                            "content": [{"type": "output_text", "text": "fixture hello"}]
                        }]
                    })
                    .to_string(),
                )
            } else {
                panic!("unexpected provider route: {method} {path}");
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{response_body}",
                response_body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("provider response write");
        }
        assert!(models_requests <= 1, "provider model request bound");
        assert_eq!(
            response_requests, 1,
            "provider did not receive exactly one /v1/responses request"
        );
    });

    home.fixture.borrow_mut().replace(FixtureResources {
        stopping,
        provider: Some(provider),
        daemon: None,
    });
    let models_file = home.path().join("models.json");
    std::fs::write(&models_file, r#"{"openai":{"name":"OpenAI","models":{"gpt-5.6":{"name":"GPT-5.6","tool_call":true,"reasoning":true,"limit":{"context":200000}}}}}"#).expect("write models fixture");

    let mut daemon = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    daemon
        .args(["serve", "--listen", "127.0.0.1:0", "--models-file"])
        .arg(&models_file)
        .current_dir(home.path())
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join("xdg-config"))
        .env("XDG_DATA_HOME", home.path().join("xdg-data"))
        .env("XDG_CACHE_HOME", home.path().join("xdg-cache"))
        .env(
            "OPENAI_BASE_URL",
            format!("http://{}:{}/v1", provider_addr.ip(), provider_addr.port()),
        )
        .env("OPENAI_API_KEY", "fixture-key")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    let daemon = daemon.spawn().expect("failed to spawn fixture daemon");
    let daemon_id = daemon.id();
    home.fixture
        .borrow_mut()
        .as_mut()
        .expect("fixture resources")
        .daemon = Some(daemon);
    let descriptor_path = home.path().join("runtime/backend.json");
    let deadline = Instant::now() + FIXTURE_DEADLINE;
    let (origin, token) = loop {
        let descriptor = match File::open(&descriptor_path) {
            Ok(file) => Some(file),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => panic!("daemon descriptor open failed: {error}"),
        };
        if let Some(file) = descriptor {
            let mut bytes = Vec::new();
            file.take(8 * 1024 + 1)
                .read_to_end(&mut bytes)
                .expect("read descriptor");
            assert!(bytes.len() <= 8 * 1024, "daemon descriptor exceeded bound");
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                let pid = value.get("pid").and_then(|v| v.as_u64());
                let origin = value.get("http_origin").and_then(|v| v.as_str());
                let token = value.get("auth_token").and_then(|v| v.as_str());
                let valid_origin = origin.is_some_and(|value| {
                    value
                        .strip_prefix("http://127.0.0.1:")
                        .is_some_and(|port| port.parse::<u16>().is_ok_and(|port| port != 0))
                });
                let alive = home
                    .fixture
                    .borrow_mut()
                    .as_mut()
                    .expect("fixture resources")
                    .daemon
                    .as_mut()
                    .expect("fixture daemon")
                    .try_wait()
                    .expect("poll fixture daemon")
                    .is_none();
                if pid == Some(u64::from(daemon_id))
                    && valid_origin
                    && alive
                    && token.is_some_and(|value| {
                        value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
                    })
                {
                    let origin = origin.unwrap().to_owned();
                    let token = token.unwrap().to_owned();
                    let host = origin.strip_prefix("http://").expect("loopback origin");
                    let mut stream = TcpStream::connect_timeout(
                        &host.parse().expect("numeric loopback readiness address"),
                        Duration::from_secs(1),
                    )
                    .expect("connect daemon readiness");
                    stream
                        .set_read_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    stream
                        .set_write_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    let wire = format!(
                        "GET /api/models?provider=openai&limit=500 HTTP/1.1\r\nhost: {host}\r\nauthorization: Bearer {token}\r\nconnection: close\r\n\r\n"
                    );
                    stream
                        .write_all(wire.as_bytes())
                        .expect("daemon readiness write");
                    let mut response = Vec::new();
                    stream
                        .take((MAX_FIXTURE_BYTES + 1) as u64)
                        .read_to_end(&mut response)
                        .expect("daemon readiness read");
                    assert!(
                        response.len() <= MAX_FIXTURE_BYTES,
                        "daemon readiness response exceeded bound"
                    );
                    assert!(
                        String::from_utf8_lossy(&response).starts_with("HTTP/1.1 200"),
                        "daemon readiness failed"
                    );
                    break (origin, token);
                }
            }
        }
        assert!(
            Instant::now() < deadline,
            "fixture daemon descriptor readiness timeout"
        );
        thread::sleep(Duration::from_millis(20));
    };
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
        .env(
            "OPENCODE_RK_DAEMON_ADDR",
            origin.strip_prefix("http://").unwrap_or(&origin),
        )
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
