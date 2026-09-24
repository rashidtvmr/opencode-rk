#![cfg(unix)]
#![forbid(unsafe_code)]

//! APP-001 repair RED: exercise the real bare `oc2` entrypoint through a Unix
//! PTY.  This intentionally has no product mocks and keeps all state disposable.

use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{atomic::{AtomicU64, Ordering}, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

const CAP: usize = 64 * 1024;
const WAIT: Duration = Duration::from_secs(12);
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("oc2-app001-repair-{}-{n}", std::process::id()));
        fs::create_dir_all(p.join("home")).expect("disposable HOME");
        fs::create_dir_all(p.join("data")).expect("disposable data");
        Self(p)
    }
    fn home(&self) -> PathBuf { self.0.join("home") }
    fn data(&self) -> PathBuf { self.0.join("data") }
}
impl Drop for Root { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

#[derive(Default)] struct Bytes( Vec<u8> );
struct Capture { state: Arc<Mutex<Bytes>>, join: Option<thread::JoinHandle<()>> }
impl Capture {
    fn new(mut r: impl Read + Send + 'static) -> Self {
        let state = Arc::new(Mutex::new(Bytes::default()));
        let copy = Arc::clone(&state);
        let join = thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                let n = match r.read(&mut buf) { Ok(0) | Err(_) => break, Ok(n) => n };
                let mut out = copy.lock().unwrap();
                if out.0.len() + n > CAP {
                    let drop_n = out.0.len() + n - CAP;
                    let remove = drop_n.min(out.0.len());
                    out.0.drain(..remove);
                }
                out.0.extend_from_slice(&buf[..n]);
            }
        });
        Self { state, join: Some(join) }
    }
    fn text(&self) -> String { String::from_utf8_lossy(&self.state.lock().unwrap().0).into_owned() }
    fn join(&mut self) { if let Some(j) = self.join.take() { let _ = j.join(); } }
}
impl Drop for Capture { fn drop(&mut self) { self.join(); } }

struct Ptty { child: Child, input: std::process::ChildStdin, out: Capture, err: Capture }
impl Ptty {
    fn spawn(bin: &Path, root: &Root, addr: &str, key: Option<&str>) -> Self {
        let mut c = Command::new("/usr/bin/script");
        c.args(["-q", "/dev/null"])
            .arg(bin).args(["--data-dir", root.data().to_str().unwrap(),])
            .env_clear().env("HOME", root.home()).env("PATH", "/usr/bin:/bin")
            .env("LANG", "C").env("LC_ALL", "C").env("TERM", "xterm-256color")
            .env("OPENCODE_RK_DAEMON_ADDR", addr);
        if let Some(secret) = key { c.env("OPENAI_API_KEY", secret); }
        c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = c.spawn().expect("spawn oc2 under /usr/bin/script");
        let input = child.stdin.take().expect("pty input");
        let out = Capture::new(child.stdout.take().expect("pty stdout"));
        let err = Capture::new(child.stderr.take().expect("pty stderr"));
        Self { child, input, out, err }
    }
    fn send(&mut self, s: &str) { self.input.write_all(s.as_bytes()).unwrap(); self.input.flush().unwrap(); }
    fn wait_for(&self, needle: &str) {
        let until = Instant::now() + WAIT;
        while Instant::now() < until {
            if self.out.text().contains(needle) { return; }
            thread::sleep(Duration::from_millis(25));
        }
        panic!("missing {needle:?}\nstdout={}\nstderr={}", self.out.text(), self.err.text());
    }
    fn exit(&mut self) -> std::process::ExitStatus {
        let until = Instant::now() + WAIT;
        loop {
            if let Some(s) = self.child.try_wait().unwrap() { self.out.join(); self.err.join(); return s; }
            if Instant::now() >= until { let _ = self.child.kill(); let _ = self.child.wait(); panic!("oc2 did not exit"); }
            thread::sleep(Duration::from_millis(25));
        }
    }
    fn text(&self) -> String { format!("{}\n{}", self.out.text(), self.err.text()) }
}
impl Drop for Ptty { fn drop(&mut self) { let _ = self.child.kill(); let _ = self.child.wait(); self.out.join(); self.err.join(); } }

fn binary() -> PathBuf {
    std::env::var_os("OC2_E2E_BIN").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_oc2")))
}
fn address() -> String { TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().to_string() }
fn health(addr: &str) -> bool {
    let Ok(mut s) = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_millis(300)) else { return false };
    let _ = s.set_read_timeout(Some(Duration::from_millis(300)));
    if s.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").is_err() { return false }
    let mut b = [0u8; 256]; let n = s.read(&mut b).unwrap_or(0); b[..n].starts_with(b"HTTP/1.1 200")
}
fn field(text: &str, name: &str) -> Option<String> {
    let marker = format!("\"{name}\":\"");
    let start = text.find(&marker)? + marker.len();
    let end = text[start..].find('"')? + start;
    Some(text[start..end].to_owned())
}
fn descriptor(data: &Path) -> String { fs::read_to_string(data.join("runtime/backend.json")).expect("backend descriptor") }
fn pid(desc: &str) -> String { let marker = "\"pid\":"; let s = desc.find(marker).unwrap() + marker.len(); desc[s..].chars().take_while(|c| c.is_ascii_digit()).collect() }
fn sessions(desc: &str) -> bool {
    let origin = field(desc, "http_origin").unwrap(); let token = field(desc, "auth_token").unwrap();
    let rest = origin.strip_prefix("http://").unwrap(); let mut s = TcpStream::connect_timeout(&rest.parse().unwrap(), Duration::from_secs(1)).unwrap();
    let req = format!("GET /api/sessions HTTP/1.1\r\nHost: {origin}\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes()).unwrap(); let mut b = Vec::new(); s.take(CAP as u64).read_to_end(&mut b).unwrap();
    let body = String::from_utf8_lossy(&b); body.contains("HTTP/1.1 200") && body.contains("\"sessions\":[{")
}

#[test]
fn bare_without_provider_opens_setup_and_restores_terminal() {
    let root = Root::new(); let addr = address(); let mut p = Ptty::spawn(&binary(), &root, &addr, None);
    p.wait_for("OpenCode RK TUI"); p.send(":q\n"); let status = p.exit(); let text = p.text();
    assert!(status.success(), "bare setup exit: {status}\n{text}");
    assert!(text.contains("setup"), "missing in-app setup view, not offline/manual path\n{text}");
    assert!(!text.contains("offline") && !text.contains("session create") && !text.contains("serve"), "wrong startup path\n{text}");
    assert!(text.contains("\x1b[?1049l") || text.contains("\x1b[0m"), "terminal restoration sequence missing\n{text}");
}

#[test]
fn bare_with_provider_creates_and_selects_first_session() {
    let root = Root::new(); let addr = address(); let mut p = Ptty::spawn(&binary(), &root, &addr, Some("fixture-secret"));
    p.wait_for("OpenCode RK TUI");
    let until = Instant::now() + WAIT; while Instant::now() < until { if root.data().join("runtime/backend.json").is_file() { let d = descriptor(&root.data()); if sessions(&d) { break; } } thread::sleep(Duration::from_millis(50)); }
    let d = descriptor(&root.data()); assert!(sessions(&d), "bare launch did not create/select a session\n{}", p.text());
    p.send(":q\n"); assert!(p.exit().success(), "provider bare client failed\n{}", p.text());
}

#[test]
fn sequential_bare_clients_reuse_daemon_and_first_exit_keeps_it_healthy() {
    let root = Root::new(); let addr = address(); let mut first = Ptty::spawn(&binary(), &root, &addr, Some("fixture-secret")); first.wait_for("OpenCode RK TUI");
    let d1 = descriptor(&root.data()); let p1 = pid(&d1); assert!(health(&addr)); first.send(":q\n"); assert!(first.exit().success());
    assert!(health(&addr), "first client exit killed the shared daemon");
    let mut second = Ptty::spawn(&binary(), &root, &addr, Some("fixture-secret")); second.wait_for("OpenCode RK TUI"); let d2 = descriptor(&root.data());
    assert_eq!(pid(&d2), p1, "second bare client did not reuse daemon PID"); assert!(health(&addr)); second.send(":q\n"); assert!(second.exit().success());
}
