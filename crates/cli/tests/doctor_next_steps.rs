#![forbid(unsafe_code)]
//! Doctor actionability lane: every unconfigured check must tell the user the
//! exact next step (env var or command) instead of a bare "unconfigured".
//! Additive `next_step` field on the doctor JSON; configured checks stay
//! hint-free. Secrets must never appear in output (re-asserted here).

use serde_json::Value;
use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

static NEXT_HOME: AtomicU64 = AtomicU64::new(0);

struct TestHome {
    path: PathBuf,
}

impl TestHome {
    fn new(label: &str) -> Self {
        let id = NEXT_HOME.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "opencode-rk-doctor-next-{label}-{}-{id}",
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

fn run_doctor(home: &TestHome, envs: &[(&str, &str)]) -> Output {
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
        .output()
        .expect("run compiled opencode-rk doctor binary")
}

fn parse_successful_json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "doctor must succeed while reporting degraded checks; stderr={}",
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
fn doctor_unconfigured_auth_names_the_exact_env_var_to_set() {
    let home = TestHome::new("auth-hint");
    let output = run_doctor(&home, &[]);
    let json = parse_successful_json(&output);
    assert_eq!(json["checks"]["auth"]["status"], "unconfigured");
    let next_step = json["checks"]["auth"]["next_step"]
        .as_str()
        .expect("unconfigured auth must carry a next_step hint");
    assert!(
        next_step.contains("OPENAI_API_KEY") || next_step.contains("ANTHROPIC_API_KEY"),
        "auth hint must name a concrete env var: {next_step}"
    );
}

#[test]
fn doctor_configured_auth_has_no_hint_and_never_echoes_the_secret() {
    let secret = "sk-doctor-hint-must-not-leak";
    let home = TestHome::new("auth-ok");
    let output = run_doctor(&home, &[("OPENAI_API_KEY", secret)]);
    let json = parse_successful_json(&output);
    assert_eq!(json["checks"]["auth"]["status"], "configured");
    assert!(
        json["checks"]["auth"].get("next_step").is_none()
            || json["checks"]["auth"]["next_step"].is_null(),
        "configured auth must not carry a remediation hint"
    );
    assert!(
        !combined_output(&output).contains(secret),
        "doctor output must never echo the credential value"
    );
}

#[test]
fn doctor_unconfigured_connectivity_and_mcp_hint_next_steps() {
    let home = TestHome::new("conn-mcp-hints");
    let output = run_doctor(&home, &[]);
    let json = parse_successful_json(&output);

    let connectivity_step = json["checks"]["connectivity"]["next_step"]
        .as_str()
        .expect("unconfigured connectivity must carry a next_step hint");
    assert!(
        connectivity_step.contains("OPENCODE_RK_DOCTOR_ENDPOINT"),
        "connectivity hint must name its env var: {connectivity_step}"
    );

    let mcp_step = json["checks"]["mcp"]["next_step"]
        .as_str()
        .expect("unconfigured mcp must carry a next_step hint");
    assert!(
        mcp_step.contains("OPENCODE_RK_MCP_CONFIG"),
        "mcp hint must name its env var: {mcp_step}"
    );
}

#[test]
fn doctor_real_daemon_health_reports_ok_without_hint() {
    // Probe a real daemon's /health (the endpoint users actually point at).
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve daemon port");
    let address = listener.local_addr().expect("daemon address");
    drop(listener); // release the port for the daemon to bind

    let home = TestHome::new("daemon-health");
    let daemon = Command::new(env!("CARGO_BIN_EXE_opencode-rk"))
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .args([
            "web",
            "--no-open",
            "--listen",
            &format!("127.0.0.1:{}", address.port()),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn daemon for health probe");
    let _daemon = ChildGuard(daemon);
    let endpoint = format!("http://127.0.0.1:{}/health", address.port());

    // Wait for daemon readiness before probing.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if std::net::TcpStream::connect_timeout(
            &address,
            std::time::Duration::from_millis(250),
        )
        .is_ok()
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "daemon did not start for health probe"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    let output = run_doctor(&home, &[("OPENCODE_RK_DOCTOR_ENDPOINT", endpoint.as_str())]);
    let json = parse_successful_json(&output);
    assert_eq!(json["checks"]["connectivity"]["status"], "ok");
    assert!(
        json["checks"]["connectivity"].get("next_step").is_none()
            || json["checks"]["connectivity"]["next_step"].is_null(),
        "passing checks must not carry remediation hints"
    );
}
