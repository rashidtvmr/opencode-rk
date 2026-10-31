#![forbid(unsafe_code)]
//! Native launch integration:
//!
//! - a redirected default launch is refused headlessly (frozen `app_start`
//!   contract: no raw mode, no daemon). `app_start::decide_launch_mode:116-124`
//!   routes any redirected stdio to `Headless`, `plan_default_launch:295-321`
//!   attaches no role/view, and the call site exits
//!   `HEADLESS_EXIT_CODE=2` with the documented message. The `--native`
//!   spelling of the same refusal is covered by `native_daemon_flow`;
//! - the top-level `--once` flag is the scriptable `tui --once` alias
//!   (`main.rs`), so redirected stdio must render exactly one bounded frame,
//!   exit 0, and start no daemon;
//! - the scriptable path still fails closed when it cannot reach its origin.

use std::{
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

/// Bound on any launch under test.
const DEADLINE: Duration = Duration::from_secs(30);

static NEXT_ID: AtomicU64 = AtomicU64::new(1000);

struct TestHome(PathBuf);

impl TestHome {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("opencode-rk-native-{}", id));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
    /// The daemon's own descriptor location (`DaemonPaths::for_data_dir`).
    fn descriptor(&self) -> PathBuf {
        self.0.join("runtime/backend.json")
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn free_loopback_addr() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    format!("127.0.0.1:{}", listener.local_addr().unwrap().port())
}

fn launch_command(home: &TestHome, daemon_addr: &str, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    command
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .env("OPENCODE_RK_DAEMON_ADDR", daemon_addr)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn bounded_output(mut command: Command) -> (ExitStatus, String, String, Duration) {
    let start = Instant::now();
    let output = command.output().expect("spawn binary");
    let elapsed = start.elapsed();
    (
        output.status,
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        elapsed,
    )
}

// ---------------------------------------------------------------------------
// Scenario A: redirected default launch is refused before raw mode or any
// daemon work.
// ---------------------------------------------------------------------------
#[test]
fn default_launch_redirected_stdio_refuses_raw_mode() {
    let home = TestHome::new();
    let daemon_addr = free_loopback_addr();

    let (status, stdout, stderr, elapsed) = bounded_output(launch_command(&home, &daemon_addr, &[]));

    assert!(
        elapsed < DEADLINE,
        "a refused redirected launch must not hang: {elapsed:?}"
    );
    assert_eq!(
        status.code(),
        Some(2),
        "redirected default launch must exit 2; stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(
        stderr.contains("raw mode is refused"),
        "the headless message must name the raw-mode refusal; stderr:\n{stderr}"
    );
    assert!(
        !home.descriptor().exists(),
        "a refused redirected launch must not start a daemon or publish {:?}",
        home.descriptor()
    );
}

// ---------------------------------------------------------------------------
// Scenario B: the `--once` alias is scriptable — one bounded frame on
// redirected stdio, no TTY, no daemon.
// ---------------------------------------------------------------------------
#[test]
fn native_once_alias_renders_one_bounded_frame_on_redirected_stdio() {
    let home = TestHome::new();
    let daemon_addr = free_loopback_addr();

    let (status, stdout, stderr, elapsed) =
        bounded_output(launch_command(&home, &daemon_addr, &["--native", "--once"]));

    assert!(
        elapsed < DEADLINE,
        "--native --once must exit within the bound: {elapsed:?}"
    );
    assert!(
        status.success(),
        "--native --once must render and exit 0 without a TTY; stderr:\n{stderr}"
    );
    assert!(
        !stdout.trim().is_empty(),
        "--native --once must print one frame to redirected stdout"
    );
    assert!(
        !stderr.contains("raw mode is refused"),
        "the scriptable alias must not take the headless refusal; stderr:\n{stderr}"
    );
    assert!(
        !home.descriptor().exists(),
        "--once must not start a daemon or publish {:?}",
        home.descriptor()
    );
}

// ---------------------------------------------------------------------------
// Scenario C: the scriptable path still fails closed when its origin is
// unreachable — never a fabricated frame.
// ---------------------------------------------------------------------------
#[test]
fn tui_once_fails_closed_when_the_origin_is_unreachable() {
    let home = TestHome::new();
    let daemon_addr = free_loopback_addr();
    let origin = format!("http://{daemon_addr}");

    let (status, stdout, stderr, elapsed) = bounded_output(launch_command(
        &home,
        &daemon_addr,
        &["tui", "--once", "--origin", &origin],
    ));

    assert!(
        elapsed < DEADLINE,
        "a dead origin must fail bounded: {elapsed:?}"
    );
    assert!(
        !status.success(),
        "an unreachable origin must fail --once; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("error:"),
        "the failure must surface as a typed error; stderr:\n{stderr}"
    );
}
