#![cfg(unix)]
#![forbid(unsafe_code)]
//! APP-001-SETUP-REPAIR-RED: true interactive setup state transition.
//!
//! Source evidence (commit `7938791ba76645115e418928a728869b48beb73b`):
//! - `crates/cli/src/main.rs:224-268` (`run`, `None` arm): no-subcommand TTY
//!   launch computes `plan_default_launch` but the non-native build calls
//!   `chat::run(&data)`; `app_start::needs_setup`/`StartupView::Setup` are never
//!   consumed, so missing provider credentials do not open in-app setup.
//! - `crates/cli/src/chat.rs:134-155` (`run`): prints the banner, the offline
//!   hint, and enters the line loop. No provider-selection or credential state.
//! - `crates/cli/src/chat.rs:447-479` (`Chat::loop_until_exit`): a plain line is
//!   a chat turn, not a setup interaction; a provider id cannot advance setup.
//! - `crates/cli/src/onboarding.rs:48-74` (`SetupStep`) and
//!   `native_host.rs:100-129`: the real ordered setup state is
//!   `Welcome -> ProviderSelect -> CredentialEntry -> ModelSelect -> Done`.
//! - `crates/cli/src/app_start.rs:337-346` (`needs_setup`/`setup_message`): a
//!   *sentence* is not interactive proof; matching it must not pass this test.
//!
//! Contract asserted here: with fresh disposable state and no provider
//! credential, a bare interactive launch must render a real provider-selection
//! state, accept a provider id, and then render the next credential-entry
//! state. A print-only `setup_message()` line, the chat banner, or the model
//! line must never satisfy either state check.
//!
//! This test is std-only so it compiles as a root integration target with no
//! new dependencies. It fails today because the first setup state is missing.
//! Missing `oc2`/PTY infrastructure is reported as a distinct setup failure,
//! never as a behavioral pass.

use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

/// Hard cap on retained child output. Overflow fails the test.
const OUTPUT_CAP: usize = 32 * 1024;
/// Per-wait bound for one UI observation.
const WAIT: Duration = Duration::from_secs(15);
/// Bounded teardown budget for the owned `script` process.
const EXIT_WAIT: Duration = Duration::from_secs(15);
/// Bounded connection budget for the in-test loopback health fixture.
const HEALTH_MAX_CONNS: usize = 64;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Fresh disposable state: its own HOME and data dir, recursively removed on
/// drop. Never touches the user's real OpenCode database or credentials.
struct TestState(PathBuf);

impl TestState {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "oc2-setup-flow-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(path.join("home")).expect("disposable HOME");
        fs::create_dir_all(path.join("data")).expect("disposable data dir");
        Self(path)
    }
    fn home(&self) -> &Path {
        &self.0
    }
    fn data(&self) -> PathBuf {
        self.0.join("data")
    }
}

impl Drop for TestState {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Fixed-capacity, bounded capture. Older bytes are dropped once the cap is
/// reached; the overflow flag is sticky and fails the test.
struct Capture {
    bytes: Arc<Mutex<Vec<u8>>>,
    overflow: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}

impl Capture {
    fn new(mut reader: impl Read + Send + 'static) -> Self {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let overflow = Arc::new(AtomicBool::new(false));
        let sink = Arc::clone(&bytes);
        let flag = Arc::clone(&overflow);
        let join = thread::spawn(move || {
            let mut buf = [0_u8; 4096];
            loop {
                let read = match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                let mut out = sink.lock().unwrap();
                out.extend_from_slice(&buf[..read]);
                if out.len() > OUTPUT_CAP {
                    flag.store(true, Ordering::Relaxed);
                    let drop_n = out.len() - OUTPUT_CAP;
                    out.drain(..drop_n);
                }
            }
        });
        Self {
            bytes,
            overflow,
            join: Some(join),
        }
    }
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes.lock().unwrap()).into_owned()
    }
    fn overflowed(&self) -> bool {
        self.overflow.load(Ordering::Relaxed)
    }
    fn finish(&mut self) {
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.finish();
    }
}

/// Owned PTY client: exactly one `/usr/bin/script` process plus two reader
/// threads. No PID-based signalling; only this owned child handle is touched.
struct PtyClient {
    child: Child,
    input: std::process::ChildStdin,
    stdout: Capture,
    stderr: Capture,
}

impl PtyClient {
    fn spawn(binary: &Path, state: &TestState, daemon_addr: &str) -> Self {
        let mut command = Command::new("/usr/bin/script");
        command
            .args(["-q", "/dev/null"])
            .arg(binary)
            .args(["--data-dir"])
            .arg(state.data());
        command
            .env_clear()
            .env("HOME", state.home())
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("TERM", "xterm-256color")
            // Point at the owned loopback fixture so no real daemon is spawned
            // and no orphan process outlives the test.
            .env("OPENCODE_RK_DAEMON_ADDR", daemon_addr)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("spawn oc2 under /usr/bin/script");
        let input = child.stdin.take().expect("pty stdin");
        let stdout = Capture::new(child.stdout.take().expect("pty stdout"));
        let stderr = Capture::new(child.stderr.take().expect("pty stderr"));
        Self {
            child,
            input,
            stdout,
            stderr,
        }
    }

    fn send(&mut self, text: &str) {
        self.input
            .write_all(text.as_bytes())
            .expect("write pty input");
        self.input.flush().expect("flush pty input");
    }

    /// Wait for `needle` within `WAIT`. Returns the combined frame on success,
    /// or panics with the bounded captured frame on timeout.
    fn wait_for(&self, needle: &str) -> String {
        let deadline = Instant::now() + WAIT;
        loop {
            let frame = self.frame();
            if frame.contains(needle) {
                return frame;
            }
            if Instant::now() >= deadline {
                panic!(
                    "timed out after {WAIT:?} waiting for interactive setup marker {needle:?}\n\
                     --- captured frame ---\n{frame}\n--- end frame ---"
                );
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    fn frame(&self) -> String {
        format!(
            "{}\n{}",
            self.stdout.text(),
            self.stderr.text()
        )
    }

    /// Bounded teardown of the owned child only. Sends the documented exit
    /// keys, waits within `EXIT_WAIT`, and kills only this owned handle if the
    /// bound elapses.
    fn shutdown(&mut self) {
        let _ = self.send("/exit\n");
        let _ = self.send(":q\n");
        let deadline = Instant::now() + EXIT_WAIT;
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(25));
                }
                _ => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    break;
                }
            }
        }
        self.stdout.finish();
        self.stderr.finish();
    }
}

impl Drop for PtyClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.stdout.finish();
        self.stderr.finish();
    }
}

/// Loopback-only `/health` fixture owned by the test. Keeps `prepare_daemon`
/// from starting a real daemon, so no orphan process survives. Bounded in
/// connections and never reads or stores secret material.
struct HealthFixture {
    addr: String,
    stop: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}

impl HealthFixture {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback fixture");
        let addr = listener.local_addr().expect("fixture addr").to_string();
        listener
            .set_nonblocking(true)
            .expect("nonblocking fixture listener");
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let join = thread::spawn(move || {
            let mut served = 0usize;
            while !flag.load(Ordering::Relaxed) && served < HEALTH_MAX_CONNS {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        served += 1;
                        let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
                        let mut request = [0_u8; 1024];
                        let _ = stream.read(&mut request);
                        let body = "{\"status\":\"ok\"}";
                        let wire = format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\
                             content-length: {}\r\nconnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(wire.as_bytes());
                        let _ = stream.flush();
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            addr,
            stop,
            join: Some(join),
        }
    }
}

impl Drop for HealthFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn installed_binary() -> PathBuf {
    if let Some(runtime) = std::env::var_os("OC2_E2E_BIN") {
        return PathBuf::from(runtime);
    }
    match option_env!("CARGO_BIN_EXE_oc2") {
        Some(path) => PathBuf::from(path),
        None => panic!(
            "no oc2 binary: build with `cargo test -p opencode-rk-cli --test \
             installed_setup_flow --no-run` or set OC2_E2E_BIN"
        ),
    }
}

fn provider_ids_in(frame_lower: &str) -> usize {
    const IDS: &[&str] = &[
        "openai", "anthropic", "google", "gemini", "azure", "ollama", "local",
    ];
    IDS.iter()
        .filter(|id| frame_lower.contains(**id))
        .count()
}

/// True only for a real provider-selection state: a provider marker on a line
/// that is not the model banner, plus at least two provider identities so a
/// single `model: openai/...` banner or a print-only setup sentence cannot
/// satisfy it.
fn shows_provider_selection_state(frame: &str) -> bool {
    let lower = frame.to_lowercase();
    let provider_marker = lower.lines().any(|line| {
        line.contains("provider") && !line.contains("model:")
    });
    provider_marker && provider_ids_in(&lower) >= 2
}

/// True only for a credential-entry state: the UI is asking for a key/secret.
fn shows_credential_entry_state(frame: &str) -> bool {
    let lower = frame.to_lowercase();
    lower.contains("api key")
        || lower.contains("apikey")
        || lower.contains("credential")
        || lower.contains("secret")
        || lower.contains("token")
}

/// APP-001-SETUP-REPAIR-RED-T1: bare launch with no provider credential must
/// open the provider-selection state, not the chat banner.
#[test]
fn bare_launch_without_credentials_opens_provider_selection_state() {
    let binary = installed_binary();
    assert!(
        binary.is_file(),
        "setup failure (not behavioral RED): oc2 binary missing at {}",
        binary.display()
    );

    let state = TestState::new();
    let health = HealthFixture::start();
    let mut client = PtyClient::spawn(&binary, &state, &health.addr);

    // The first setup state must be visible in the initial frame. Today the
    // frame is only the chat banner and the offline/manual path, so this fails.
    let frame = client.wait_for("OpenCode RK");
    assert!(
        !client.stdout.overflowed(),
        "child output exceeded the {OUTPUT_CAP}-byte bound"
    );
    assert!(
        shows_provider_selection_state(&frame),
        "bare launch with missing credentials did not render a provider-selection \
         state; only banner/offline text was observed. A print-only setup sentence \
         or `model: openai/...` banner alone must not satisfy this.\n\
         --- captured frame ---\n{frame}\n--- end frame ---"
    );

    client.shutdown();
}

/// APP-001-SETUP-REPAIR-RED-T2: entering a provider id advances the interactive
/// setup to the credential-entry state, proving an actual state transition
/// rather than a single printed line.
#[test]
fn provider_id_input_advances_to_credential_entry_state() {
    let binary = installed_binary();
    assert!(
        binary.is_file(),
        "setup failure (not behavioral RED): oc2 binary missing at {}",
        binary.display()
    );

    let state = TestState::new();
    let health = HealthFixture::start();
    let mut client = PtyClient::spawn(&binary, &state, &health.addr);

    let initial = client.wait_for("OpenCode RK");
    assert!(
        !client.stdout.overflowed(),
        "child output exceeded the {OUTPUT_CAP}-byte bound"
    );
    // Precondition of the transition: the credential state is not already
    // present. If it were, the transition below would be vacuous.
    assert!(
        !shows_credential_entry_state(&initial),
        "credential-entry state was already present before any provider id was \
         entered; cannot attribute the transition to the input.\n\
         --- captured frame ---\n{initial}\n--- end frame ---"
    );

    // Drive the first setup step with a provider id only. Never a credential.
    client.send("openai\n");

    let deadline = Instant::now() + WAIT;
    loop {
        let frame = client.frame();
        if shows_credential_entry_state(&frame) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "after entering provider id `openai`, the UI never advanced to a \
             credential-entry state; no interactive setup transition exists.\n\
             --- captured frame ---\n{frame}\n--- end frame ---"
        );
        thread::sleep(Duration::from_millis(25));
    }

    client.shutdown();
}

/// APP-001-SETUP-REPAIR-RED-T3: the stream stays bounded and teardown is
/// owned. This pins the fixture contract; it does not substitute for T1/T2.
#[test]
fn capture_is_bounded_and_child_teardown_is_owned() {
    let binary = installed_binary();
    assert!(
        binary.is_file(),
        "setup failure (not behavioral RED): oc2 binary missing at {}",
        binary.display()
    );

    let state = TestState::new();
    let health = HealthFixture::start();
    let mut client = PtyClient::spawn(&binary, &state, &health.addr);
    let _ = client.wait_for("OpenCode RK");
    assert!(
        !client.stdout.overflowed(),
        "child output exceeded the {OUTPUT_CAP}-byte bound"
    );
    // Owned teardown returns within the bounded budget without external PIDs.
    client.shutdown();
    // The loopback fixture and disposable state are removed by their own Drops.
}