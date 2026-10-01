#![forbid(unsafe_code)]
//! LANE-CI-FLAG frozen tests: CI mode end-to-end.
//!
//! Scenario A: valid JSONL ending with TurnFinished and exit 0.
//! Scenario B: approval fails closed with exit 20, never auto-approved.
//! Scenario C: identical runs produce identical JSONL after removing timestamps.

#[path = "../src/ci_output.rs"]
mod ci_output;
use ci_output::{CiEvent, CiExitCode, OutputFormat};
use std::{
    cell::RefCell,
    fs::{self, File},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
static NEXT_ID: AtomicU64 = AtomicU64::new(0);
const DEADLINE: Duration = Duration::from_secs(30);
const MAX: usize = 128 * 1024;
const MODEL: &str = "openai/gpt-5.6";
struct TestHome {
    dir: PathBuf,
    fixture: RefCell<Option<FixtureResources>>,
}
impl TestHome {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("opencode-rk-ci-{}-{id}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        Self {
            dir,
            fixture: RefCell::new(None),
        }
    }
    fn path(&self) -> &Path {
        &self.dir
    }
}
impl Drop for TestHome {
    fn drop(&mut self) {
        drop(self.fixture.get_mut().take());
        let _ = fs::remove_dir_all(&self.dir);
    }
}
struct FixtureResources {
    daemon: Option<Child>,
}
impl Drop for FixtureResources {
    fn drop(&mut self) {
        if let Some(mut child) = self.daemon.take() {
            report_cleanup(stop_child(&mut child), "fixture daemon");
        }
    }
}
fn report_cleanup(result: std::io::Result<()>, resource: &str) {
    if let Err(error) = result {
        if thread::panicking() {
            eprintln!("CI-mode {resource} failure cleanup: {error}");
        } else {
            panic!("CI-mode {resource} cleanup failed: {error}");
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
struct ChildGuard {
    child: Arc<Mutex<Child>>,
}
impl ChildGuard {
    fn wait(&mut self) -> std::io::Result<ExitStatus> {
        let deadline = Instant::now() + DEADLINE;
        loop {
            if let Some(status) = self.child.lock().unwrap().try_wait()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                stop_child(&mut self.child.lock().unwrap())?;
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "CI-mode child exit deadline",
                ));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        report_cleanup(stop_child(&mut self.child.lock().unwrap()), "CI child");
    }
}
fn free_loopback_addr() -> String {
    "127.0.0.1:0".into()
}
struct StdoutReader {
    buffer: Arc<Mutex<Vec<u8>>>,
    child: Arc<Mutex<Child>>,
    overflow: Arc<AtomicBool>,
    read_failed: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}
impl StdoutReader {
    fn spawn(mut pipe: impl Read + Send + 'static, child: Arc<Mutex<Child>>) -> Self {
        let b = Arc::new(Mutex::new(Vec::new()));
        let s = Arc::clone(&b);
        let overflow = Arc::new(AtomicBool::new(false));
        let read_failed = Arc::new(AtomicBool::new(false));
        let worker_overflow = Arc::clone(&overflow);
        let worker_failed = Arc::clone(&read_failed);
        let h = thread::spawn(move || {
            let mut x = [0; 4096];
            loop {
                match pipe.read(&mut x) {
                    Ok(0) => break,
                    Err(_) => {
                        worker_failed.store(true, Ordering::Release);
                        break;
                    }
                    Ok(n) => {
                        let mut o = s.lock().unwrap();
                        let k = n.min(MAX.saturating_sub(o.len()));
                        o.extend_from_slice(&x[..k]);
                        worker_overflow.fetch_or(k < n, Ordering::AcqRel);
                    }
                }
            }
        });
        Self {
            buffer: b,
            child,
            overflow,
            read_failed,
            handle: Some(h),
        }
    }
    fn text(&self) -> String {
        String::from_utf8(self.buffer.lock().unwrap().clone()).expect("CI stdout UTF-8")
    }
    fn wait_for(&self, n: &str, t: Duration) -> String {
        let d = Instant::now() + t;
        loop {
            let exited = self
                .child
                .lock()
                .unwrap()
                .try_wait()
                .expect("poll CI child");
            if exited.is_some() && self.handle.as_ref().unwrap().is_finished() {
                assert!(
                    !self.overflow.load(Ordering::Acquire),
                    "CI output exceeded bound"
                );
                assert!(
                    !self.read_failed.load(Ordering::Acquire),
                    "CI stdout read failed"
                );
                let text = self.text();
                assert!(
                    text.contains(n),
                    "expected {n:?} in complete CI stdout: {text}"
                );
                return text;
            }
            if Instant::now() >= d {
                stop_child(&mut self.child.lock().unwrap()).expect("reclaim timed-out CI child");
                panic!(
                    "timed out waiting for {n:?}; bounded stdout: {}",
                    self.text()
                );
            }
            thread::sleep(Duration::from_millis(25));
        }
    }
}
impl Drop for StdoutReader {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            let child_result = stop_child(&mut self.child.lock().unwrap());
            let deadline = Instant::now() + Duration::from_secs(2);
            while !h.is_finished() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            let reader_result = if !h.is_finished() {
                Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "CI stdout join deadline",
                ))
            } else if h.join().is_err() || self.read_failed.load(Ordering::Acquire) {
                Err(std::io::Error::other("CI stdout reader failed"))
            } else if self.overflow.load(Ordering::Acquire) {
                Err(std::io::Error::other("CI output exceeded bound"))
            } else {
                Ok(())
            };
            report_cleanup(child_result.and(reader_result), "stdout reader");
        }
    }
}
struct ProviderTask {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}
impl ProviderTask {
    fn finish(&mut self) -> std::io::Result<()> {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let deadline = Instant::now() + Duration::from_secs(3);
            while !handle.is_finished() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            if !handle.is_finished() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "provider join deadline",
                ));
            }
            handle
                .join()
                .map_err(|_| std::io::Error::other("provider fixture panicked"))?;
        }
        Ok(())
    }
    fn join(mut self) -> Result<(), ()> {
        self.finish()
            .expect("CI-mode provider must finish without protocol or cleanup failures");
        Ok(())
    }
}
impl Drop for ProviderTask {
    fn drop(&mut self) {
        let result = self.finish();
        report_cleanup(result, "provider");
    }
}
fn request(s: &mut TcpStream) -> Vec<u8> {
    let mut b = Vec::new();
    let d = Instant::now() + Duration::from_secs(3);
    loop {
        assert!(Instant::now() < d, "provider request deadline");
        let mut x = [0; 4096];
        let n = s.read(&mut x).expect("provider request read");
        assert!(n > 0, "provider request ended early");
        assert!(b.len() <= MAX.saturating_sub(n), "provider request bound");
        b.extend_from_slice(&x[..n]);
        if let Some(e) = b.windows(4).position(|w| w == b"\r\n\r\n") {
            let h = String::from_utf8_lossy(&b[..e]);
            let l = h
                .lines()
                .find_map(|v| {
                    v.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|x| x.trim().parse().ok())
                })
                .unwrap_or(0);
            assert!(l <= MAX - e - 4, "provider body bound");
            if b.len() >= e + 4 + l {
                return b;
            }
        }
    }
}
fn spawn_openai_fixture() -> (String, ProviderTask) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let a = l.local_addr().unwrap();
    l.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let st = Arc::clone(&stop);
    let h = thread::spawn(move || {
        let d = Instant::now() + DEADLINE;
        let mut got = false;
        while !st.load(Ordering::Acquire) && Instant::now() < d {
            let (mut s, _) = match l.accept() {
                Ok(x) => x,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => panic!("provider accept: {e}"),
            };
            s.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
            s.set_write_timeout(Some(Duration::from_secs(1))).unwrap();
            let b = request(&mut s);
            let e = b.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
            let t = String::from_utf8_lossy(&b);
            let mut q = t.lines().next().unwrap().split_whitespace();
            assert_eq!(
                (q.next(), q.next(), q.next()),
                (Some("POST"), Some("/v1/responses"), Some("HTTP/1.1"))
            );
            assert!(
                t[..e].lines().any(|line| {
                    line.split_once(':').is_some_and(|(name, value)| {
                        name.eq_ignore_ascii_case("authorization")
                            && value.trim() == "Bearer fixture-secret"
                    })
                }),
                "provider bearer mismatch"
            );
            let j: serde_json::Value = serde_json::from_slice(&b[e + 4..]).unwrap();
            assert_eq!(j["model"].as_str(), Some("gpt-5.6"));
            assert!(matches!(
                j.get("stream"),
                None | Some(serde_json::Value::Bool(false))
            ));
            assert!(
                j["input"].as_array().is_some_and(|items| {
                    items.last().is_some_and(|item| {
                        item["role"] == "user"
                            && matches!(
                                item["content"].as_str(),
                                Some("say hi" | "determinism probe")
                            )
                    })
                }),
                "provider prompt mismatch"
            );
            assert!(!got, "provider received more than one request");
            let body=serde_json::json!({"id":"resp_fixture","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"CI fixture response"}]}]}).to_string();
            let w=format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",body.len(),body);
            s.write_all(w.as_bytes()).unwrap();
            s.flush().unwrap();
            got = true
        }
        assert!(got, "provider did not receive request")
    });
    (
        format!("http://{a}/v1"),
        ProviderTask {
            stop,
            handle: Some(h),
        },
    )
}

fn start_daemon(home: &TestHome, extra: &[(&str, &str)]) -> (String, String) {
    assert!(
        home.fixture.borrow().is_none(),
        "one daemon per CI-mode home"
    );
    let m = home.path().join("models.json");
    fs::write(&m,r#"{"openai":{"name":"OpenAI","models":{"gpt-5.6":{"name":"GPT-5.6","tool_call":true,"reasoning":true,"limit":{"context":200000}}}}}"#).unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    c.args(["serve", "--listen", "127.0.0.1:0", "--models-file"])
        .arg(&m)
        .current_dir(home.path())
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join("xdg-config"))
        .env("XDG_DATA_HOME", home.path().join("xdg-data"))
        .env("XDG_STATE_HOME", home.path().join("xdg-state"))
        .env("XDG_CACHE_HOME", home.path().join("xdg-cache"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for &(key, value) in extra {
        assert!(matches!(key, "OPENAI_BASE_URL" | "OPENAI_API_KEY"));
        c.env(key, value);
    }
    let c = c.spawn().unwrap();
    let pid = c.id();
    home.fixture
        .borrow_mut()
        .replace(FixtureResources { daemon: Some(c) });
    let p = home.path().join("runtime/backend.json");
    let d = Instant::now() + DEADLINE;
    loop {
        let alive = home
            .fixture
            .borrow_mut()
            .as_mut()
            .unwrap()
            .daemon
            .as_mut()
            .unwrap()
            .try_wait()
            .expect("poll fixture daemon")
            .is_none();
        assert!(alive, "fixture daemon exited before readiness");
        if let Ok(file) = File::open(&p) {
            let mut b = Vec::new();
            file.take(8193)
                .read_to_end(&mut b)
                .expect("bounded daemon descriptor read");
            assert!(b.len() <= 8192);
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&b) {
                let o = v["http_origin"].as_str();
                let tok = v["auth_token"].as_str();
                if v["pid"].as_u64() == Some(u64::from(pid))
                    && o.is_some_and(|x| {
                        x.strip_prefix("http://127.0.0.1:")
                            .is_some_and(|port| port.parse::<u16>().is_ok_and(|port| port != 0))
                    })
                    && tok
                        .is_some_and(|x| x.len() == 64 && x.bytes().all(|z| z.is_ascii_hexdigit()))
                {
                    let o = o.unwrap().to_owned();
                    let tok = tok.unwrap().to_owned();
                    let host = o.strip_prefix("http://").unwrap();
                    let mut s =
                        TcpStream::connect_timeout(&host.parse().unwrap(), Duration::from_secs(1))
                            .unwrap();
                    s.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
                    s.set_write_timeout(Some(Duration::from_secs(1))).unwrap();
                    write!(s,"GET /api/models?provider=openai&limit=500 HTTP/1.1\r\nhost: {host}\r\nauthorization: Bearer {tok}\r\nconnection: close\r\n\r\n").unwrap();
                    let mut r = Vec::new();
                    s.take((MAX + 1) as u64).read_to_end(&mut r).unwrap();
                    assert!(
                        r.len() <= MAX && String::from_utf8_lossy(&r).starts_with("HTTP/1.1 200")
                    );
                    return (o, tok);
                }
            }
        }
        assert!(Instant::now() < d, "daemon descriptor readiness timeout");
        thread::sleep(Duration::from_millis(20))
    }
}
fn ci_command(home: &TestHome, _: &str, extra: &[(&str, &str)]) -> Command {
    let (origin, token) = start_daemon(home, extra);
    let address = origin
        .strip_prefix("http://")
        .expect("loopback daemon origin");
    let mut c = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    c.env_clear()
        .current_dir(home.path())
        .env("OPENCODE_RK_HOME", home.path())
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join("xdg-config"))
        .env("XDG_DATA_HOME", home.path().join("xdg-data"))
        .env("XDG_STATE_HOME", home.path().join("xdg-state"))
        .env("XDG_CACHE_HOME", home.path().join("xdg-cache"))
        .env("OPENCODE_RK_DAEMON_ADDR", address)
        .env("OPENCODE_RK_DAEMON_TOKEN", token)
        .env("OPENCODE_RK_CI_MODEL", MODEL)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for &(key, value) in extra {
        assert!(matches!(key, "OPENAI_BASE_URL" | "OPENAI_API_KEY"));
        c.env(key, value);
    }
    c
}
fn spawn_guarded(mut c: Command) -> (ChildGuard, StdoutReader) {
    let mut x = c.spawn().unwrap();
    let out = x.stdout.take().unwrap();
    let a = Arc::new(Mutex::new(x));
    let r = StdoutReader::spawn(out, Arc::clone(&a));
    (ChildGuard { child: a.clone() }, r)
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
                let mut parsed: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
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
