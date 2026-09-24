#![cfg(target_os = "macos")]
#![forbid(unsafe_code)]

//! Installed default-entrypoint journey through the real macOS PTY.
//!
//! Source evidence (current code, 2026-09-23):
//! - `crates/cli/src/main.rs:225-271` - no-subcommand planning and native route.
//! - `crates/cli/src/app_start.rs:287-320` - TTY owner/attacher and setup/main view.
//! - `crates/cli/src/chat.rs:99-137,657-698` - authenticated singleton discovery,
//!   spawn, and client lease.
//! - `crates/cli/src/daemon_client.rs:661-743` - bounded descriptor, bearer,
//!   loopback, and PID validation.
//! - `crates/cli/src/tui_entry.rs:1393-1455,1470-1522,1606-1625` - default
//!   setup/main, one-shot frame, and the real OpenTUI `render_once` caller.
//! - `crates/cli/tests/app001_repair_e2e.rs:18-124` - bounded macOS PTY
//!   capture, disposable roots, loopback selection, and descriptor helpers.

use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const CAP: usize = 64 * 1024;
const WAIT: Duration = Duration::from_secs(12);
const CLEANUP_WAIT: Duration = Duration::from_secs(2);
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Root(PathBuf);

impl Root {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("oc2-installed-default-{}-{n}", std::process::id()));
        fs::create_dir_all(path.join("home")).expect("disposable HOME");
        fs::create_dir_all(path.join("data")).expect("disposable data");
        Self(path)
    }

    fn home(&self) -> PathBuf {
        self.0.join("home")
    }

    fn data(&self) -> PathBuf {
        self.0.join("data")
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        if let Some(desc) = bounded_file(&self.data().join("runtime/backend.json")) {
            if let Some(pid) = descriptor_pid(&desc) {
                terminate_pid(pid);
            }
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Default)]
struct Bytes(Vec<u8>);

struct Capture {
    state: Arc<Mutex<Bytes>>,
    join: Option<thread::JoinHandle<()>>,
}

impl Capture {
    fn new(mut reader: impl Read + Send + 'static) -> Self {
        let state = Arc::new(Mutex::new(Bytes::default()));
        let copy = Arc::clone(&state);
        let join = thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            loop {
                let count = match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => count,
                };
                let mut output = copy.lock().expect("capture lock");
                if output.0.len() + count > CAP {
                    let remove = (output.0.len() + count - CAP).min(output.0.len());
                    output.0.drain(..remove);
                }
                output.0.extend_from_slice(&buffer[..count]);
            }
        });
        Self {
            state,
            join: Some(join),
        }
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.state.lock().expect("capture lock").0).into_owned()
    }

    fn join(&mut self) {
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.join();
    }
}

struct Pty {
    child: Child,
    input: std::process::ChildStdin,
    out: Capture,
    err: Capture,
}

impl Pty {
    fn spawn(bin: &Path, root: &Root, addr: &str, key: Option<&str>) -> Self {
        let mut command = Command::new("/usr/bin/script");
        command
            .args(["-q", "/dev/null"])
            .arg(bin)
            .args(["--data-dir", root.data().to_str().expect("data path")])
            .env_clear()
            .env("HOME", root.home())
            .env("OPENCODE_RK_HOME", root.home())
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("TERM", "xterm-256color")
            .env("OPENCODE_RK_DAEMON_ADDR", addr)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(secret) = key {
            command.env("OPENAI_API_KEY", secret);
        }
        let mut child = command
            .spawn()
            .expect("spawn executable under macOS script PTY");
        let input = child.stdin.take().expect("PTY input");
        let out = Capture::new(child.stdout.take().expect("PTY stdout"));
        let err = Capture::new(child.stderr.take().expect("PTY stderr"));
        Self {
            child,
            input,
            out,
            err,
        }
    }

    fn send(&mut self, input: &str) {
        self.input.write_all(input.as_bytes()).expect("PTY write");
        self.input.flush().expect("PTY flush");
    }

    fn wait_for(&self, needle: &str) {
        let deadline = Instant::now() + WAIT;
        while Instant::now() < deadline {
            if self.out.text().contains(needle) {
                return;
            }
            thread::sleep(Duration::from_millis(25));
        }
        panic!(
            "missing {needle:?}\nstdout={}\nstderr={}",
            self.out.text(),
            self.err.text()
        );
    }

    fn exit(&mut self) -> ExitStatus {
        let deadline = Instant::now() + WAIT;
        loop {
            if let Some(status) = self.child.try_wait().expect("PTY wait") {
                self.out.join();
                self.err.join();
                return status;
            }
            if Instant::now() >= deadline {
                terminate_child(&mut self.child);
                panic!(
                    "executable did not exit\nstdout={}\nstderr={}",
                    self.out.text(),
                    self.err.text()
                );
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    fn text(&self) -> String {
        format!("{}\n{}", self.out.text(), self.err.text())
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        terminate_child(&mut self.child);
        self.out.join();
        self.err.join();
    }
}

fn binary() -> PathBuf {
    std::env::var_os("OC2_E2E_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_oc2")))
}

fn address() -> String {
    TcpListener::bind("127.0.0.1:0")
        .expect("loopback address")
        .local_addr()
        .expect("loopback local address")
        .to_string()
}

fn bounded_file(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take((CAP + 1) as u64).read_to_end(&mut bytes).ok()?;
    if bytes.len() > CAP {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn field(text: &str, name: &str) -> Option<String> {
    let marker = format!("\"{name}\":\"");
    let start = text.find(&marker)? + marker.len();
    let end = text[start..].find('"')? + start;
    Some(text[start..end].to_owned())
}

fn descriptor_pid(text: &str) -> Option<u32> {
    let marker = "\"pid\":";
    let start = text.find(marker)? + marker.len();
    let digits: String = text[start..]
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect();
    digits.parse().ok().filter(|pid| *pid > 1)
}

fn wait_for_descriptor(root: &Root, addr: &str) -> String {
    let path = root.data().join("runtime/backend.json");
    let deadline = Instant::now() + WAIT;
    while Instant::now() < deadline {
        if let Some(desc) = bounded_file(&path) {
            if field(&desc, "http_origin").as_deref() == Some(format!("http://{addr}").as_str())
                && field(&desc, "auth_token").is_some()
                && descriptor_pid(&desc).is_some()
            {
                return desc;
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("backend descriptor not published at {}", path.display());
}

fn request(addr: &str, path: &str, bearer: Option<&str>) -> (u16, String) {
    let mut stream = TcpStream::connect_timeout(
        &addr.parse().expect("loopback address"),
        Duration::from_secs(1),
    )
    .expect("daemon connection");
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("daemon read timeout");
    let authorization = bearer
        .map(|token| format!("Authorization: Bearer {token}\r\n"))
        .unwrap_or_default();
    let wire = format!(
        "GET {path} HTTP/1.1\r\nHost: localhost\r\n{authorization}Connection: close\r\n\r\n"
    );
    stream.write_all(wire.as_bytes()).expect("daemon request");
    let mut bytes = Vec::new();
    stream
        .take((CAP + 1) as u64)
        .read_to_end(&mut bytes)
        .expect("daemon response");
    assert!(bytes.len() <= CAP, "daemon response exceeded 64KiB");
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .expect("HTTP status");
    (status, text)
}

fn health(addr: &str) -> bool {
    request(addr, "/health", None).0 == 200
}

fn process_alive(pid: u32) -> bool {
    Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn signal(pid: u32, signal_name: &str) {
    let _ = Command::new("/bin/kill")
        .args([signal_name, &pid.to_string()])
        .stderr(Stdio::null())
        .status();
}

fn terminate_pid(pid: u32) {
    if pid <= 1 || !process_alive(pid) {
        return;
    }
    signal(pid, "-TERM");
    let deadline = Instant::now() + CLEANUP_WAIT;
    while process_alive(pid) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(25));
    }
    if process_alive(pid) {
        signal(pid, "-KILL");
        let deadline = Instant::now() + CLEANUP_WAIT;
        while process_alive(pid) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(25));
        }
    }
}

fn terminate_child(child: &mut Child) {
    if child.try_wait().ok().flatten().is_some() {
        return;
    }
    signal(child.id(), "-TERM");
    let deadline = Instant::now() + CLEANUP_WAIT;
    while child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(25));
    }
    if child.try_wait().ok().flatten().is_none() {
        signal(child.id(), "-KILL");
    }
    let _ = child.wait();
}

fn revision_receipt() -> Option<String> {
    std::env::var("OC2_E2E_REVISION")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| option_env!("GIT_COMMIT").map(str::to_owned))
}

fn strip_terminal_controls(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(character) = chars.next() {
        if character != '\x1b' {
            output.push(character);
            continue;
        }
        if chars.next() == Some('[') {
            for control in chars.by_ref() {
                if ('@'..='~').contains(&control) {
                    break;
                }
            }
        }
    }
    output.replace('\r', "")
}

#[test]
fn bare_no_command_enters_planned_native_route() {
    let root = Root::new();
    let addr = address();
    let mut pty = Pty::spawn(&binary(), &root, &addr, None);
    pty.wait_for("OpenCode RK TUI");
    let initial = pty.text();
    assert!(
        initial.contains("provider setup"),
        "planned setup view missing\n{initial}"
    );
    assert!(
        initial.contains("Ctrl+P"),
        "native route marker missing\n{initial}"
    );
    assert!(
        !initial.contains("type /help for commands"),
        "compatibility chat route\n{initial}"
    );
    assert!(
        !initial.contains("manual") && !initial.contains("serve"),
        "manual serve path\n{initial}"
    );
    pty.send(":q\n");
    let status = pty.exit();
    assert!(
        status.success(),
        "planned native route exit: {status}\n{}",
        pty.text()
    );
}

#[test]
fn single_authenticated_daemon_reused_by_second_client() {
    let root = Root::new();
    let addr = address();
    let bin = binary();
    let mut first = Pty::spawn(&bin, &root, &addr, Some("fixture-secret"));
    first.wait_for("OpenCode RK TUI");
    let first_desc = wait_for_descriptor(&root, &addr);
    let token = field(&first_desc, "auth_token").expect("bearer descriptor token");
    assert!(token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let first_pid = descriptor_pid(&first_desc).expect("daemon PID");
    assert_eq!(
        request(&addr, "/api/sessions", None).0,
        401,
        "API accepted no bearer"
    );
    assert_eq!(
        request(&addr, "/api/sessions", Some(&token)).0,
        200,
        "bearer rejected"
    );
    assert!(health(&addr), "daemon health failed");

    first.send(":q\n");
    assert!(
        first.exit().success(),
        "first client exit failed\n{}",
        first.text()
    );
    assert!(health(&addr), "first client exit killed shared daemon");

    let mut second = Pty::spawn(&bin, &root, &addr, Some("fixture-secret"));
    second.wait_for("OpenCode RK TUI");
    let second_desc = wait_for_descriptor(&root, &addr);
    assert_eq!(
        descriptor_pid(&second_desc).expect("second daemon PID"),
        first_pid,
        "second client started a second daemon"
    );
    assert_eq!(field(&second_desc, "auth_token"), Some(token));
    assert!(health(&addr), "second client daemon unhealthy");
    second.send(":q\n");
    assert!(
        second.exit().success(),
        "second client exit failed\n{}",
        second.text()
    );
}

#[test]
fn missing_credentials_open_setup_without_offline_instruction_or_secret() {
    let root = Root::new();
    let addr = address();
    let mut pty = Pty::spawn(&binary(), &root, &addr, None);
    pty.wait_for("OpenCode RK TUI");
    pty.send(":q\n");
    let status = pty.exit();
    let text = pty.text();
    assert!(status.success(), "setup exit: {status}\n{text}");
    assert!(
        text.to_ascii_lowercase().contains("setup"),
        "setup view missing\n{text}"
    );
    assert!(!text.contains("offline"), "offline path exposed\n{text}");
    assert!(
        !text.contains("serve") && !text.contains("session create"),
        "manual instruction\n{text}"
    );
    assert!(
        !text.contains("OPENAI_API_KEY"),
        "raw credential name exposed\n{text}"
    );
    assert!(
        !text.contains("fixture-secret") && !text.contains("Bearer "),
        "raw secret exposed\n{text}"
    );
}

#[test]
fn native_renderer_emits_frame_and_restores_alternate_screen() {
    let root = Root::new();
    let addr = address();
    let mut pty = Pty::spawn(&binary(), &root, &addr, Some("fixture-secret"));
    pty.wait_for("Ctrl+P");
    let frame = pty.text();
    assert!(
        frame.contains("OpenCode RK TUI"),
        "native frame missing title\n{frame}"
    );
    assert!(
        strip_terminal_controls(&frame)
            .lines()
            .any(|line| !line.trim().is_empty()),
        "native frame is empty\n{frame}"
    );
    assert!(
        frame.contains("\x1b[?1049h"),
        "native renderer did not enter alternate screen\n{frame}"
    );
    assert!(
        !frame.to_ascii_lowercase().contains("legacy"),
        "fallback claim in native frame\n{frame}"
    );
    pty.send(":q\n");
    let status = pty.exit();
    let final_text = pty.text();
    assert!(
        status.success(),
        "native renderer exit: {status}\n{final_text}"
    );
    assert!(
        final_text.contains("\x1b[?1049l"),
        "alternate screen was not restored\n{final_text}"
    );
}

#[test]
fn rerun_same_executable_and_revision_is_deterministic() {
    let revision = revision_receipt().expect(
        "revision receipt missing: set OC2_E2E_REVISION in packaging or provide compile-time GIT_COMMIT",
    );
    assert!(!revision.trim().is_empty(), "revision receipt is empty");
    let bin = binary();
    assert!(
        bin.is_file(),
        "supplied executable is missing: {}",
        bin.display()
    );

    let first_root = Root::new();
    let first_addr = address();
    let mut first = Pty::spawn(&bin, &first_root, &first_addr, None);
    first.wait_for("Ctrl+P");
    let first_frame = strip_terminal_controls(&first.text());
    first.send(":q\n");
    assert!(
        first.exit().success(),
        "first deterministic run failed\n{}",
        first.text()
    );

    let second_root = Root::new();
    let second_addr = address();
    let mut second = Pty::spawn(&bin, &second_root, &second_addr, None);
    second.wait_for("Ctrl+P");
    let second_frame = strip_terminal_controls(&second.text());
    second.send(":q\n");
    assert!(
        second.exit().success(),
        "second deterministic run failed\n{}",
        second.text()
    );

    assert_eq!(
        first_frame, second_frame,
        "same executable/revision changed native frame"
    );
}
