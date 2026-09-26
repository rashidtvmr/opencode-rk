//! TUI-011-LINUX-REAL-ASSET-RED: Linux behavioral RED for the real installer.
//!
//! Source evidence (repo commit 2f169298):
//! - `scripts/install-oc2.sh:18`: `MAX_ARCHIVE_PAYLOAD=134217728`.
//! - `scripts/install-oc2.sh:229-246`: per-member payload probe, exit 65.
//! - `scripts/install-oc2.sh:321-323`: `$BIN` to `INSTALL_DIR/oc2`, lib to `INSTALL_DIR/../lib/libopentui.so`.
//! - `scripts/install-oc2.sh:285-301`: staged `oc2 --version` identity check.
//! Observed Linux RED receipt `nomad-1d6bf109-receipt.txt`
//! (SHA256 2a027450...adee810): source-built oc2 118773880 B +
//! libopentui.so 26583552 B = 145357432 B > 134217728 B cap; ELF RUNPATH
//! `$ORIGIN/../lib`; checksum-valid archive exits 65. Fixture expired with
//! its Nomad allocation; rebuilt archive/csum/script arrive via env only.
//! Mirrors frozen Mac `tests/e2e/native_archive_install.rs` (reference only,
//! untouched). RED-only: a missing fixture panics as blocker, never as RED.

use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Owned-child deadline (45 s) per task requirements.
const CHILD_TIMEOUT: Duration = Duration::from_secs(45);
/// Byte budget for retained child output (no unbounded output).
const MAX_OUTPUT: u64 = 8192;

static NEXT_ID: AtomicU64 = AtomicU64::new(9000);

/// Disposable dir, removed on drop; never removes a path it did not create.
struct TempDir(PathBuf);

impl TempDir {
    fn new(prefix: &str) -> Self {
        let base = env::temp_dir();
        let pid = std::process::id();
        for _ in 0..1000u64 {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!("{prefix}-{pid}-{id}"));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(ref e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("create temp {prefix}: {e}"),
            }
        }
        panic!("create temp {prefix}: exhausted retries");
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Missing fixture is a distinct blocker (panic), never a RED assertion.
fn require_file(env_name: &str, default: &str) -> PathBuf {
    let p = PathBuf::from(env::var(env_name).unwrap_or_else(|_| default.into()));
    if !p.is_file() {
        panic!("blocker: {env_name} not a file: {p:?} (set {env_name} explicitly)");
    }
    p
}

fn require_str(env_name: &str, default: &str) -> String {
    let v = env::var(env_name).unwrap_or_else(|_| default.into());
    if v.is_empty() {
        panic!("blocker: {env_name} is empty (set {env_name} explicitly)");
    }
    v
}

/// Wait for the owned child until the deadline; kill and reap only it.
fn wait_owned(child: &mut std::process::Child, deadline: Instant) -> bool {
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return false,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return true;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => panic!("try_wait: {e}"),
        }
    }
}

/// Full Linux RED: install exits 0, `bin/oc2` + `lib/libopentui.so` exist,
/// `oc2 --version` exits 0 under `env_clear`, no `LD_LIBRARY_PATH`.
#[cfg(target_os = "linux")]
#[test]
fn linux_native_archive_install_succeeds() {
    let archive = require_file("OC2_LINUX_ARCHIVE", "/tmp/oc2-linux-real/oc2-linux-x86_64.tar.gz");
    let checksum = require_str(
        "OC2_LINUX_CHECKSUM",
        "8dc056db07e4b4e1778b590831b5e984edc6ea1ea4fec1efea3a29ffcf1e3d40",
    );
    let script = require_file(
        "OC2_INSTALLER_SCRIPT",
        &format!("{}/scripts/install-oc2.sh", env::var("CARGO_MANIFEST_DIR").unwrap_or('.'.into())),
    );

    let root = TempDir::new("tui011-linux-root-");
    let install_dir = root.path().join("bin");
    let home = TempDir::new("tui011-linux-home-");
    let tmp = TempDir::new("tui011-linux-tmp-");

    let deadline = Instant::now() + CHILD_TIMEOUT;
    let mut install = Command::new("sh");
    install
        .arg(&script)
        .arg("--archive").arg(&archive)
        .arg("--checksum").arg(&checksum)
        .arg("--install-dir").arg(&install_dir)
        .env_clear()
        .env("PATH", env::var("PATH").unwrap_or_default())
        .env("HOME", home.path())
        .env("TMPDIR", tmp.path())
        .env("TERM", "dumb")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut install = install.spawn().expect("spawn install-oc2.sh");
    assert!(!wait_owned(&mut install, deadline), "install-oc2.sh timed out");
    let status = install.wait().expect("reap install-oc2.sh");
    assert!(status.success(), "install-oc2.sh must exit 0; got {status} (RED: 145357432 B > 134217728 B cap exits 65)");

    let oc2 = install_dir.join("oc2");
    assert!(oc2.is_file(), "installed binary must exist at {oc2:?}");
    let lib = root.path().join("lib").join("libopentui.so");
    assert!(lib.is_file(), "native lib must exist at {lib:?} (sibling of bin/)");

    let vdeadline = Instant::now() + CHILD_TIMEOUT;
    let mut version = Command::new(&oc2);
    version
        .arg("--version")
        .env_clear()
        .env("PATH", env::var("PATH").unwrap_or_default())
        .env("HOME", home.path())
        .env("TMPDIR", tmp.path())
        .env("TERM", "dumb")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut version = version.spawn().expect("spawn oc2 --version");
    assert!(!wait_owned(&mut version, vdeadline), "oc2 --version timed out");
    let vstatus = version.wait().expect("reap oc2 --version");
    let mut out = Vec::new();
    if let Some(o) = version.stdout.as_mut() {
        let _ = o.take(MAX_OUTPUT).read_to_end(&mut out);
    }
    assert!(vstatus.success(), "bin/oc2 --version must exit 0 with no LD_LIBRARY_PATH; got {vstatus}");
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("oc2"), "oc2 --version must identify as oc2; got {text:?}");
}
