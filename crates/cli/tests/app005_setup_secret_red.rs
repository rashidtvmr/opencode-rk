#![cfg(target_os = "macos")]
#![forbid(unsafe_code)]

//! APP-005-SECURE-RED: provider-setup secret masking (RED).
//!
//! Bare `oc2` with no provider credential opens in-app provider setup.
//! Typing a credential-like sentinel (never submitted) must never surface
//! raw in PTY output; a non-secret masked marker (`[hidden]`) must appear
//! instead. Ctrl-C from the nonempty draft must exit success with bounded
//! alternate-screen restoration. Currently RED: `tui_entry.rs`
//! `native_page_lines` echoes `draft` verbatim (`> {draft}`).

use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const CAP: usize = 64 * 1024;
const WAIT: Duration = Duration::from_secs(12);
const SETTLE: Duration = Duration::from_millis(1500);
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let p =
            std::env::temp_dir().join(format!("oc2-app005-secure-{}-{n}", std::process::id()));
        fs::create_dir_all(p.join("home")).expect("disposable HOME");
        fs::create_dir_all(p.join("data")).expect("disposable data");
        Self(p)
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
    fn new(mut r: impl Read + Send + 'static) -> Self {
        let state = Arc::new(Mutex::new(Bytes::default()));
        let copy = Arc::clone(&state);
        let join = thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                let n = match r.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                let mut out = copy.lock().unwrap();
                if out.0.len() + n > CAP {
                    let drop_n = out.0.len() + n - CAP;
                    let remove = drop_n.min(out.0.len());
                    out.0.drain(..remove);
                }
                out.0.extend_from_slice(&buf[..n]);
            }
        });
        Self {
            state,
            join: Some(join),
        }
    }
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.state.lock().unwrap().0).into_owned()
    }
    fn join(&mut self) {
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.join();
    }
}

struct Ptty {
    child: Child,
    input: std::process::ChildStdin,
    out: Capture,
    err: Capture,
}
impl Ptty {
    fn spawn(bin: &Path, root: &Root, addr: &str) -> Self {
        let mut c = Command::new("/usr/bin/script");
        c.args(["-q", "/dev/null"])
            .arg(bin)
            .args(["--data-dir", root.data().to_str().unwrap()])
            .env_clear()
            .env("HOME", root.home())
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("TERM", "xterm-256color")
            .env("OPENCODE_RK_DAEMON_ADDR", addr);
        c.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = c.spawn().expect("spawn oc2 under /usr/bin/script");
        let input = child.stdin.take().expect("pty input");
        let out = Capture::new(child.stdout.take().expect("pty stdout"));
        let err = Capture::new(child.stderr.take().expect("pty stderr"));
        Self {
            child,
            input,
            out,
            err,
        }
    }
    fn send(&mut self, s: &str) {
        self.input.write_all(s.as_bytes()).unwrap();
        self.input.flush().unwrap();
    }
    fn send_byte(&mut self, b: u8) {
        self.input.write_all(&[b]).unwrap();
        self.input.flush().unwrap();
    }
    fn wait_for(&self, needle: &str) {
        // Only used before the sentinel is typed, so raw output is secret-free.
        let until = Instant::now() + WAIT;
        while Instant::now() < until {
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
    fn exit(&mut self) -> std::process::ExitStatus {
        let until = Instant::now() + WAIT;
        loop {
            if let Some(s) = self.child.try_wait().unwrap() {
                self.out.join();
                self.err.join();
                return s;
            }
            if Instant::now() >= until {
                let _ = self.child.kill();
                let _ = self.child.wait();
                panic!("oc2 did not exit");
            }
            thread::sleep(Duration::from_millis(25));
        }
    }
    fn text(&self) -> String {
        format!("{}\n{}", self.out.text(), self.err.text())
    }
}
impl Drop for Ptty {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.out.join();
        self.err.join();
    }
}

fn binary() -> PathBuf {
    std::env::var_os("OC2_E2E_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_oc2")))
}

fn loopback_addr() -> String {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .to_string()
}

/// Redact the sentinel from captured output before it ever reaches a failure
/// message, log, or worklog excerpt.
fn redact(text: &str, sentinel: &str) -> String {
    text.replace(sentinel, "[REDACTED-SENTINEL]")
}

fn descriptor_pid(data: &Path) -> Option<u32> {
    let raw = fs::read_to_string(data.join("runtime/backend.json")).ok()?;
    let marker = "\"pid\":";
    let s = raw.find(marker)? + marker.len();
    raw[s..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()
}

fn cleanup_daemon(data: &Path) {
    if let Some(pid) = descriptor_pid(data) {
        if pid > 1 {
            let _ = Command::new("/bin/kill").arg(pid.to_string()).output();
        }
    }
}

#[test]
fn setup_secret_masked_and_ctrl_c_restores() {
    let root = Root::new();
    let addr = loopback_addr();
    let mut p = Ptty::spawn(&binary(), &root, &addr);
    p.wait_for("provider setup");
    // Unique credential-like sentinel, typed but never submitted.
    let sentinel = format!(
        "sk-sentinel-{:08x}{:08x}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
            ^ (Instant::now().elapsed().as_nanos() as u64),
    );
    assert!(sentinel.len() >= 16, "sentinel must be credential-length");
    p.send(&sentinel);
    thread::sleep(SETTLE);
    let text = p.text();
    let shown = redact(&text, &sentinel);
    assert!(
        !text.contains(&sentinel),
        "provider setup echoed raw secret bytes; redacted capture:\n{shown}"
    );
    assert!(
        text.contains("[hidden]"),
        "provider setup must show a non-secret masked marker; redacted capture:\n{shown}"
    );
    // Nonempty draft: Ctrl-C (byte 3), not :q, must quit and restore.
    p.send_byte(3);
    let status = p.exit();
    let text = p.text();
    let shown = redact(&text, &sentinel);
    cleanup_daemon(&root.data());
    assert!(
        status.success(),
        "Ctrl-C from setup draft exit: {status}\n{shown}"
    );
    assert!(
        text.contains("\x1b[?1049l"),
        "alternate-screen restoration sequence missing\n{shown}"
    );
}
