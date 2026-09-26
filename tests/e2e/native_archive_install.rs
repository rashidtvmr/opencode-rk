//! TUI-011-INSTALLER-REAL-ASSET-RED
//!
//! Behavioral RED test for the real native archive installer contract.
//!
//! Source evidence (repo commit 9cf998a):
//! - `scripts/install-oc2.sh:16`: `MAX_ARCHIVE_PAYLOAD=1048576` (1 MB hard cap)
//! - `scripts/install-oc2.sh:224-246`: per-member payload probe; rejects any
//!   regular file exceeding `MAX_ARCHIVE_PAYLOAD`, exiting 65.
//! - `scripts/install-oc2.sh:309-310`: installs `$BIN` to `INSTALL_DIR/$BIN`
//!   and the native lib to `INSTALL_DIR/../lib/$NATIVE_NAME`.
//! - `scripts/install-oc2.sh:369`: on success prints `identity_out` (from
//!   `--version`) and exits 0.
//!
//! The real archive at SHA 9cf998a contains:
//!   oc2                          45,447,176 bytes
//!   native/lib/macos-arm64/libopentui.dylib  26,367,408 bytes
//!   total expanded payload      71,814,584 bytes
//!
//! RED state: the real `oc2` binary alone (45 MB) exceeds the 1 MB
//! `MAX_ARCHIVE_PAYLOAD`, so the script's size probe at line 237-239 exits 65
//! before any file is written. The install never happens. This test asserts the
//! desired GREEN contract (exit 0, binary installed, `--version` works without
//! `DYLD_*`, sibling `lib/libopentui.dylib` exists), which fails today.
//!
//! This lane is RED-only (test author, not implementer): it never edits the
//! installer script, frozen tests, or controller files. It is intentionally
//! NOT marked completed.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Hard child timeout: 30 seconds for the installer + version check combined.
const CHILD_TIMEOUT: Duration = Duration::from_secs(30);

static NEXT_ID: AtomicU64 = AtomicU64::new(8000);

/// Disposable temp dir; removed on drop (owned fixture cleanup only).
///
/// Creates a unique directory by appending a counter until creation succeeds
/// (retry on AlreadyExists). Never deletes a pre-existing path we did not
/// create; remove_dir_all is a no-op if the directory was missing at creation
/// or already gone by teardown.
struct TempDir(PathBuf);

impl TempDir {
    fn new(prefix: &str) -> Self {
        let base = env::temp_dir();
        let pid = std::process::id();
        for attempt in 0..1000u64 {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!("{prefix}-{pid}-{id}"));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(ref e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if attempt == 999 {
                        panic!("create temp {prefix}: exhausted retries (AlreadyExists)");
                    }
                    continue;
                }
                Err(e) => panic!("create temp {prefix}: {e}"),
            }
        }
        unreachable!("TempDir::new retry loop exited without returning");
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

/// Resolve a prerequisite env var (file path) with a default. Requires the
/// path to be an existing file. Missing prerequisites panic rather than
/// asserting RED on a missing fixture — per task requirements.
fn require_prereq_file(env_name: &str, default: &str) -> PathBuf {
    let value = env::var(env_name).unwrap_or_else(|_| default.to_string());
    let path = PathBuf::from(&value);
    if !path.is_file() {
        panic!(
            "prerequisite {env_name} not a file: {path:?} (set {env_name} explicitly)"
        );
    }
    path
}

/// Resolve a prerequisite env var (string value) with a default. The value
/// must be non-empty. Used for checksum/hash values.
fn require_prereq_str(env_name: &str, default: &str) -> String {
    let value = env::var(env_name).unwrap_or_else(|_| default.to_string());
    if value.is_empty() {
        panic!(
            "prerequisite {env_name} is empty (set {env_name} explicitly)"
        );
    }
    value
}

/// Wait for the owned Child with a hard deadline. On timeout, kill and reap
/// the owned child only (no group kills, no unbounded pipes). stdout/stderr
/// are Stdio::null per task requirements (exit status is enough).
fn wait_or_kill(child: &mut ChildGuard, deadline: Instant) -> bool {
    loop {
        match child.child.try_wait() {
            Ok(Some(_)) => return false,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.child.kill();
                    let _ = child.child.wait();
                    return true;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => panic!("try_wait error: {e}"),
        }
    }
}

/// Guard wrapper so the Child is always reaped (no zombies) on scope exit.
struct ChildGuard {
    child: std::process::Child,
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.child.wait();
    }
}

/// The full RED test.
///
/// Steps:
/// 1. Resolve the real archive, checksum, and installer-script fixtures.
/// 2. Create a fresh disposable install root (no user data, no symlinks outside).
/// 3. Run `sh scripts/install-oc2.sh --archive ... --checksum ... --install-dir <root>/bin`
///    with `env_clear` keeping ONLY PATH/HOME/TMPDIR/TERM (no DYLD_*).
/// 4. Assert the installer exits 0 (currently exits 65 — size probe rejects the
///    45 MB binary against the 1 MB MAX_ARCHIVE_PAYLOAD).
/// 5. Assert `bin/oc2 --version` exits 0 with no DYLD_* env (binary must resolve
///    `../lib/libopentui.dylib` via its LC_RPATH `@loader_path/../lib`).
/// 6. Assert the sibling `../lib/libopentui.dylib` exists in the install root.
#[test]
fn native_archive_install_succeeds() {
    let archive = require_prereq_file(
        "OC2_REAL_ARCHIVE",
        "/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/real-native-install-9cf998a/oc2-macos-arm64.tar.gz",
    );
    let checksum_str = require_prereq_str(
        "OC2_REAL_CHECKSUM",
        "4d7a0ded5f3e0610b7cf8eb8b168404225f11827d81be7c9d2ce614379989131",
    );
    let script = require_prereq_file(
        "OC2_INSTALLER_SCRIPT",
        "/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/red-tui011-real-installer/scripts/install-oc2.sh",
    );

    // Read the installer script path (may be relative to the repo root).
    let script_path = if Path::new(&script).is_absolute() {
        script.clone()
    } else {
        // Resolve relative to the worktree root (CARGO_MANIFEST_DIR or cwd).
        let root = env::var("CARGO_MANIFEST_DIR")
            .unwrap_or_else(|_| env::var("PWD").unwrap_or_else(|_| ".".to_string()));
        PathBuf::from(&root).join(&script)
    };

    // Disposable install root: <fresh>/bin will be --install-dir, <fresh>/lib
    // is the sibling the binary's LC_RPATH @loader_path/../lib resolves to.
    let install_root = TempDir::new("tui011-install-root-");
    let install_dir = install_root.path().join("bin");

    // Fresh disposable HOME and TMPDIR so no user DB / secrets leak in.
    let home_dir = TempDir::new("tui011-home-");
    let tmp_dir = TempDir::new("tui011-tmp-");

    // --- Run the installer ---
    let deadline = Instant::now() + CHILD_TIMEOUT;
    let mut guard = ChildGuard {
        child: Command::new("sh")
            .arg(&script_path)
            .arg("--archive")
            .arg(&archive)
            .arg("--checksum")
            .arg(checksum_str)
            .arg("--install-dir")
            .arg(&install_dir)
            .env_clear()
            .env("PATH", env::var("PATH").unwrap_or_default())
            .env("HOME", home_dir.path())
            .env("TMPDIR", tmp_dir.path())
            .env("TERM", "dumb")
            // Explicitly NO DYLD_* env vars.
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn install-oc2.sh"),
    };

    let timed_out = wait_or_kill(&mut guard, deadline);
    assert!(
        !timed_out,
        "install-oc2.sh did not exit within {}s",
        CHILD_TIMEOUT.as_secs()
    );

    let status = guard.child.wait().expect("reap installer");

    // The installer must succeed (exit 0).
    assert!(
        status.success(),
        "install-oc2.sh must exit 0 (success); got status {status}; \
         current RED: MAX_ARCHIVE_PAYLOAD=1048576 rejects real 45MB binary (exit 65 at size probe)"
    );

    // --- Assert installed binary exists ---
    let oc2_bin = install_dir.join("oc2");
    assert!(
        oc2_bin.is_file(),
        "installed binary must exist at {oc2_bin:?}"
    );

    // --- Assert ../lib/libopentui.dylib exists (sibling to bin/) ---
    let lib_dir = install_root.path().join("lib");
    let dylib = lib_dir.join("libopentui.dylib");
    assert!(
        dylib.is_file(),
        "installed native library must exist at {dylib:?} (sibling to bin/)"
    );

    // --- Run `oc2 --version` with env_clear + NO DYLD_* ---
    let version_deadline = Instant::now() + CHILD_TIMEOUT;
    let mut vguard = ChildGuard {
        child: Command::new(&oc2_bin)
            .arg("--version")
            .env_clear()
            .env("PATH", env::var("PATH").unwrap_or_default())
            .env("HOME", home_dir.path())
            .env("TMPDIR", tmp_dir.path())
            .env("TERM", "dumb")
            // NO DYLD_* — the binary must resolve the dylib via LC_RPATH.
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn oc2 --version"),
    };

    let v_timed_out = wait_or_kill(&mut vguard, version_deadline);
    // Suppress unused variable warning: tmp_dir is used for TMPDIR env.
    let _ = &tmp_dir;
    assert!(
        !v_timed_out,
        "oc2 --version did not exit within {}s",
        CHILD_TIMEOUT.as_secs()
    );

    let v_status = vguard.child.wait().expect("reap oc2 --version");
    assert!(
        v_status.success(),
        "bin/oc2 --version must exit 0 with no DYLD_* env; got status {v_status}; \
         the binary relies on LC_RPATH @loader_path/../lib to find libopentui.dylib"
    );
}
