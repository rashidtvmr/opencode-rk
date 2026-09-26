//! APP-012 installed application RED journey, scoped to the FIRST observable
//! setup failure.
//!
//! SCOPE (honest, deliberately narrow). This RED proves exactly one thing about
//! the installed `oc2` executable: launched with a fresh HOME, no provider
//! credentials and a real terminal, it must surface the documented in-app
//! setup state instead of silently entering the compatibility chat. At this
//! base it does not, so this test fails on that single assertion.
//!
//! Source evidence for the expected behavior:
//! - `crates/cli/src/app_start.rs:337-345`: `needs_setup` and `setup_message`
//!   define the in-app setup condition and its stable text.
//! - `crates/cli/src/main.rs:228-258`: the no-subcommand `NativeTui` arm calls
//!   `chat::prepare_daemon` then `chat::run`; `plan.view` / `needs_setup` are
//!   never consulted, so missing credentials fall through to the chat UI.
//!
//! UNPROVEN BY THIS FILE (documented, NOT claimed):
//! - provider-backed coding turn and streamed assistant output;
//! - approval flow executing a real fixture file edit through the broker;
//! - two clients observing one session/permit without duplicate execution;
//! - restart/resume persistence transitions and durable history;
//! - native OpenTUI rendering (the vendored macOS native artifact is absent at
//!   this base: only
//!   `crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so`
//!   exists while the host is `aarch64-apple-darwin`). A real terminal is
//!   provided via macOS `/usr/bin/script`, and the assertion targets the
//!   documented setup text, not a native frame. This file must never be read
//!   as proof of an installed native E2E.
//!
//! FIXTURE SAFETY. The test starts its OWN `oc2 serve` child, keeps the
//! `std::process::Child` handle, and on drop kills and waits ONLY that handle.
//! It never reads a PID from a descriptor and never signals an arbitrary PID,
//! so a stale, forged or foreign descriptor cannot redirect a kill. The owned
//! child is verified to have no descendant processes (single-handle kill is
//! therefore sufficient). Reader threads are drained through a bounded channel
//! receive so a retained pipe cannot hang the test.
//!
//! This target is std-only and is compiled directly with `rustc --test` so a
//! missing Cargo test target cannot make the journey appear to run.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

const CHILD_TIMEOUT: Duration = Duration::from_secs(15);
const DAEMON_READY_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_TIMEOUT: Duration = Duration::from_secs(2);
const READER_DRAIN_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_STREAM_BYTES: usize = 32 * 1024;
const MAX_HTTP_BYTES: usize = 64 * 1024;

/// Stable substring of `app_start::setup_message()` (`app_start.rs:344-346`).
const SETUP_MARKER: &str = "opening in-app setup to configure a provider or local endpoint";

struct Captured {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    timed_out: bool,
    overflowed: bool,
    /// False when a reader thread did not reach EOF within the bounded drain
    /// window (a descendant retained the pipe); output is then incomplete.
    drained: bool,
}

/// Owned disposable daemon: the test owns the `Child` handle and the HOME
/// tree. Dropping the guard kills and reaps ONLY this handle.
struct OwnedDaemon {
    child: Child,
    home: PathBuf,
}

impl OwnedDaemon {
    fn spawn(binary: &Path, home: &Path, port: u16) -> Result<Self, String> {
        let mut command = Command::new(binary);
        command
            .args(["serve", "--listen", &format!("127.0.0.1:{port}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_command(&mut command, home, port);
        let child = command
            .spawn()
            .map_err(|error| format!("spawn owned oc2 serve: {error}"))?;
        Ok(Self {
            child,
            home: home.to_path_buf(),
        })
    }

    /// Bounded wait until the owned daemon answers `/health` with 200.
    fn wait_ready(&mut self, port: u16) -> bool {
        let deadline = Instant::now() + DAEMON_READY_TIMEOUT;
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                return false;
            }
            if matches!(http_status(port, "/health"), Ok(200)) {
                return true;
            }
            thread::sleep(Duration::from_millis(50));
        }
        false
    }

    fn still_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    fn descendant_count(&mut self) -> Option<usize> {
        let output = Command::new("/usr/bin/pgrep")
            .arg("-P")
            .arg(self.child.id().to_string())
            .output()
            .ok()?;
        Some(
            output
                .stdout
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty())
                .count(),
        )
    }
}

impl Drop for OwnedDaemon {
    fn drop(&mut self) {
        // Positively owned cleanup only: kill and reap the exact child we
        // spawned. No descriptor PID, no /bin/kill, no process-group signal.
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.home);
    }
}

fn fresh_home() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "opencode2-app012-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create disposable HOME");
    path
}

fn source_binary() -> PathBuf {
    let raw = std::env::var_os("OC2_TEST_BINARY")
        .expect("PREREQUISITE: OC2_TEST_BINARY must name a staged real oc2 executable");
    let path = PathBuf::from(raw);
    assert!(
        path.is_file(),
        "PREREQUISITE: staged oc2 is not a file: {}",
        path.display()
    );
    assert!(
        is_executable(&path),
        "PREREQUISITE: staged oc2 is not executable: {}",
        path.display()
    );
    path
}

fn stage_binary_under_home(source: &Path, home: &Path) -> PathBuf {
    let install_dir = home.join(".local/bin");
    fs::create_dir_all(&install_dir).expect("create disposable install directory");
    let target = install_dir.join("oc2");
    fs::copy(source, &target).expect("stage real oc2 under disposable HOME");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&target)
            .expect("stat staged oc2")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&target, permissions).expect("make staged oc2 executable");
    }
    assert!(is_executable(&target), "staged HOME oc2 is not executable");
    target
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn selected_port() -> u16 {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("select loopback port");
    listener
        .local_addr()
        .expect("read selected port")
        .port()
}

fn configure_command(command: &mut Command, home: &Path, port: u16) {
    command
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .env("TERM", "xterm-256color")
        .env("LANG", "C")
        .env("OPENCODE_RK_DAEMON_ADDR", format!("127.0.0.1:{port}"));
}

/// Bounded reader that always DRAINS the pipe to EOF (never backpressures the
/// child) while retaining at most [`MAX_STREAM_BYTES`]. It reports once over a
/// channel so the test can wait with a bounded timeout instead of an unbounded
/// `join` that a retained pipe descriptor could hang.
fn bounded_reader<R: Read + Send + 'static>(mut reader: R) -> Receiver<(Vec<u8>, bool)> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut kept: Vec<u8> = Vec::with_capacity(4096);
        let mut overflowed = false;
        let mut chunk = [0u8; 8192];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => {
                    let remaining = MAX_STREAM_BYTES.saturating_sub(kept.len());
                    if read > remaining {
                        kept.extend_from_slice(&chunk[..remaining]);
                        overflowed = true;
                    } else {
                        kept.extend_from_slice(&chunk[..read]);
                    }
                }
                Err(_) => break,
            }
        }
        let _ = sender.send((kept, overflowed));
    });
    receiver
}

fn drain(receiver: Receiver<(Vec<u8>, bool)>) -> (Vec<u8>, bool, bool) {
    match receiver.recv_timeout(READER_DRAIN_TIMEOUT) {
        Ok((bytes, overflowed)) => (bytes, overflowed, true),
        Err(_) => (Vec::new(), false, false),
    }
}

fn run_child(mut command: Command, input: Option<&[u8]>) -> Captured {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn installed application");
    let stdout = bounded_reader(child.stdout.take().expect("capture stdout"));
    let stderr = bounded_reader(child.stderr.take().expect("capture stderr"));
    match input {
        Some(input) => {
            let mut stdin = child.stdin.take().expect("capture stdin");
            stdin.write_all(input).expect("write bounded test input");
        }
        None => {
            drop(child.stdin.take());
        }
    }

    let deadline = Instant::now() + CHILD_TIMEOUT;
    let mut timed_out = false;
    let status = loop {
        match child.try_wait().expect("poll installed application") {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                timed_out = true;
                let _ = child.kill();
                break child.wait().expect("wait killed installed application");
            }
            None => thread::sleep(Duration::from_millis(20)),
        }
    };
    let (stdout, stdout_overflow, stdout_drained) = drain(stdout);
    let (stderr, stderr_overflow, stderr_drained) = drain(stderr);
    Captured {
        status,
        stdout,
        stderr,
        timed_out,
        overflowed: stdout_overflow || stderr_overflow,
        drained: stdout_drained && stderr_drained,
    }
}

fn run_installed(
    binary: &Path,
    home: &Path,
    port: u16,
    args: &[&str],
    input: Option<&[u8]>,
) -> Captured {
    let mut command = Command::new(binary);
    command.args(args);
    configure_command(&mut command, home, port);
    run_child(command, input)
}

/// Launch the installed client under a real terminal (macOS `/usr/bin/script`),
/// feeding `/exit` so a correctly behaving client terminates promptly.
fn run_installed_tty(binary: &Path, transcript: &Path, home: &Path, port: u16) -> Captured {
    let mut command = Command::new("/usr/bin/script");
    command.args(["-q", transcript.to_str().expect("transcript path is UTF-8")]);
    command.arg(binary);
    configure_command(&mut command, home, port);
    run_child(command, Some(b"/exit\n"))
}

fn http_status(port: u16, path: &str) -> Result<u16, String> {
    let address = ("127.0.0.1", port)
        .to_socket_addrs()
        .map_err(|error| error.to_string())?
        .next()
        .ok_or_else(|| "no loopback address".to_owned())?;
    let mut stream =
        TcpStream::connect_timeout(&address, HTTP_TIMEOUT).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(HTTP_TIMEOUT))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(HTTP_TIMEOUT))
        .map_err(|e| e.to_string())?;
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let read = stream.read(&mut chunk).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        if raw.len() + read > MAX_HTTP_BYTES {
            return Err("HTTP response exceeded bound".to_owned());
        }
        raw.extend_from_slice(&chunk[..read]);
    }
    let head = String::from_utf8_lossy(&raw);
    head.lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| "missing HTTP status".to_owned())
}

/// Prerequisite gate: the staged executable must answer `--version` with its
/// own `oc2` identity before any RED assertion is meaningful. A failure here is
/// a broken fixture, never evidence about the missing setup behavior.
fn assert_prerequisite_identity(binary: &Path, home: &Path, port: u16) {
    let version = run_installed(binary, home, port, &["--version"], None);
    assert!(
        !version.timed_out,
        "PREREQUISITE: `oc2 --version` timed out; staged executable is not functional"
    );
    assert!(
        version.status.success(),
        "PREREQUISITE: `oc2 --version` exit {:?}: {}",
        version.status.code(),
        String::from_utf8_lossy(&version.stderr)
    );
    let text = String::from_utf8_lossy(&version.stdout);
    assert!(
        text.contains("oc2"),
        "PREREQUISITE: `oc2 --version` lacks oc2 identity: {text}"
    );
}

#[cfg(unix)]
#[test]
fn installed_local_application_setup_state() {
    assert!(
        Path::new("/usr/bin/script").is_file(),
        "PREREQUISITE: macOS /usr/bin/script is required for a real terminal"
    );

    let source = source_binary();
    let home = fresh_home();
    let binary = stage_binary_under_home(&source, &home);
    let port = selected_port();

    // 1. Prerequisite identity check must pass; not part of the RED.
    assert_prerequisite_identity(&binary, &home, port);

    // 2. Owned disposable daemon so the installed client attaches instead of
    //    spawning an unmanaged orphan. Cleanup is the owned Child handle only.
    let mut daemon =
        OwnedDaemon::spawn(&binary, &home, port).expect("PREREQUISITE: start owned oc2 serve");
    assert!(
        daemon.wait_ready(port),
        "PREREQUISITE: owned daemon never became healthy on 127.0.0.1:{port}"
    );
    assert_eq!(
        daemon.descendant_count(),
        Some(0),
        "owned daemon unexpectedly has descendants; single-handle kill unsafe"
    );

    // 3. Installed client, fresh HOME, no provider credentials, real terminal.
    let transcript = home.join("terminal.log");
    let launch = run_installed_tty(&binary, &transcript, &home, port);
    let stdout = String::from_utf8_lossy(&launch.stdout);
    let stderr = String::from_utf8_lossy(&launch.stderr);

    assert!(!launch.timed_out, "installed client did not exit within timeout");
    assert!(!launch.overflowed, "installed client output exceeded byte bound");
    assert!(
        launch.drained,
        "installed client pipes were not drained within the bounded window: {stderr}"
    );
    assert_eq!(
        launch.status.code(),
        Some(0),
        "installed client failed before the setup state: stdout={stdout} stderr={stderr}"
    );
    assert!(transcript.is_file(), "script must archive a terminal recording");

    // 4. The app must surface the documented in-app setup state. At this base
    //    it instead enters the compatibility chat, which is the RED failure.
    //    The captured output is reported in the failure message so it cannot be
    //    misread as a missing executable or missing native library: the child
    //    exits 0 and prints the chat banner.
    assert!(
        stdout.contains(SETUP_MARKER),
        "installed app did not surface in-app setup without credentials; \
         observed chat path instead. setup marker missing from: {stdout}"
    );

    // 5. Sanity: the client attached to the owned daemon, which survives client
    //    exit. Reached only after the setup assertion, so it documents contract
    //    rather than masking the RED.
    assert!(
        daemon.still_running(),
        "owned daemon did not survive installed client exit"
    );
    assert!(
        matches!(http_status(port, "/health"), Ok(200)),
        "owned daemon /health not 200 after client exit"
    );
}

#[cfg(not(unix))]
#[test]
fn installed_local_application_setup_state() {
    panic!("APP-012 installed journey requires a unix terminal harness");
}