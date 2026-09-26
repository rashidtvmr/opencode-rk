//! TUI-011-NATIVE-COMPILE-RED
//!
//! Verifies that the native-enabled `oc2` binary can be built with the
//! genuine libopentui dylib fixture at source SHA 7938791.
//!
//! At SHA 7938791, `crates/cli/src/tui_entry.rs:491` inside the
//! `#[cfg(feature = "native")]` function `native_page_lines` pushes a `&str`
//! literal ("Enter send ...") into a `Vec<String>` (`lines`), producing
//! E0308 mismatched-types. This prevents the native-enabled build from
//! completing even when the genuine dylib is present.
//!
//! This test stages the exact source revision into a disposable snapshot,
//! copies the verified fixture dylib (sha256 5e0265d4...) into the location
//! `crates/opentui-bridge/build.rs` expects (`native/lib/<target-triple>/`),
//! then runs a bounded `cargo build --locked -p opencode-rk-cli --bin oc2
//! --features native` with a fresh target dir and asserts process exit 0 and
//! that the built binary exists.
//!
//! The test is RED at SHA 7938791 because the type mismatch at
//! `tui_entry.rs:491` causes the build to fail, so the exit-0/binary-exists
//! assertions do not hold.
//!
//! No DYLD_LIBRARY_PATH is set for the build step (the linker resolves the
//! dylib via `cargo:rustc-link-search` emitted by build.rs). No user
//! database, HOME, or secrets are used: a fresh temp HOME and CARGO_TARGET_DIR
//! are provided. Stdout/stderr are capped at 32 KiB for failure reports.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// 300 seconds: hard child timeout with kill of the owned Child only.
const BUILD_TIMEOUT: Duration = Duration::from_secs(300);
/// Cap captured stdout/stderr for failure reports.
const CAP: usize = 32 * 1024;

static NEXT_ID: AtomicU64 = AtomicU64::new(1000);

/// Disposable temp dir; removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(prefix: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "{}-{id}-{}",
            prefix,
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap_or_else(|e| {
            panic!("create temp {prefix}: {e}");
        });
        Self(path)
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

/// Resolve the canonical repository root from `NATIVE_COMPILE_REPO_ROOT` or
/// fall back to the runtime `CARGO_MANIFEST_DIR` environment variable.
fn repo_root() -> PathBuf {
    if let Ok(p) = env::var("NATIVE_COMPILE_REPO_ROOT") {
        let canonical = fs::canonicalize(&p).unwrap_or_else(|e| {
            panic!("NATIVE_COMPILE_REPO_ROOT {p:?} not resolvable: {e}");
        });
        return canonical;
    }
    // Runtime fallback: rustc --test sets CARGO_MANIFEST_DIR from the
    // --env flag.  When compiled inside the worktree this points to the
    // workspace root.
    let p = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| {
        panic!("set NATIVE_COMPILE_REPO_ROOT or CARGO_MANIFEST_DIR");
    });
    let canonical = fs::canonicalize(&p).unwrap_or_else(|e| {
        panic!("CARGO_MANIFEST_DIR {p:?} not resolvable: {e}");
    });
    canonical
}

/// Get the host target triple by asking rustc.
fn host_triple() -> String {
    let out = Command::new("rustc")
        .arg("-vV")
        .output()
        .expect("rustc not available");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stdout}\n{stderr}");
    for line in combined.lines() {
        if let Some(rest) = line.strip_prefix("host:") {
            return rest.trim().to_string();
        }
    }
    panic!("could not determine rustc host triple");
}

/// Copy a file, creating parent directories as needed.
fn copy_file(src: &Path, dst: &Path) {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|e| {
            panic!("mkdir {parent:?}: {e}");
        });
    }
    fs::copy(src, dst).unwrap_or_else(|e| {
        panic!("copy {src:?} -> {dst:?}: {e}");
    });
}

/// Stage the source tree into a temp dir via `git archive` piped to `tar`
/// extraction. Returns nothing; the snapshot is written into `snapshot`.
/// Uses `--no-default-prefix` so paths are relative to repo root.
fn stage_snapshot(repo: &Path, snapshot: &Path) {
    let mut archive = Command::new("git")
        .arg("archive")
        .arg("--format=tar")
        .arg("HEAD")
        .current_dir(repo)
        .stdout(Stdio::piped())
        .spawn()
        .expect("git archive spawn");

    // Take stdout before calling .spawn on tar to avoid partial move.
    let archive_stdout = archive.stdout.take().expect("archive stdout");

    let mut tar = Command::new("tar")
        .arg("-x")
        .arg("-C")
        .arg(snapshot)
        .stdin(archive_stdout)
        .spawn()
        .expect("tar spawn");

    let tar_status = tar.wait().expect("tar wait");
    let git_status = archive.wait().expect("git archive wait");
    assert!(
        git_status.success(),
        "git archive failed: {git_status}"
    );
    assert!(
        tar_status.success(),
        "tar extract failed: {tar_status}"
    );
}

/// Wait for child with timeout; kill if exceeded. Returns whether we timed out.
struct WaitResult {
    timed_out: bool,
}

fn wait_or_kill(child: &mut Child, deadline: Instant) -> WaitResult {
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return WaitResult { timed_out: false },
            Ok(None) => {
                if Instant::now() >= deadline {
                    // Kill the owned child only.
                    let _ = child.kill();
                    let _ = child.wait();
                    return WaitResult { timed_out: true };
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => panic!("try_wait error: {e}"),
        }
    }
}

/// Cap a string to CAP bytes, adding a truncation marker.
fn cap_output(s: String) -> String {
    if s.len() <= CAP {
        s
    } else {
        let mut out = s[..CAP].to_string();
        out.push_str("...[truncated]");
        out
    }
}

/// Run cargo build with a hard timeout, killing only the owned Child on
/// timeout. Returns (exit status code, stdout, stderr).
fn run_cargo_build(
    cwd: &Path,
    target_dir: &Path,
    home: &Path,
) -> (i32, String, String) {
    let mut cmd = Command::new("cargo");
    cmd.env_clear()
        .env("PATH", env::var("PATH").unwrap_or_default())
        .env("HOME", home)
        .env("CARGO_TARGET_DIR", target_dir)
        .env("CARGO_BUILD_JOBS", "1")
        .env("RUST_TEST_THREADS", "1")
        // No DYLD_LIBRARY_PATH: the linker resolves via build.rs link search.
        .args([
            "build",
            "--locked",
            "-p", "opencode-rk-cli",
            "--bin", "oc2",
            "--features", "native",
        ])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("cargo build spawn");

    let deadline = Instant::now() + BUILD_TIMEOUT;
    let wait_result = wait_or_kill(&mut child, deadline);

    if wait_result.timed_out {
        // Child was killed due to timeout. Collect whatever output we can.
        // wait_with_output reads remaining stdout/stderr after the process
        // has exited.
        let out = child.wait_with_output().expect("wait_with_output after kill");
        let stdout = cap_output(String::from_utf8_lossy(&out.stdout).into_owned());
        let stderr = cap_output(String::from_utf8_lossy(&out.stderr).into_owned());
        return (-1, stdout, stderr);
    }

    // Process finished on its own; wait_with_output collects the buffered
    // stdout/stderr and the exit status.
    let out = child.wait_with_output().expect("wait_with_output");
    let stdout = cap_output(String::from_utf8_lossy(&out.stdout).into_owned());
    let stderr = cap_output(String::from_utf8_lossy(&out.stderr).into_owned());
    let code = out.status.code().unwrap_or(-1);
    (code, stdout, stderr)
}

/// The core test: stage source at 7938791, copy genuine dylib, build native,
/// assert success. Fails (RED) because tui_entry.rs:491 has a type mismatch.
#[test]
fn native_compile_succeeds_with_genuine_dylib() {
    let root = repo_root();
    let triple = host_triple();

    // Fixture dylib: the task provides an absolute, pinned path + sha256.
    // Environment override MAC_OPENTUI_FIXTURE allows a test harness to
    // point to the fixture without compile-time CARGO_MANIFEST_DIR.
    let fixture_path: PathBuf = {
        let env_override = env::var("MAC_OPENTUI_FIXTURE");
        if let Ok(p) = env_override {
            PathBuf::from(p)
        } else {
            // Default: the known preserved fixture path for this worktree.
            PathBuf::from(
                "/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/\
                 T/opencode/mac-opentui-preserved-4954312/\
                 packages/native/lib/aarch64-macos/libopentui.dylib"
            )
        }
    };

    // Stage the source tree into a disposable snapshot.
    let snapshot_dir = TempDir::new("tui011-snapshot-");
    let snapshot = snapshot_dir.path();
    stage_snapshot(&root, snapshot);

    // Copy the genuine fixture dylib into the location build.rs expects.
    let dst_dylib = snapshot.join("crates/opentui-bridge/native/lib")
        .join(&triple)
        .join("libopentui.dylib");
    copy_file(&fixture_path, &dst_dylib);

    // Fresh HOME and CARGO_TARGET_DIR: no user DB, no secrets.
    let home_dir = TempDir::new("tui011-home-");
    let target_dir = TempDir::new("tui011-target-");

    let (code, stdout, stderr) = run_cargo_build(
        snapshot,
        target_dir.path(),
        home_dir.path(),
    );

    // The build must succeed (exit 0) when the genuine dylib is present.
    // At SHA 7938791 it FAILS due to the type mismatch at tui_entry.rs:491,
    // so `code != 0` -- this is the intended RED state.
    assert_eq!(
        code, 0,
        "native-enabled cargo build must succeed with genuine dylib\n\
         exit_code: {code}\n\
         stdout:\n{stdout}\n\
         stderr:\n{stderr}"
    );

    // The built binary must exist at the expected cargo target location.
    let bin_path = target_dir.path()
        .join("debug")
        .join("oc2");
    assert!(
        bin_path.is_file(),
        "built binary must exist at {bin_path:?}"
    );
}
