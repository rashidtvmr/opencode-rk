//! TUI-011-MAC-LOADER-RED
//!
//! Behavioral RED test for the installed-layout macOS loader contract.
//!
//! Source evidence (repo commit 55a4292):
//! - `crates/cli/Cargo.toml:9-18`: `[[bin]] oc2`; the CLI crate has NO build.rs.
//! - `crates/opentui-bridge/build.rs:11-21`: emits only
//!   `cargo:rustc-link-search=native=<libdir>`; it does NOT emit any
//!   `cargo:rustc-link-arg=-rpath` and therefore the linked binary carries no
//!   `LC_RPATH`.
//! - `crates/opentui-bridge/Cargo.toml:14`: `crate-type = ["rlib"]`; build-script
//!   link attributes are not transitive, so the `oc2` binary itself has no rpath
//!   to resolve `@rpath/libopentui.dylib`.
//!
//! Contract under test: an installed arm64 `oc2` placed in `bin/`, with its
//! genuine `libopentui.dylib` placed in the sibling `../lib/` directory (the
//! layout `scripts/install-oc2.sh:309` installs into), MUST load the dylib at
//! runtime WITHOUT any `DYLD_LIBRARY_PATH` or `DYLD_FALLBACK_LIBRARY_PATH`
//! set. A correct build injects an `LC_RPATH` (e.g. `@loader_path/../lib`) so
//! dyld can resolve `@rpath/libopentui.dylib` from the sibling lib dir.
//!
//! RED state: the prebuilt `oc2` at SHA 55a4292 has ZERO `LC_RPATH` entries
//! (verified via `otool -l`), so spawning `bin/oc2 --version` with
//! `env_clear` and only HOME/PATH/TERM set (NO DYLD_*) exits 134 with
//! `dyld: Library not loaded: @rpath/libopentui.dylib ... Reason: no
//! LC_RPATH's found`. The assertion `status.success()` therefore fails today.
//!
//! This lane is RED-only (test author, not implementer): it never edits product
//! code, frozen tests, or controller files. It is intentionally NOT marked
//! completed — the fix (adding an rpath in the CLI build path) is a separate
//! lane.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

/// Hard child timeout: the dyld failure is immediate (<1s) but bound anyway.
const CHILD_TIMEOUT: Duration = Duration::from_secs(5);

static NEXT_ID: AtomicU64 = AtomicU64::new(3000);

/// Disposable temp dir; removed on drop (owned fixture cleanup only).
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

/// Resolve a prerequisite path: env var override first, then the task-pinned
/// default. Requirement: must resolve to an existing, non-empty file.
fn require_prereq(env_name: &str, default: &str) -> PathBuf {
    let value = env::var(env_name).unwrap_or_else(|_| default.to_string());
    let path = PathBuf::from(&value);
    if !path.is_file() {
        panic!(
            "prerequisite {env_name} not a file: {path:?} (set {env_name} or provide the fixture)"
        );
    }
    path
}

/// The single behavioral RED test.
///
/// Stages owned copies of the prebuilt `oc2` and the genuine dylib fixture into
/// a disposable `bin/` + sibling `lib/` layout (NO symlinks to the user
/// fixtures), spawns `bin/oc2 --version` with `env_clear` and ONLY
/// HOME/PATH/TERM exported (NO DYLD_*), and asserts the child exits
/// successfully. Today the binary has no LC_RPATH, so dyld cannot resolve
/// `@rpath/libopentui.dylib` from `../lib` and the process exits 134 — the
/// assertion fails, which is the intended RED state.
#[test]
fn installed_layout_loader_finds_dylib_without_dyld_env() {
    let oc2_src = require_prereq(
        "OC2_NATIVE_BINARY",
        "/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/impl-tui011-native-compile/target/debug/oc2",
    );
    let dylib_src = require_prereq(
        "MAC_OPENTUI_FIXTURE",
        "/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/mac-opentui-preserved-4954312/packages/native/lib/aarch64-macos/libopentui.dylib",
    );

    // Disposable installed layout: bin/oc2 + lib/libopentui.dylib (sibling).
    let root = TempDir::new("tui011-mac-loader-");
    let bin_dir = root.path().join("bin");
    let lib_dir = root.path().join("lib");
    let oc2_dst = bin_dir.join("oc2");
    let dylib_dst = lib_dir.join("libopentui.dylib");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&lib_dir).unwrap();
    // Fresh copies, no symlinks. Make the binary executable.
    fs::copy(&oc2_src, &oc2_dst).expect("copy oc2 into bin/");
    fs::copy(&dylib_src, &dylib_dst).expect("copy dylib into lib/");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&oc2_dst).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&oc2_dst, perms).unwrap();
    }

    // Spawn with env_clear; restore ONLY HOME/PATH/TERM. NO DYLD_*.
    let home = TempDir::new("tui011-mac-loader-home-");
    let mut child = Command::new(&oc2_dst)
        .arg("--version")
        .env_clear()
        .env("HOME", home.path())
        .env("PATH", env::var("PATH").unwrap_or_default())
        .env("TERM", "dumb")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn oc2");

    // Bounded poll: try_wait <= 5s, kill+wait only the owned child on timeout.
    let deadline = Instant::now() + CHILD_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Assertion surface: a conforming loader must exit successfully.
                // A conforming binary (with LC_RPATH) exits 0; the current RED
                // binary aborts via dyld SIGABRT (signal 6, exit 134) because it
                // cannot resolve @rpath/libopentui.dylib without DYLD_*.
                if !status.success() {
                    #[cfg(unix)]
                    let (code, sig) = (
                        status.code().unwrap_or(-1),
                        status.signal().unwrap_or(0),
                    );
                    #[cfg(not(unix))]
                    let (code, sig) = (status.code().unwrap_or(-1), 0);
                    panic!(
                        "installed-layout oc2 must load sibling lib/libopentui.dylib without DYLD_* env; \
                         oc2 failed (dyld no LC_RPATH's found expected at SHA 55a4292); \
                         exit_code={code} signal={sig} (134 = 128+SIGABRT)"
                    );
                }
                return;
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => panic!("try_wait error: {e}"),
        }
    }

    // Timeout path: kill and reap the owned child only.
    let _ = child.kill();
    let _ = child.wait();
    panic!(
        "installed-layout oc2 did not exit within {}s",
        CHILD_TIMEOUT.as_secs()
    );
}
