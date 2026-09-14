//! Trusted user toggle that never bypasses mandatory controls.
#![forbid(unsafe_code)]
use super::{classify_destructive_argv, is_secret_path};
use std::{
    path::Path,
    sync::Mutex,
    time::{Duration, Instant},
};
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Trust {
    Untrusted,
    SessionTrust,
    FullTrust,
}
#[derive(Copy, Clone, Debug)]
pub struct TrustLevel {
    pub level: Trust,
    pub granted_at: Instant,
    pub duration: Duration,
}
impl TrustLevel {
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.level != Trust::Untrusted && self.granted_at.elapsed() < self.duration
    }
}
#[derive(Debug)]
pub struct TrustedUserToggle {
    current: Mutex<TrustLevel>,
}
impl TrustedUserToggle {
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: Mutex::new(TrustLevel {
                level: Trust::Untrusted,
                granted_at: Instant::now(),
                duration: Duration::ZERO,
            }),
        }
    }
    pub fn grant(&self, level: Trust, duration: Duration) {
        if let Ok(mut cur) = self.current.lock() {
            *cur = TrustLevel {
                level,
                granted_at: Instant::now(),
                duration,
            };
        }
    }
    pub fn revoke(&self) {
        if let Ok(mut cur) = self.current.lock() {
            *cur = TrustLevel {
                level: Trust::Untrusted,
                granted_at: Instant::now(),
                duration: Duration::ZERO,
            };
        }
    }
    #[must_use]
    pub fn current(&self) -> Trust {
        self.current
            .lock()
            .map(|cur| {
                if cur.is_live() {
                    cur.level
                } else {
                    Trust::Untrusted
                }
            })
            .unwrap_or(Trust::Untrusted)
    }
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.current
            .lock()
            .map(|cur| cur.level != Trust::Untrusted && cur.granted_at.elapsed() >= cur.duration)
            .unwrap_or(false)
    }
    #[must_use]
    pub fn can_bypass_for_path(&self, path: &Path) -> bool {
        if self.current() == Trust::Untrusted {
            return false;
        }
        !is_secret_path(path)
    }
    #[must_use]
    pub fn can_bypass_for_command(&self, program: &str, args: &[String], cwd: &Path) -> bool {
        if self.current() == Trust::Untrusted {
            return false;
        }
        classify_destructive_argv(program, args, cwd).is_none()
    }
}
impl Default for TrustedUserToggle {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, thread, time::Duration as StdDuration};
    #[test]
    fn default_untrusted() {
        let t = TrustedUserToggle::new();
        assert_eq!(t.current(), Trust::Untrusted);
        assert!(!t.is_expired());
    }
    #[test]
    fn grant_and_check() {
        let t = TrustedUserToggle::new();
        t.grant(Trust::SessionTrust, StdDuration::from_secs(60));
        assert_eq!(t.current(), Trust::SessionTrust);
        assert!(!t.is_expired());
    }
    #[test]
    fn expiry_works() {
        let t = TrustedUserToggle::new();
        t.grant(Trust::SessionTrust, Duration::ZERO);
        thread::sleep(StdDuration::from_millis(1));
        assert!(t.is_expired());
        assert_eq!(t.current(), Trust::Untrusted);
    }
    #[test]
    fn mandatory_never_bypassed() {
        let t = TrustedUserToggle::new();
        t.grant(Trust::FullTrust, StdDuration::from_secs(60));
        assert_eq!(t.current(), Trust::FullTrust);
        assert!(!t.can_bypass_for_path(Path::new("/work/project/.env")));
        assert!(!t.can_bypass_for_path(Path::new("/work/project/.env.local")));
        assert!(!t.can_bypass_for_command(
            "rm",
            &["-rf".to_owned(), "*".to_owned()],
            Path::new("/work/project")
        ));
        assert!(!t.can_bypass_for_command(
            "mkfs",
            &["/dev/sda1".to_owned()],
            Path::new("/work/project")
        ));
        assert!(t.can_bypass_for_path(Path::new("/work/project/src/lib.rs")));
        assert!(t.can_bypass_for_command(
            "cargo",
            &["test".to_owned()],
            Path::new("/work/project")
        ));
        let _ = PathBuf::from("keep");
    }
    #[test]
    fn revoke_immediate() {
        let t = TrustedUserToggle::new();
        t.grant(Trust::FullTrust, StdDuration::from_secs(60));
        assert_eq!(t.current(), Trust::FullTrust);
        t.revoke();
        assert_eq!(t.current(), Trust::Untrusted);
        assert!(!t.can_bypass_for_path(Path::new("/work/project/src/lib.rs")));
    }
}
