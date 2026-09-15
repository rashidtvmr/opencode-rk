use serde_json::Value;
use std::{
    fs,
    io::{ErrorKind, Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

static NEXT_TEST_HOME: AtomicU64 = AtomicU64::new(0);

struct TestHome {
    path: PathBuf,
}

impl TestHome {
    fn new(label: &str) -> Self {
        let id = NEXT_TEST_HOME.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "opencode-rk-doctor-{label}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create isolated doctor data directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn doctor_command(home: &TestHome, envs: &[(&str, &str)]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    command
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .args(["doctor", "--json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in envs {
        command.env(key, value);
    }
    command
}

fn run_doctor(home: &TestHome, envs: &[(&str, &str)]) -> Output {
    doctor_command(home, envs)
        .output()
        .expect("run compiled opencode-rk doctor binary")
}

fn parse_successful_json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "doctor should report individual check failures in JSON instead of failing the command; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("doctor --json stdout must be one JSON document")
}

fn combined_output(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn ops_010_t01_doctor_json_returns_structured_auth_connectivity_tools_and_mcp_checks() {
    let home = TestHome::new("shape");
    let output = run_doctor(&home, &[]);
    let json = parse_successful_json(&output);
    let checks = json["checks"]
        .as_object()
        .expect("doctor JSON must expose a checks object");

    for name in ["auth", "connectivity", "tools", "mcp"] {
        let check = checks
            .get(name)
            .unwrap_or_else(|| panic!("doctor JSON is missing {name} check"));
        assert!(
            check["status"].is_string(),
            "{name} check must expose a user-visible status"
        );
    }
}

#[test]
fn ops_010_t02_auth_reports_configured_or_unconfigured_without_echoing_secret() {
    let secret = "sk-ops010-must-never-be-printed";

    let configured_home = TestHome::new("auth-configured");
    let configured = run_doctor(&configured_home, &[("OPENAI_API_KEY", secret)]);
    let configured_json = parse_successful_json(&configured);
    assert_eq!(configured_json["checks"]["auth"]["status"], "configured");
    assert!(
        !combined_output(&configured).contains(secret),
        "doctor output must never echo the configured credential value"
    );

    let unconfigured_home = TestHome::new("auth-unconfigured");
    let unconfigured = run_doctor(&unconfigured_home, &[]);
    let unconfigured_json = parse_successful_json(&unconfigured);
    assert_eq!(
        unconfigured_json["checks"]["auth"]["status"],
        "unconfigured"
    );
}

#[test]
fn ops_010_t03_connectivity_passes_on_loopback_and_fails_cleanly_when_unavailable() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind isolated loopback fixture");
    listener
        .set_nonblocking(true)
        .expect("make loopback fixture bounded");
    let endpoint = format!(
        "http://{}/health",
        listener.local_addr().expect("fixture address")
    );

    let available_home = TestHome::new("connectivity-ok");
    let mut child = doctor_command(
        &available_home,
        &[("OPENCODE_RK_DOCTOR_ENDPOINT", endpoint.as_str())],
    )
    .spawn()
    .expect("spawn doctor against loopback fixture");

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut accepted = false;
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                accepted = true;
                stream
                    .set_read_timeout(Some(Duration::from_millis(100)))
                    .expect("bound fixture read");
                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request);
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                );
                break;
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                if child
                    .try_wait()
                    .expect("check doctor child status")
                    .is_some()
                {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("loopback fixture accept failed: {error}"),
        }
    }
    let available = child
        .wait_with_output()
        .expect("collect doctor loopback output");
    let available_json = parse_successful_json(&available);
    assert!(
        accepted,
        "doctor connectivity check must probe the configured endpoint"
    );
    assert_eq!(available_json["checks"]["connectivity"]["status"], "ok");

    let unavailable_listener =
        TcpListener::bind("127.0.0.1:0").expect("reserve unavailable loopback endpoint");
    let unavailable_addr = unavailable_listener
        .local_addr()
        .expect("unavailable fixture address");
    drop(unavailable_listener);
    let unavailable_endpoint = format!("http://{unavailable_addr}/health");
    let unavailable_home = TestHome::new("connectivity-error");
    let unavailable = run_doctor(
        &unavailable_home,
        &[("OPENCODE_RK_DOCTOR_ENDPOINT", unavailable_endpoint.as_str())],
    );
    let unavailable_json = parse_successful_json(&unavailable);
    assert_eq!(
        unavailable_json["checks"]["connectivity"]["status"],
        "error"
    );
}

#[test]
fn ops_010_t04_tools_check_reports_core_builtins() {
    let home = TestHome::new("tools");
    let output = run_doctor(&home, &[]);
    let json = parse_successful_json(&output);
    let mut builtins = json["checks"]["tools"]["builtins"]
        .as_array()
        .expect("tools check must list built-in tool ids")
        .iter()
        .map(|value| value.as_str().expect("tool id must be a string"))
        .collect::<Vec<_>>();
    builtins.sort_unstable();

    assert_eq!(builtins, ["bash", "edit", "file", "grep", "read", "write"]);
}

#[test]
fn ops_010_t05_mcp_reports_configured_or_unconfigured_without_launching_external_mcp() {
    let configured_home = TestHome::new("mcp-configured");
    let mcp_config = r#"{"servers":{"fixture":{"command":"command-that-must-not-run"}}}"#;
    let configured = run_doctor(&configured_home, &[("OPENCODE_RK_MCP_CONFIG", mcp_config)]);
    let configured_json = parse_successful_json(&configured);
    assert_eq!(configured_json["checks"]["mcp"]["status"], "configured");

    let unconfigured_home = TestHome::new("mcp-unconfigured");
    let unconfigured = run_doctor(&unconfigured_home, &[]);
    let unconfigured_json = parse_successful_json(&unconfigured);
    assert_eq!(unconfigured_json["checks"]["mcp"]["status"], "unconfigured");
}
