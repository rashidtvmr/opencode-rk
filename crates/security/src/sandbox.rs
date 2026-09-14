//! Filesystem sandbox policy with path resolution (SEC-005). Policy only, no Landlock syscalls.
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    path::{Component, Path, PathBuf},
};
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileAction {
    Read,
    Write,
    Execute,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxDenied {
    pub path: PathBuf,
    pub action: FileAction,
    pub reason: String,
}
impl SandboxDenied {
    fn new(path: PathBuf, action: FileAction, reason: impl Into<String>) -> Self {
        Self {
            path,
            action,
            reason: reason.into(),
        }
    }
}
impl fmt::Display for SandboxDenied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "sandbox denied ({:?}): {} [{}]",
            self.action,
            self.reason,
            self.path.display()
        )
    }
}
impl std::error::Error for SandboxDenied {}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxPolicy {
    pub allowed_read: Vec<PathBuf>,
    pub allowed_write: Vec<PathBuf>,
    pub denied: Vec<PathBuf>,
}
impl SandboxPolicy {
    #[must_use]
    pub fn new(
        allowed_read: Vec<PathBuf>,
        allowed_write: Vec<PathBuf>,
        denied: Vec<PathBuf>,
    ) -> Self {
        Self {
            allowed_read,
            allowed_write,
            denied,
        }
    }
}
impl Default for SandboxPolicy {
    fn default() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut allowed_read = vec![cwd.clone(), PathBuf::from("/tmp")];
        if let Ok(home) = std::env::var("HOME") {
            if !home.is_empty() {
                allowed_read.push(PathBuf::from(home));
            }
        }
        Self {
            allowed_read,
            allowed_write: vec![cwd, PathBuf::from("/tmp")],
            denied: Vec::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct SandboxCheck {
    policy: SandboxPolicy,
}
impl SandboxCheck {
    #[must_use]
    pub fn new(policy: SandboxPolicy) -> Self {
        Self { policy }
    }
    #[must_use]
    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }
    pub fn is_allowed(
        &self,
        path: impl AsRef<Path>,
        action: FileAction,
    ) -> Result<(), SandboxDenied> {
        let resolved = resolve_path(path.as_ref());
        for d in &self.policy.denied {
            if resolved.starts_with(resolve_path(d)) {
                return Err(SandboxDenied::new(
                    resolved,
                    action,
                    "path is inside a denied prefix",
                ));
            }
        }
        let under =
            |list: &Vec<PathBuf>| list.iter().any(|a| resolved.starts_with(resolve_path(a)));
        let ok = match action {
            FileAction::Read => under(&self.policy.allowed_read),
            FileAction::Write => under(&self.policy.allowed_write),
            FileAction::Execute => {
                under(&self.policy.allowed_read) || under(&self.policy.allowed_write)
            }
        };
        if ok {
            Ok(())
        } else {
            Err(SandboxDenied::new(
                resolved,
                action,
                "path is outside the sandbox",
            ))
        }
    }
}
#[must_use]
pub fn resolve_path(path: &Path) -> PathBuf {
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
    if abs.is_symlink() {
        if let Ok(target) = std::fs::read_link(&abs) {
            let base = if target.is_absolute() {
                target
            } else {
                abs.parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("/"))
                    .join(target)
            };
            return resolve_path(&base);
        }
    }
    let mut prefix = abs.clone();
    while !prefix.exists() {
        match prefix.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => prefix = parent.to_path_buf(),
            _ => {
                break;
            }
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
        std::env::temp_dir().join(format!("rk-sandbox-{}-{tag}", std::process::id()))
    }
    fn check(
        policy: &SandboxPolicy,
        path: impl AsRef<Path>,
        action: FileAction,
    ) -> Result<(), SandboxDenied> {
        SandboxCheck::new(policy.clone()).is_allowed(path, action)
    }
    #[test]
    fn default_allows_cwd() {
        let policy = SandboxPolicy::default();
        let cwd = std::env::current_dir().unwrap();
        let file = cwd.join("sandbox-default-probe.txt");
        assert!(check(&policy, &file, FileAction::Read).is_ok());
        assert!(check(&policy, &file, FileAction::Write).is_ok());
    }
    #[test]
    fn denied_overrides_allowed() {
        let policy = SandboxPolicy {
            allowed_read: vec![PathBuf::from("/etc")],
            allowed_write: vec![PathBuf::from("/etc")],
            denied: vec![PathBuf::from("/etc/shadow")],
        };
        assert!(check(&policy, "/etc/hosts", FileAction::Read).is_ok());
        assert!(check(&policy, "/etc/shadow", FileAction::Read).is_err());
        assert!(check(&policy, "/etc/shadow", FileAction::Write).is_err());
    }
    #[test]
    fn traversal_blocked() {
        let base = tmp_base("traversal");
        let _ = fs::remove_dir_all(&base);
        let allowed = base.join("allowed");
        let outside = base.join("outside");
        fs::create_dir_all(&allowed).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.txt"), b"secret").unwrap();
        let policy = SandboxPolicy {
            allowed_read: vec![allowed.clone()],
            allowed_write: vec![allowed.clone()],
            denied: Vec::new(),
        };
        assert!(check(&policy, allowed.join("ok.txt"), FileAction::Read).is_ok());
        assert!(check(
            &policy,
            allowed.join("../outside/secret.txt"),
            FileAction::Read
        )
        .is_err());
        assert!(check(
            &policy,
            PathBuf::from("/tmp/../etc/passwd"),
            FileAction::Read
        )
        .is_err());
        let _ = fs::remove_dir_all(&base);
    }
    #[test]
    fn symlink_resolved() {
        let base = tmp_base("symlink");
        let _ = fs::remove_dir_all(&base);
        let allowed = base.join("allowed");
        let outside = base.join("outside");
        fs::create_dir_all(&allowed).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.txt"), b"secret").unwrap();
        fs::write(allowed.join("ok.txt"), b"ok").unwrap();
        std::os::unix::fs::symlink(outside.join("secret.txt"), allowed.join("link.txt")).unwrap();
        let policy = SandboxPolicy {
            allowed_read: vec![allowed.clone()],
            allowed_write: Vec::new(),
            denied: vec![outside.clone()],
        };
        assert!(check(&policy, allowed.join("ok.txt"), FileAction::Read).is_ok());
        assert!(check(&policy, allowed.join("link.txt"), FileAction::Read).is_err());
        assert!(check(&policy, outside.join("secret.txt"), FileAction::Read).is_err());
        let _ = fs::remove_dir_all(&base);
    }
    #[test]
    fn outside_sandbox_denied() {
        let base = tmp_base("outside");
        let _ = fs::remove_dir_all(&base);
        let allowed = base.join("allowed");
        fs::create_dir_all(&allowed).unwrap();
        let policy = SandboxPolicy {
            allowed_read: vec![allowed.clone()],
            allowed_write: vec![allowed.clone()],
            denied: Vec::new(),
        };
        assert!(check(&policy, allowed.join("in.txt"), FileAction::Read).is_ok());
        assert!(check(&policy, "/etc/hosts", FileAction::Read).is_err());
        assert!(check(&policy, base.join("elsewhere/out.txt"), FileAction::Write).is_err());
        let _ = fs::remove_dir_all(&base);
    }
}
