#![forbid(unsafe_code)]

#[path = "../src/ci_output.rs"]
mod ci_output;

use ci_output::CiExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------------------
// Helpers (same pattern as ci_mode.rs tests)
// ---------------------------------------------------------------------------

struct TestHome {
    dir: std::path::PathBuf,
}

impl TestHome {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "opencode-rk-ci-ext-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }
    fn path(&self) -> &std::path::Path {
        &self.dir
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
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

struct StdoutReader {
    lines: Vec<String>,
}

impl StdoutReader {
    fn read_until_match(pattern: &str, timeout: Duration) -> Vec<String> {
        // This reads from stdin (already piped parent-side) — not used directly;
        // real matching happens in wait_for.
        Vec::new()
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
    let addr = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap()
    };
    // Write a minimal OpenAI-compatible fixture config.
    let config = format!(
        r#"{{
            "daemon": {{
                "address": "{}"
            }},
            "providers": {{
                "openai": {{
                    "base_url": "http://{}:{}",
                    "api_key": "sk-fake"
                }}
            }}
        }}"#,
        addr,
        addr.ip(),
        addr.port()
    );
    std::fs::write(home.path().join("config.json"), config).unwrap();

    let mut args = vec![
        "run".to_string(),
        "--ci".to_string(),
    ];
    args.extend(extra_args);

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    cmd.args(&args)
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .env("OPENCODE_RK_DAEMON_ADDR", addr.to_string())
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
    let doctor_line = lines.iter().find(|l| l.contains("Doctor")).expect(
        "expected a Doctor JSONL line in output",
    );

    let parsed: serde_json::Value =
        serde_json::from_str(doctor_line.trim()).expect("Doctor line is not valid JSON");

    // Must have a "checks" array
    let checks = parsed
        .get("checks")
        .and_then(|v| v.as_array())
        .expect("Doctor JSONL must contain a 'checks' array");

    assert!(
        !checks.is_empty(),
        "Doctor checks array must not be empty"
    );

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
    assert!(
        parsed.is_object(),
        "Doctor line must be a JSON object"
    );
}
