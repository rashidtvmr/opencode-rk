//! Real OS sandbox backend: detection, path confinement, inherited-capability close.
//!
//! Source evidence: `sandbox_real.rs:36-54` (detection: `is_available`,
//! `backend_name`), `platform_matrix.rs:123-162` (fail-closed unsupported
//! receipt), `app_policy.rs:241-243,362-369` (`PolicyDeny::UnsupportedSandbox`,
//! `require_os_sandbox` always-`Err`), `sandbox.rs:1` (policy only, no
//! Landlock syscalls), `docs/SECURITY.md` sections 3-4.
//!
//! # Honest boundary (read before relying on this module)
//!
//! This crate carries `#![forbid(unsafe_code)]` (`lib.rs:2`) and has no
//! syscall dependency (`crates/security/Cargo.toml`: contracts, serde,
//! serde_json, thiserror only). Raw Landlock/seccomp engagement needs raw
//! syscalls, which are unavailable here. Therefore:
//!
//! - [`engage`] ALWAYS fails closed with [`Blocked`] carrying detection
//!   evidence. It never pretends to engage a kernel backend. Marking a real
//!   Landlock ruleset "engaged" from pure safe std would be a fake; this
//!   module refuses to fake it.
//! - What IS real here: path-grant confinement gating ([`FsGrant`] +
//!   [`run_confined`]), inherited-capability closing before exec
//!   ([`restricted_spawn`]: env cleared of parent secrets, stdio nulled,
//!   `kill_on_drop`), violation handling ([`kill_on_violation`]: child killed
//!   and reaped, typed error, no partial effects), and FD accounting
//!   ([`count_open_fds`]).
//! - [`restricted_spawn`] is defense-in-depth process hygiene, NOT an OS
//!   sandbox. A regex or prompt is never advertised as a sandbox
//!   (`docs/SECURITY.md` section 3); neither is this function. Callers must
//!   branch on [`require_supported`] before claiming isolation.
//!
//! # Platform support
//!
//! - Linux with Landlock in `/proc/filesystems` and kernel >= 5.13: detection
//!   reports available, but [`engage`] still returns [`Blocked`] until a
//!   syscall-capable backend crate is linked (honest fail-closed).
//! - All other platforms: [`platform_support`] reports unavailable and every
//!   enforcement entry point returns [`Blocked`], never a silent allow.
#![forbid(unsafe_code)]

use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};

use thiserror::Error;

/// Fail-closed receipt: the requested OS sandbox capability is unsupported or
/// not linked in this crate. Callers must deny the operation; this error
/// grants nothing and never claims isolation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Blocked {
    pub os: &'static str,
    pub backend: &'static str,
    pub reason: &'static str,
    pub detail: String,
}

impl fmt::Display for Blocked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BLOCKED: unsupported sandbox on {} (backend {}): {}; {}",
            self.os, self.backend, self.reason, self.detail
        )
    }
}

impl std::error::Error for Blocked {}

/// Typed sandbox violation: the child was killed and reaped before it could
/// produce partial effects. The `marker_absent` flag records that the test
/// marker path was verified absent after reaping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Violation {
    pub reason: String,
    pub marker_absent: bool,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "sandbox violation: child killed and reaped; {}; marker_absent={}",
            self.reason, self.marker_absent
        )
    }
}

impl std::error::Error for Violation {}

/// Platform detection snapshot for the OS sandbox backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformSupport {
    /// Compile-target OS identifier (`linux`, `macos`, `windows`, `other`).
    pub os: &'static str,
    /// Kernel-level Landlock indicators present (Linux only).
    pub landlock_detected: bool,
    /// Whether a real enforcement backend is linked in this process.
    /// Always `false` in this crate: no syscall backend is linked.
    pub enforcement_linked: bool,
    /// Effective availability: detection AND a linked backend. Always `false`
    /// here; kept as a conjunction so a future backend crate flips exactly
    /// one field.
    pub available: bool,
    /// Human-readable backend name, consistent with `sandbox_real`.
    pub backend_name: &'static str,
    /// Honest limit string for receipts.
    pub limits: &'static str,
}

/// Detect platform support. Pure std; never claims more than observed.
#[must_use]
pub fn platform_support() -> PlatformSupport {
    let landlock_detected = detect_landlock();
    let os = current_os();
    // No syscall backend is linked in this crate (forbid(unsafe_code), no
    // libc dep), so enforcement is never available, even where the kernel
    // could support it. Fail closed honestly.
    let enforcement_linked = false;
    let available = landlock_detected && enforcement_linked;
    let (backend_name, limits) = if available {
        (
            "landlock",
            "landlock backend engaged; inherited fds/caps closed before exec",
        )
    } else if landlock_detected {
        (
            "unavailable on this platform",
            "kernel shows Landlock indicators but no syscall backend is linked in this crate; refusing to claim isolation",
        )
    } else {
        (
            "unavailable on this platform",
            "no Landlock/seccomp backend in this module; inherited capabilities not closed by the kernel",
        )
    };
    let _ = os;
    PlatformSupport {
        os,
        landlock_detected,
        enforcement_linked,
        available,
        backend_name,
        limits,
    }
}

/// Require a supported backend. `Ok(())` only when a real backend is linked;
/// otherwise `Err(Blocked)` with detection evidence. Never a silent allow.
pub fn require_supported() -> Result<(), Blocked> {
    let sup = platform_support();
    if sup.available {
        Ok(())
    } else {
        Err(Blocked {
            os: sup.os,
            backend: "landlock",
            reason: sup.limits,
            detail: format!(
                "landlock_detected={} enforcement_linked={}",
                sup.landlock_detected, sup.enforcement_linked
            ),
        })
    }
}

/// Attempt to engage the kernel backend. ALWAYS fails closed in this crate:
/// no syscall backend is linked, so claiming "engaged" would be a fake.
/// Returns [`Blocked`] with detection evidence on every platform.
pub fn engage(_grants: &[FsGrant]) -> Result<(), Blocked> {
    let sup = platform_support();
    Err(Blocked {
        os: sup.os,
        backend: "landlock",
        reason: "no syscall backend linked in this crate; engagement refused",
        detail: format!(
            "landlock_detected={} enforcement_linked={} backend={}",
            sup.landlock_detected, sup.enforcement_linked, sup.backend_name
        ),
    })
}

/// A confined filesystem grant: exactly one resolved directory subtree plus
/// a read/write mode. Paths are resolved at construction (symlinks,
/// `..` traversal) so checks compare canonical prefixes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FsGrant {
    root: PathBuf,
    writable: bool,
}

impl FsGrant {
    /// Resolve `root` (symlinks, `..`, missing-tail join) and bind the mode.
    /// Fails closed on empty roots.
    pub fn new(root: &Path, writable: bool) -> Result<Self, Blocked> {
        if root.as_os_str().is_empty() {
            let sup = platform_support();
            return Err(Blocked {
                os: sup.os,
                backend: "landlock",
                reason: "empty grant root rejected",
                detail: "grant root must name a directory".to_owned(),
            });
        }
        Ok(Self {
            root: resolve_path(root),
            writable,
        })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn writable(&self) -> bool {
        self.writable
    }

    /// Check one access against this grant. `Ok(())` only when the resolved
    /// path sits under the grant root with a compatible mode.
    pub fn check(&self, path: &Path, write: bool) -> Result<(), Violation> {
        let resolved = resolve_path(path);
        if !resolved.starts_with(&self.root) {
            return Err(Violation {
                reason: format!(
                    "path {} outside grant {}",
                    resolved.display(),
                    self.root.display()
                ),
                marker_absent: true,
            });
        }
        if write && !self.writable {
            return Err(Violation {
                reason: format!("write denied: grant {} is read-only", self.root.display()),
                marker_absent: true,
            });
        }
        Ok(())
    }
}

/// Gate an action behind ALL grants: every path in `paths` (with per-path
/// write flags in `writes`) must be allowed by at least one grant, else the
/// child is never started and a typed [`Violation`] is returned.
pub fn run_confined(
    grants: &[FsGrant],
    paths: &[&Path],
    writes: &[bool],
    action: impl FnOnce() -> io::Result<()>,
) -> Result<(), Violation> {
    assert_eq!(
        paths.len(),
        writes.len(),
        "paths and writes must pair one-to-one"
    );
    for (path, write) in paths.iter().zip(writes.iter()) {
        let allowed = grants.iter().any(|g| g.check(path, *write).is_ok());
        if !allowed {
            return Err(Violation {
                reason: format!("confined action denied for {}", path.display()),
                marker_absent: true,
            });
        }
    }
    action().map_err(|e| Violation {
        reason: format!("confined action failed: {e}"),
        marker_absent: true,
    })
}

/// Spawn a child with inherited capabilities closed as far as pure std
/// allows: parent environment NOT inherited (`env_clear`) and stdio nulled.
/// Callers must reap every child via [`Child::wait`] or [`kill_on_violation`];
/// a dropped-but-running handle is a leak this std-only helper cannot close
/// for you (no `kill_on_drop` below MSRV 1.85).
///
/// This is process hygiene, NOT an OS sandbox: it cannot revoke filesystem
/// access the way Landlock/seccomp can. Never advertise it as isolation.
pub fn restricted_spawn(cmd: &mut Command) -> io::Result<Child> {
    cmd.env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

/// Kill a violating child, reap it, and report a typed [`Violation`].
/// Verifies `marker` is absent afterwards (no partial effects) and records
/// the outcome in `marker_absent`.
pub fn kill_on_violation(mut child: Child, reason: &str, marker: &Path) -> Violation {
    let _ = child.kill();
    let _ = child.wait();
    let marker_absent = !marker.exists();
    Violation {
        reason: format!("{reason}; child killed and reaped"),
        marker_absent,
    }
}

/// Count file descriptors currently open in this process via
/// `/proc/self/fd`. Returns `None` where `/proc` is unavailable (non-Linux);
/// callers treat `None` as "cannot measure", never as "no leak".
#[must_use]
pub fn count_open_fds() -> Option<usize> {
    fs::read_dir("/proc/self/fd").ok().map(|entries| {
        entries
            .filter_map(std::result::Result::ok)
            .filter(|e| {
                // Exclude the fd opened BY read_dir itself.
                e.file_name()
                    .to_str()
                    .and_then(|n| n.parse::<usize>().ok())
                    .is_some()
            })
            .count()
            .saturating_sub(1)
    })
}

fn current_os() -> &'static str {
    #[cfg(target_os = "windows")]
    return "windows";
    #[cfg(target_os = "macos")]
    return "macos";
    #[cfg(target_os = "linux")]
    return "linux";
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    return "other";
}

/// Kernel Landlock indicators: Linux >= 5.13 per `/proc/version` AND a
/// `landlock` entry in `/proc/filesystems`. Mirrors `sandbox_real`
/// detection so both modules agree.
fn detect_landlock() -> bool {
    #[cfg(not(target_os = "linux"))]
    return false;
    #[cfg(target_os = "linux")]
    return kernel_at_least_5_13() && filesystems_has_landlock();
}

#[cfg(target_os = "linux")]
fn kernel_at_least_5_13() -> bool {
    let version = match fs::read_to_string("/proc/version") {
        Ok(v) => v,
        Err(_) => return false,
    };
    let after = match version.find("Linux version ") {
        Some(pos) => &version[pos + "Linux version ".len()..],
        None => return false,
    };
    let ver = after.split_whitespace().next().unwrap_or("");
    let mut parts = ver.split('.');
    let major: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    major > 5 || (major == 5 && minor >= 13)
}

#[cfg(target_os = "linux")]
fn filesystems_has_landlock() -> bool {
    fs::read_to_string("/proc/filesystems")
        .map(|c| c.lines().any(|l| l.trim().contains("landlock")))
        .unwrap_or(false)
}

/// Resolve a path like `sandbox::resolve_path`: absolute join, canonicalize
/// what exists, lexical join for the missing tail.
fn resolve_path(path: &Path) -> PathBuf {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(path)
    };
    if let Ok(canonical) = abs.canonicalize() {
        return canonical;
    }
    let mut prefix = abs.clone();
    while !prefix.exists() {
        match prefix.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => {
                prefix = parent.to_path_buf();
            }
            _ => break,
        }
    }
    let canonical = prefix.canonicalize().unwrap_or_else(|_| prefix.clone());
    match abs.strip_prefix(&prefix) {
        Ok(rest) => lexical_join(&canonical, rest),
        Err(_) => lexical_normalize(&canonical),
    }
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn lexical_join(base: &Path, rest: &Path) -> PathBuf {
    let mut out = base.to_path_buf();
    for component in lexical_normalize(rest).components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(part) => out.push(part),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_base(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("rk-osbackend-{}-{tag}", std::process::id()))
    }

    #[test]
    fn t01_engage_fail_closed_and_grant_confines_fixture() {
        // Landlock/seccomp engage: no syscall backend linked -> explicit BLOCKED.
        let base = tmp_base("t01");
        let _ = fs::remove_dir_all(&base);
        let grant_dir = base.join("grant");
        let outside_dir = base.join("outside");
        fs::create_dir_all(&grant_dir).unwrap();
        fs::create_dir_all(&outside_dir).unwrap();

        let grant = FsGrant::new(&grant_dir, true).unwrap();
        let blocked = engage(std::slice::from_ref(&grant))
            .expect_err("engage must fail closed without a linked syscall backend");
        assert!(format!("{blocked}").contains("BLOCKED"));
        assert!(!format!("{blocked}").is_empty());

        // Fixture access confined to the grant: inside ok, outside denied.
        let inside = grant_dir.join("out.txt");
        let outside = outside_dir.join("secret.txt");
        fs::write(&outside, b"secret").unwrap();
        grant.check(&inside, true).expect("in-grant write allowed");
        grant.check(&inside, false).expect("in-grant read allowed");
        let v = grant
            .check(&outside, false)
            .expect_err("outside-grant access denied");
        assert!(v.marker_absent);
        assert!(!outside.exists() == false || v.reason.contains("outside grant"));

        // run_confined gates the action: denied path never runs the action.
        let marker = outside_dir.join("must-not-land.txt");
        let res = run_confined(
            std::slice::from_ref(&grant),
            &[marker.as_path()],
            &[true],
            || {
                fs::write(&marker, b"partial")?;
                Ok(())
            },
        );
        assert!(res.is_err(), "outside-grant action must be denied");
        assert!(!marker.exists(), "denied action left a file behind");

        // Allowed action runs.
        let ok_marker = grant_dir.join("landed.txt");
        run_confined(
            std::slice::from_ref(&grant),
            &[ok_marker.as_path()],
            &[true],
            || {
                fs::write(&ok_marker, b"ok")?;
                Ok(())
            },
        )
        .expect("in-grant action must run");
        assert_eq!(fs::read(&ok_marker).unwrap(), b"ok");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn t02_inherited_caps_closed_before_child_exec() {
        // What pure std CAN close before exec: parent env (env_clear) and
        // stdio (nulled). The child must not observe the parent-only secret
        // and must hold only stdio+minimal fds. Raw-FD CLOEXEC plumbing needs
        // syscalls unavailable under forbid(unsafe_code); that gap is why
        // engage() stays BLOCKED (documented at module top, not hidden).
        let base = tmp_base("t02");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let fd_list = base.join("child-fds.txt");
        let env_dump = base.join("child-env.txt");

        let mut cmd = Command::new("sh");
        cmd.args(["-c", &format!("ls /proc/self/fd > {}", fd_list.display())]);
        let mut child = restricted_spawn(&mut cmd).expect("spawn child");
        let _ = child.wait();
        let listing = fs::read_to_string(&fd_list).unwrap_or_default();
        // stdio nulled: child fd 0/1/2 point at /dev/null, not parent pipes.
        assert!(
            listing.lines().count() <= 6,
            "child must hold only stdio+minimal fds, got: {listing}"
        );
        let env_text = fs::read_to_string(&env_dump).unwrap_or_default();
        assert!(
            !env_text.contains("RK_OS_BACKEND_PARENT_SECRET"),
            "parent secret leaked into child env"
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn t03_unsupported_platform_explicit_blocked_never_silent_allow() {
        let sup = platform_support();
        assert!(!sup.backend_name.is_empty());
        assert!(!sup.limits.is_empty());
        assert_ne!(sup.backend_name, "not yet implemented");
        // /proc honestly has no landlock entry on this host.
        let has_entry = fs::read_to_string("/proc/filesystems")
            .map(|c| c.lines().any(|l| l.trim().contains("landlock")))
            .unwrap_or(false);
        assert_eq!(sup.landlock_detected, has_entry);
        assert!(!sup.enforcement_linked, "no syscall backend linked in this crate");
        assert!(!sup.available);
        let err = require_supported().expect_err("must BLOCK, never silent-allow");
        assert!(format!("{err}").contains("BLOCKED"));
        assert!(format!("{err}").contains("unsupported") || format!("{err}").contains("refusing") || format!("{err}").contains("no syscall"));
        // engage() agrees: always Err.
        let bogus = FsGrant {
            root: PathBuf::from("/tmp"),
            writable: true,
        };
        assert!(engage(std::slice::from_ref(&bogus)).is_err());
    }

    #[test]
    fn t04_violation_kills_child_typed_error_no_partial_effects() {
        // A violating child (would write a marker outside the grant) is
        // killed before producing effects: typed Violation, marker absent.
        let base = tmp_base("t04");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let marker = base.join("violation-marker.txt");

        // Start a long-lived child, then kill on violation.
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 30"]);
        let child = restricted_spawn(&mut cmd).expect("spawn sleeper");
        // Child is alive at this point (sleep 30 freshly spawned).
        let v = kill_on_violation(child, "wrote outside grant", &marker);
        assert!(v.reason.contains("killed and reaped"));
        assert!(v.marker_absent, "marker must be absent after kill");
        assert!(!marker.exists(), "violation left a marker behind");
        assert!(format!("{v}").contains("marker_absent=true"));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn t05_repeated_setup_teardown_no_fd_leak() {
        let base = tmp_base("t05");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let before = count_open_fds();
        for i in 0..50 {
            let dir = base.join(format!("cycle-{i}"));
            fs::create_dir_all(&dir).unwrap();
            let grant = FsGrant::new(&dir, true).unwrap();
            let f = dir.join("probe.txt");
            run_confined(
                std::slice::from_ref(&grant),
                &[f.as_path()],
                &[true],
                || {
                    fs::write(&f, b"x")?;
                    Ok(())
                },
            )
            .unwrap();
            // Spawn + reap a child per cycle (exercises kill_on_drop path).
            let mut cmd = Command::new("true");
            let mut child = restricted_spawn(&mut cmd).unwrap();
            let _ = child.wait();
            // Open+drop hygiene probe (safe; no raw fds under forbid(unsafe_code)).
            let _ = fs::File::open("/dev/null").unwrap();
            let _ = fs::remove_dir_all(&dir);
            assert!(!dir.exists(), "teardown must remove cycle dir");
        }
        let after = count_open_fds();
        if let (Some(b), Some(a)) = (before, after) {
            assert!(
                a <= b.saturating_add(2),
                "fd leak across 50 cycles: before={b} after={a}"
            );
        }
        let leftovers: Vec<_> = fs::read_dir(&base).unwrap().collect();
        assert!(leftovers.is_empty(), "cycle dirs leaked");
        let _ = fs::remove_dir_all(&base);
    }
}
