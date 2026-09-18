//! Platform capability matrix types (PAR-006).
//!
//! Types and policy declarations ONLY. This module is NOT an OS sandbox and
//! claims no isolation. There is no Landlock/Seatbelt/Job-Object backend here;
//! every enforcement request fails closed via [`UnsupportedSandbox`] with an
//! honest per-OS limit string. Real isolation needs a tested OS backend that
//! closes inherited capabilities (docs/SECURITY.md sections 3-4).
//!
//! Source evidence: repo HEAD `5af7884`,
//! `crates/security/src/sandbox.rs:1` (policy only, no Landlock syscalls),
//! `docs/SECURITY.md` sections 3-4, `AGENTS.md` sandbox rules,
//! PAR-006 card via `python3 tools/completion_plan.py --card PAR-006`.
#![forbid(unsafe_code)]

use std::fmt;
use std::path::{Path, PathBuf};

/// Operating systems covered by the matrix. Data rows only.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Os {
    Linux,
    Macos,
    Windows,
}

impl Os {
    /// Short stable identifier for logs and receipts.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Macos => "macos",
            Self::Windows => "windows",
        }
    }

    /// OS this binary was compiled for.
    #[must_use]
    pub fn current() -> Self {
        #[cfg(target_os = "windows")]
        return Self::Windows;
        #[cfg(target_os = "macos")]
        return Self::Macos;
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        return Self::Linux;
    }
}

impl fmt::Display for Os {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Isolation backend selector. This crate ships no backend, so both variants
/// fail closed in [`enforce`]; `PlatformOptIn` only names the backend an
/// integrator would have to provide and test on the real platform.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Backend {
    None,
    PlatformOptIn,
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::PlatformOptIn => write!(f, "platform-opt-in"),
        }
    }
}

/// One per-OS capability row. Plain data: no syscalls, no enforcement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformRow {
    pub os: Os,
    /// Backend an integrator would need for real isolation on this OS.
    pub required_backend: Backend,
    /// Always `false` for this types-only module: no isolation is provided.
    pub isolated: bool,
    /// Honest platform limit shown to callers instead of fake isolation.
    pub limits: &'static str,
}

/// Full matrix: one row per supported OS.
pub const PLATFORM_MATRIX: [PlatformRow; 3] = [
    PlatformRow {
        os: Os::Linux,
        required_backend: Backend::PlatformOptIn,
        isolated: false,
        limits: "no Landlock backend in this module; inherited capabilities not closed",
    },
    PlatformRow {
        os: Os::Macos,
        required_backend: Backend::PlatformOptIn,
        isolated: false,
        limits: "no Seatbelt profile backend in this module; inherited capabilities not closed",
    },
    PlatformRow {
        os: Os::Windows,
        required_backend: Backend::PlatformOptIn,
        isolated: false,
        limits: "no Job-Object/AppContainer backend in this module; inherited capabilities not closed",
    },
];

/// Returns the per-OS capability table (Linux/macOS/Windows rows).
#[must_use]
pub fn capability_table() -> [PlatformRow; 3] {
    PLATFORM_MATRIX
}

/// Looks up the matrix row for one OS.
#[must_use]
pub fn row_for(os: Os) -> PlatformRow {
    match os {
        Os::Linux => PLATFORM_MATRIX[0],
        Os::Macos => PLATFORM_MATRIX[1],
        Os::Windows => PLATFORM_MATRIX[2],
    }
}

/// Fail-closed receipt: the requested sandbox capability is unsupported.
/// Callers must deny the operation; this error grants nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsupportedSandbox {
    pub os: Os,
    pub backend: Backend,
    pub reason: &'static str,
}

impl fmt::Display for UnsupportedSandbox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unsupported sandbox on {} (backend {}): {}",
            self.os, self.backend, self.reason
        )
    }
}

impl std::error::Error for UnsupportedSandbox {}

/// Session token a real backend would hand out. Unconstructible outside this
/// module; [`enforce`] never returns one today (fail-closed).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnforcedSession {
    _token: (),
}

/// Fail-closed enforcement entry point. Always returns
/// `Err(UnsupportedSandbox)`: this types-only module provides no isolation.
/// `Backend::None` fails closed (nothing to enforce with); `PlatformOptIn`
/// fails closed (no tested OS backend bundled).
pub fn enforce(os: Os, backend: Backend) -> Result<EnforcedSession, UnsupportedSandbox> {
    let row = row_for(os);
    Err(UnsupportedSandbox {
        os,
        backend,
        reason: row.limits,
    })
}

/// Marker: pre/post hooks and project config are advisory only and can never
/// grant authority (docs/SECURITY.md section 1, PAR-006-T03). Code must branch
/// on this constant's meaning, not on any hook output, when policy denies.
pub const HOOK_CANNOT_GRANT_AUTHORITY: bool = true;

/// Advisory hook opinion. `allow == true` must never override a policy deny.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HookOpinion {
    pub allow: bool,
}

/// Policy decision. Hooks feed into [`authorize_with_hook`] but cannot flip a
/// deny into an allow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Decision {
    Allow,
    Deny { reason: &'static str },
}

impl Decision {
    #[must_use]
    pub fn is_allow(&self) -> bool {
        matches!(self, Self::Allow)
    }

    #[must_use]
    pub fn is_deny(&self) -> bool {
        matches!(self, Self::Deny { .. })
    }
}

/// Combines a policy decision with an advisory hook opinion. A hook `allow`
/// never grants authority: policy deny always wins. A hook refusal can only
/// deny further, never allow.
#[must_use]
pub fn authorize_with_hook(policy: &Decision, hook: HookOpinion) -> Decision {
    debug_assert!(HOOK_CANNOT_GRANT_AUTHORITY);
    match policy {
        Decision::Deny { reason } => Decision::Deny { reason },
        Decision::Allow => {
            if hook.allow {
                Decision::Allow
            } else {
                Decision::Deny {
                    reason: "hook refused advisory allow; policy allow withheld",
                }
            }
        }
    }
}

/// Mandatory protected paths a wildcard permission can never open
/// (docs/SECURITY.md section 1, PAR-006-T01). Case-insensitive on purpose:
/// secrets must not hide behind capitalisation.
#[must_use]
pub fn is_protected_path(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    if name == ".env" || name.starts_with(".env.") {
        return true;
    }
    const MARKERS: [&str; 12] = [
        ".ssh/", ".aws/", ".gnupg/", ".kube/", ".docker/", "id_rsa", "id_ed25519", ".pem",
        ".p12", "credential", "secret", "token",
    ];
    if MARKERS.iter().any(|m| lower.contains(m)) {
        return true;
    }
    const PREFIXES: [&str; 8] = [
        "/etc/", "/proc/", "/sys/", "/boot/", "c:/windows", "c:/program files", "/system/",
        "/private/etc/",
    ];
    if PREFIXES.iter().any(|p| lower.starts_with(p)) {
        return true;
    }
    false
}

/// File authorization where mandatory protection beats any permission
/// pattern, including `"*"`. Protected paths always deny; anything else
/// allows only on an exact match or a global `"*"` grant.
#[must_use]
pub fn authorize_file(path: &str, permission_pattern: &str) -> Decision {
    if is_protected_path(path) {
        return Decision::Deny {
            reason: "mandatory protection holds regardless of permission grants",
        };
    }
    let pattern = permission_pattern.trim();
    if pattern == "*" || pattern == path {
        Decision::Allow
    } else {
        Decision::Deny {
            reason: "no matching permission grant",
        }
    }
}

/// Receipt proving a denial performed no side effect (PAR-006-T04,
/// docs/SECURITY.md section 4). Constructed without touching the filesystem.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DenialReceipt {
    pub path: PathBuf,
    pub reason: &'static str,
    pub side_effect_performed: bool,
    /// Always `None`: a denial records no marker artifact.
    pub side_effect_marker: Option<PathBuf>,
}

impl DenialReceipt {
    #[must_use]
    pub fn deny(path: impl Into<PathBuf>, reason: &'static str) -> Self {
        Self {
            path: path.into(),
            reason,
            side_effect_performed: false,
            side_effect_marker: None,
        }
    }
}

impl fmt::Display for DenialReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "denied {} [{}]; side_effect_performed={}",
            self.path.display(),
            self.reason,
            self.side_effect_performed
        )
    }
}

impl std::error::Error for DenialReceipt {}

/// Attempts a file write only when `allowed` is true. When `allowed` is
/// false the filesystem is never touched and a [`DenialReceipt`] with no
/// side-effect marker is returned, so tests can assert absence of effects.
pub fn attempt_write_if_allowed(
    allowed: bool,
    path: &Path,
    bytes: &[u8],
) -> Result<(), DenialReceipt> {
    if allowed {
        std::fs::write(path, bytes).map_err(|_| DenialReceipt::deny(path, "write failed"))?;
        Ok(())
    } else {
        Err(DenialReceipt::deny(path, "policy denied write"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_fails_closed() {
        for os in [Os::Linux, Os::Macos, Os::Windows] {
            for backend in [Backend::None, Backend::PlatformOptIn] {
                let err = enforce(os, backend).expect_err("types-only module must fail closed");
                assert_eq!(err.os, os);
                assert_eq!(err.backend, backend);
                assert!(!err.reason.is_empty(), "limit string must be honest");
                assert!(format!("{err}").contains("unsupported sandbox"));
            }
        }
        let table = capability_table();
        assert_eq!(table.len(), 3);
        assert!(table.iter().all(|row| !row.isolated));
    }

    #[test]
    fn wildcard_cannot_bypass_protected() {
        for protected in [
            "/work/project/.env",
            "/work/project/.env.local",
            "/etc/shadow",
            "/proc/self/environ",
            "C:\\Windows\\System32\\config\\SAM",
            "/work/project/.ssh/id_rsa",
            "/work/project/secrets/token.json",
        ] {
            assert!(is_protected_path(protected), "missing {protected}");
            assert!(
                authorize_file(protected, "*").is_deny(),
                "wildcard must not open {protected}"
            );
        }
        assert!(authorize_file("/work/project/src/main.rs", "*").is_allow());
        assert!(authorize_file("/work/project/src/main.rs", "/other/path").is_deny());
    }

    #[test]
    fn hooks_cannot_grant() {
        assert!(HOOK_CANNOT_GRANT_AUTHORITY);
        let policy_deny = Decision::Deny {
            reason: "mandatory protection holds regardless of permission grants",
        };
        let granted_hook = HookOpinion { allow: true };
        assert!(authorize_with_hook(&policy_deny, granted_hook).is_deny());
        let refused = authorize_with_hook(&Decision::Allow, HookOpinion { allow: false });
        assert!(refused.is_deny());
        assert!(authorize_with_hook(&Decision::Allow, granted_hook).is_allow());
    }

    #[test]
    fn denial_leaves_no_side_effect_marker() {
        let dir = std::env::temp_dir().join(format!("rk-pm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("blocked.txt");
        let receipt = attempt_write_if_allowed(false, &target, b"must not land")
            .expect_err("denied write must return a receipt");
        assert!(!receipt.side_effect_performed);
        assert_eq!(receipt.side_effect_marker, None);
        assert!(!target.exists(), "denied write left a file behind");
        assert!(format!("{receipt}").contains("side_effect_performed=false"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn matrix_covers_linux_macos_windows() {
        let table = capability_table();
        let oss: Vec<Os> = table.iter().map(|row| row.os).collect();
        assert!(oss.contains(&Os::Linux));
        assert!(oss.contains(&Os::Macos));
        assert!(oss.contains(&Os::Windows));
        for row in &table {
            assert_eq!(row.required_backend, Backend::PlatformOptIn);
            assert!(!row.limits.is_empty());
            assert_eq!(row_for(row.os), *row);
        }
    }
}
