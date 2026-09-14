//! System-file policy: readable system files, never writable by agent.
#![forbid(unsafe_code)]
use std::{fmt, path::Path};
pub const READ_ALLOWED: &[&str] = &["/etc/hosts", "/etc/resolv.conf", "/proc/cpuinfo", "/etc/os-release"];
pub const WRITE_DENIED_PREFIXES: &[&str] = &["/etc/", "/usr/", "/boot/", "/sbin/", "/sys/"];
pub const SYSTEM_READ_ALLOWED: &[&str] = READ_ALLOWED;
pub const SYSTEM_WRITE_DENIED: &[&str] = WRITE_DENIED_PREFIXES;
pub const SYSTEM_PATHS: &[&str] = WRITE_DENIED_PREFIXES;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemFileDenied { pub path: String, pub reason: String }
impl fmt::Display for SystemFileDenied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}: {}", self.path, self.reason) }
}
impl std::error::Error for SystemFileDenied {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SystemFilePolicy { pub trusted: bool }
impl SystemFilePolicy {
    #[must_use] pub fn new(trusted: bool) -> Self { Self { trusted } }
    pub fn set_trusted(&mut self, trusted: bool) { self.trusted = trusted; }
    pub fn check_read(&self, path: impl AsRef<Path>) -> Result<(), SystemFileDenied> {
        let n = norm(path.as_ref());
        if self.trusted && n.starts_with("/etc/") { return Ok(()); }
        if READ_ALLOWED.contains(&n.as_str()) { return Ok(()); }
        if WRITE_DENIED_PREFIXES.iter().any(|p| n.starts_with(p)) || n == "/etc" || is_sys(&n) {
            return Err(denied(&n, "system-file read not in allowlist"));
        }
        Ok(())
    }
    pub fn check_write(&self, path: impl AsRef<Path>) -> Result<(), SystemFileDenied> {
        let n = norm(path.as_ref());
        if WRITE_DENIED_PREFIXES.iter().any(|p| n.starts_with(p)) || n == "/etc" || is_sys(&n) {
            return Err(denied(&n, "system paths are read-only to agents"));
        }
        Ok(())
    }
}
impl Default for SystemFilePolicy { fn default() -> Self { Self::new(false) } }
fn denied(path: &str, reason: &str) -> SystemFileDenied { SystemFileDenied { path: path.to_owned(), reason: reason.to_owned() } }
fn is_sys(n: &str) -> bool { n.starts_with("/proc/") || n == "/proc" || n.starts_with("/sys") }
fn norm(path: &Path) -> String { path.to_string_lossy().replace('\\', "/").to_ascii_lowercase() }
#[cfg(test)] mod tests {
    use super::*;
    fn untrusted() -> SystemFilePolicy { SystemFilePolicy::new(false) }
    #[test] fn read_etc_hosts_allowed() { assert!(untrusted().check_read("/etc/hosts").is_ok()); }
    #[test] fn write_etc_hosts_denied() { assert!(untrusted().check_write("/etc/hosts").is_err()); }
    #[test] fn trusted_read_more() { assert!(SystemFilePolicy::new(true).check_read("/etc/passwd").is_ok()); }
    #[test] fn trusted_write_still_denied() {
        let p = SystemFilePolicy::new(true);
        assert!(p.check_write("/etc/anything").is_err());
        assert!(p.check_write("/usr/bin/x").is_err());
    }
    #[test] fn non_system_allowed() {
        assert!(untrusted().check_read("/home/user/file.txt").is_ok());
        assert!(untrusted().check_write("/home/user/file.txt").is_ok());
    }
}
