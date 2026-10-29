#![forbid(unsafe_code)]

#[cfg(unix)]
mod posix_packaging {
//! Portable packaging journeys for the real `install-oc2.sh` script.
//!
//! This contract intentionally remains strict: aliases, download mode, and the
//! release URL convention are product behavior, not test conveniences.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const BIN: &str = "oc2";
const MAX_OUTPUT: usize = 64 * 1024;
const MAX_ARCHIVE: usize = 256 * 1024;
const CHILD_TIMEOUT: Duration = Duration::from_secs(20);
const READER_GRACE: Duration = Duration::from_secs(2);
static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

struct Sandbox(PathBuf);
impl Sandbox {
    fn new(label: &str) -> Self {
        for _ in 0..64 {
            let root = std::env::temp_dir().join(format!("oc2-packaging-{label}-{}-{}", std::process::id(), NEXT_ROOT.fetch_add(1, Ordering::Relaxed)));
            match fs::create_dir(&root) {
                Ok(()) => { fs::create_dir(root.join("home")).unwrap(); fs::create_dir(root.join("tmp")).unwrap(); return Self(root); }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("create private fixture root: {e}"),
            }
        }
        panic!("unable to allocate unique fixture root")
    }
    fn path(&self) -> &Path { &self.0 }
}
impl Drop for Sandbox { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

#[derive(Debug)]
struct Captured { status: ExitStatus, stdout: Vec<u8>, stderr: Vec<u8>, timed_out: bool, truncated: bool }
impl Captured {
    fn assert_bounded(&self, what: &str) {
        assert!(!self.timed_out, "{what} exceeded {CHILD_TIMEOUT:?}");
        assert!(!self.truncated, "{what} exceeded {MAX_OUTPUT}-byte output bound");
    }
    fn stdout(&self) -> String { String::from_utf8_lossy(&self.stdout).into_owned() }
    fn stderr(&self) -> String { String::from_utf8_lossy(&self.stderr).into_owned() }
}

fn read_capped(mut reader: impl Read) -> (Vec<u8>, bool) {
    let mut kept = Vec::with_capacity(4096); let mut buf = [0u8; 4096]; let mut truncated = false;
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => { let keep = n.min(MAX_OUTPUT.saturating_sub(kept.len())); kept.extend_from_slice(&buf[..keep]); if keep != n { truncated = true; } }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    (kept, truncated)
}

fn reap(child: &mut Child) -> ExitStatus { let _ = child.kill(); child.wait().expect("reap child") }

fn run(program: &str, args: &[PathBuf], cwd: &Path, extra_env: &[(&str, PathBuf)]) -> Captured {
    let mut command = Command::new(program);
    command.args(args).current_dir(cwd).env_clear().env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (key, value) in extra_env { command.env(key, value); }
    let mut child = command.spawn().unwrap_or_else(|e| panic!("start {program}: {e}"));
    let stdout = child.stdout.take().expect("stdout pipe"); let stderr = child.stderr.take().expect("stderr pipe");
    let (tx, rx) = mpsc::sync_channel(2);
    let readers: [(&str, Box<dyn Read + Send>); 2] = [("stdout", Box::new(stdout)), ("stderr", Box::new(stderr))];
    for (name, reader) in readers {
        let tx = tx.clone(); thread::spawn(move || { let _ = tx.send((name, read_capped(reader))); });
    }
    drop(tx);
    let deadline = Instant::now() + CHILD_TIMEOUT; let mut timed_out = false;
    let status = loop { match child.try_wait().expect("poll child") {
        Some(status) => break status,
        None if Instant::now() >= deadline => { timed_out = true; break reap(&mut child); }
        None => thread::sleep(Duration::from_millis(10)),
    }};
    let collect_until = Instant::now() + READER_GRACE; let mut out = Vec::new(); let mut err = Vec::new(); let mut truncated = false;
    while Instant::now() < collect_until {
        match rx.recv_timeout(Duration::from_millis(25)) {
            Ok(("stdout", (bytes, cut))) => { out = bytes; truncated |= cut; }
            Ok(("stderr", (bytes, cut))) => { err = bytes; truncated |= cut; }
            Ok(_) => unreachable!(),
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    Captured { status, stdout: out, stderr: err, timed_out, truncated }
}

fn script() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/install-oc2.sh") }
fn platform() -> String {
    let arch = match std::env::consts::ARCH { "x86_64" => "x64", "aarch64" => "arm64", other => panic!("unsupported arch {other}") };
    match std::env::consts::OS { "linux" => format!("linux-{arch}"), "macos" => format!("macos-{arch}"), other => panic!("unsupported OS {other}") }
}
fn native_suffix() -> &'static str { if std::env::consts::OS == "linux" { "so" } else { "dylib" } }

fn archive_fixture(sandbox: &Sandbox, version: &str) -> PathBuf {
    let payload = sandbox.path().join("payload"); let native = payload.join(format!("native/lib/{}/libopentui.{}", platform(), native_suffix()));
    fs::create_dir_all(native.parent().unwrap()).unwrap();
    fs::write(payload.join(BIN), format!("#!/bin/sh\ncase \"${{1:-}}\" in --version) printf '%s\\n' 'oc2 {version}' ;; --help) printf '%s\\n' 'oc2 - fixture usage' ;; *) exit 0 ;; esac\n").as_bytes()).unwrap();
    fs::write(native, b"native fixture bytes").unwrap();
    let archive = sandbox.path().join(format!("oc2-{version}-{}.tar.gz", platform()));
    let result = run("tar", &vec![PathBuf::from("-czf"), archive.clone(), PathBuf::from("-C"), payload, PathBuf::from(BIN), PathBuf::from("native")], sandbox.path(), &[]);
    result.assert_bounded("archive fixture"); assert!(result.status.success(), "tar failed: {}", result.stderr());
    assert!(fs::metadata(&archive).unwrap().len() as usize <= MAX_ARCHIVE, "fixture archive too large"); archive
}
fn sha256(sandbox: &Sandbox, archive: &Path) -> String {
    let probe = run("sh", &[PathBuf::from("-c"), PathBuf::from("command -v sha256sum >/dev/null 2>&1")], sandbox.path(), &[]); probe.assert_bounded("checksum tool availability probe"); let program = if probe.status.success() { "sha256sum" } else { "shasum" };
    let args = if program == "sha256sum" { vec![archive.to_owned()] } else { vec![PathBuf::from("-a"), PathBuf::from("256"), archive.to_owned()] };
    let result = run(program, &args, sandbox.path(), &[]); result.assert_bounded("checksum fixture"); assert!(result.status.success(), "checksum failed: {}", result.stderr()); result.stdout().split_whitespace().next().expect("sha256 digest").to_owned()
}
fn run_installer(sandbox: &Sandbox, args: Vec<PathBuf>, install_dir: &Path, extra: &[(&str, PathBuf)]) -> Captured {
    let mut full = vec![script()]; full.extend(args); full.extend([PathBuf::from("--install-dir"), install_dir.to_owned()]);
    let mut env = vec![("HOME", sandbox.path().join("home")), ("TMPDIR", sandbox.path().join("tmp"))]; env.extend_from_slice(extra); run("sh", &full, sandbox.path(), &env)
}

#[test]
fn aliases_exist_as_same_file_and_preserve_content() {
    let s = Sandbox::new("aliases"); let archive = archive_fixture(&s, "test"); let digest = sha256(&s, &archive); let dir = s.path().join("install with spaces");
    let result = run_installer(&s, vec![PathBuf::from("--archive"), archive, PathBuf::from("--checksum"), PathBuf::from(digest), PathBuf::from("--aliases"), PathBuf::from("oc,ooo,oooo,ocrt")], &dir, &[]); result.assert_bounded("alias install"); assert!(result.status.success(), "alias install: {}", result.stderr());
    let original = fs::read(dir.join(BIN)).unwrap(); let probe = dir.join("oc"); let marker = b"alias identity probe"; let before = fs::read(&probe).unwrap(); fs::write(&probe, marker).unwrap(); assert_eq!(fs::read(dir.join(BIN)).unwrap(), marker, "alias is not the same file"); fs::write(&probe, &before).unwrap();
    assert_eq!(before, original); for alias in ["oc", "ooo", "oooo", "ocrt"] { let path = dir.join(alias); assert_eq!(fs::read(&path).unwrap(), original, "{alias} content differs"); }
}

#[test]
fn alias_collision_exits_64_preserves_old_bytes_and_installs_nothing() {
    let s = Sandbox::new("collision"); let archive = archive_fixture(&s, "test"); let digest = sha256(&s, &archive); let dir = s.path().join("install"); fs::create_dir_all(&dir).unwrap(); let old = b"old alias bytes"; fs::write(dir.join("oc"), old).unwrap();
    let result = run_installer(&s, vec![PathBuf::from("--archive"), archive, PathBuf::from("--checksum"), PathBuf::from(digest), PathBuf::from("--aliases"), PathBuf::from("oc,ooo")], &dir, &[]); result.assert_bounded("alias collision"); assert_eq!(result.status.code(), Some(64), "stderr: {}", result.stderr()); assert_eq!(fs::read(dir.join("oc")).unwrap(), old); assert!(!dir.join(BIN).exists()); assert!(!dir.join("ooo").exists());
}

#[test]
fn install_then_uninstall_removes_primary_and_aliases() {
    let s = Sandbox::new("uninstall"); let archive = archive_fixture(&s, "test"); let digest = sha256(&s, &archive); let dir = s.path().join("install"); let result = run_installer(&s, vec![PathBuf::from("--archive"), archive, PathBuf::from("--checksum"), PathBuf::from(digest), PathBuf::from("--aliases"), PathBuf::from("oc,ooo")], &dir, &[]); result.assert_bounded("install before uninstall"); assert!(result.status.success(), "install: {}", result.stderr());
    let uninstall = run_installer(&s, vec![PathBuf::from("--uninstall")], &dir, &[]); uninstall.assert_bounded("uninstall"); assert!(uninstall.status.success(), "uninstall: {}", uninstall.stderr()); for name in [BIN, "oc", "ooo"] { assert!(!dir.join(name).exists(), "{name} remains after uninstall"); }
}

#[test]
fn download_mode_requests_exact_loopback_archive_and_checksum_urls() {
    let s = Sandbox::new("download"); let version = "v-test"; let archive = archive_fixture(&s, version); let archive_bytes = fs::read(&archive).unwrap(); let digest = sha256(&s, &archive); let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap(); let addr = listener.local_addr().unwrap(); let base = format!("http://{addr}"); let expected_archive = format!("/{version}/oc2-{}.tar.gz", platform()); let expected_checksum = format!("{expected_archive}.sha256");
    let requests = Arc::new(Mutex::new(Vec::new())); let seen = Arc::clone(&requests); let expected_digest = digest.clone(); let server_archive = expected_archive.clone(); let server_checksum = expected_checksum.clone(); let server = thread::spawn(move || { listener.set_nonblocking(true).unwrap(); let deadline = Instant::now() + CHILD_TIMEOUT; while seen.lock().unwrap().len() < 2 && Instant::now() < deadline { let (mut stream, _) = match listener.accept() { Ok(v) => v, Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => { thread::sleep(Duration::from_millis(10)); continue; }, Err(e) => panic!("accept release request: {e}") }; stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap(); stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap(); let mut request = [0u8; 4096]; let n = stream.read(&mut request).unwrap(); let text = String::from_utf8_lossy(&request[..n]).to_string(); seen.lock().unwrap().push(text.clone()); let target = text.lines().next().unwrap_or_default().split_whitespace().nth(1).unwrap_or_default(); let body = if target == server_archive { archive_bytes.clone() } else if target == server_checksum { format!("{expected_digest}  oc2-{}.tar.gz\n", platform()).into_bytes() } else { b"not found".to_vec() }; let status = if target == server_archive || target == server_checksum { "200 OK" } else { "404 Not Found" }; let header = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()); stream.write_all(header.as_bytes()).unwrap(); stream.write_all(&body).unwrap(); } });
    let dir = s.path().join("download install");
    let result = run_installer(
        &s,
        vec![PathBuf::from("--version"), PathBuf::from(version)],
        &dir,
        &[("OC2_RELEASE_BASE", PathBuf::from(base))],
    );
    result.assert_bounded("download install");
    assert!(server.join().is_ok(), "release fixture server failed");
    let captured = requests.lock().unwrap();
    assert_eq!(captured.len(), 2, "expected exactly archive and checksum requests: {captured:?}");
    assert!(captured.iter().any(|r| {
        r.starts_with(&format!("GET {expected_archive} HTTP/"))
    }));
    assert!(captured.iter().any(|r| {
        r.starts_with(&format!("GET {expected_checksum} HTTP/"))
    }));
    assert!(result.status.success(), "download install: {}", result.stderr());
}

#[test]
fn executable_script_lines_contain_no_stale_opencode2_name() {
    for (line_number, line) in fs::read_to_string(script()).unwrap().lines().enumerate() { let trimmed = line.trim(); if !trimmed.starts_with('#') { assert!(!trimmed.contains("opencode2"), "line {} contains stale name: {line}", line_number + 1); } }
}

}
