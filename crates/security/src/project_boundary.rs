//! Cross-project file access control (SEC-007). Files outside project root need approval.
#![forbid(unsafe_code)]
use std::{
    fmt,
    path::{Component, Path, PathBuf},
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeedsApproval {
    pub path: PathBuf,
    pub reason: String,
}
impl NeedsApproval {
    fn new(path: PathBuf, reason: impl Into<String>) -> Self {
        Self {
            path,
            reason: reason.into(),
        }
    }
}
impl fmt::Display for NeedsApproval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "needs approval: {} [{}]",
            self.reason,
            self.path.display()
        )
    }
}
impl std::error::Error for NeedsApproval {}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectBoundary {
    pub root: PathBuf,
    pub approved_external: Vec<PathBuf>,
}
impl ProjectBoundary {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: lexical_normalize(&root.into()),
            approved_external: Vec::new(),
        }
    }
    #[must_use]
    pub fn is_within_project(&self, path: impl AsRef<Path>) -> bool {
        self.normalized(path.as_ref()).starts_with(&self.root)
    }
    pub fn check_access(&self, path: impl AsRef<Path>) -> Result<(), NeedsApproval> {
        let n = self.normalized(path.as_ref());
        if n.starts_with(&self.root) {
            return Ok(());
        }
        if self
            .approved_external
            .iter()
            .any(|a| n == *a || n.starts_with(a))
        {
            return Ok(());
        }
        Err(NeedsApproval::new(
            n,
            "file is outside the project root and not approved",
        ))
    }
    pub fn approve_external(&mut self, path: impl Into<PathBuf>) {
        let n = lexical_normalize(&path.into());
        if !self.approved_external.contains(&n) {
            self.approved_external.push(n);
        }
    }
    pub fn revoke_external(&mut self, path: impl AsRef<Path>) -> bool {
        let n = lexical_normalize(path.as_ref());
        let before = self.approved_external.len();
        self.approved_external.retain(|a| *a != n);
        self.approved_external.len() != before
    }
    fn normalized(&self, path: &Path) -> PathBuf {
        let abs = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        lexical_normalize(&abs)
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
#[cfg(test)]
mod tests {
    use super::*;
    fn boundary() -> ProjectBoundary {
        ProjectBoundary::new("/work/project")
    }
    #[test]
    fn within_project_allowed() {
        let b = boundary();
        assert!(b.is_within_project("/work/project/src/main.rs"));
        assert!(b.check_access("/work/project/src/main.rs").is_ok());
        assert!(b.check_access("/work/project/Cargo.toml").is_ok());
    }
    #[test]
    fn outside_project_needs_approval() {
        let b = boundary();
        assert!(!b.is_within_project("/tmp/data.json"));
        let err = b.check_access("/tmp/data.json").unwrap_err();
        assert_eq!(err.path, PathBuf::from("/tmp/data.json"));
    }
    #[test]
    fn approved_external_works() {
        let mut b = boundary();
        assert!(b.check_access("/tmp/data.json").is_err());
        b.approve_external("/tmp/data.json");
        assert!(b.check_access("/tmp/data.json").is_ok());
    }
    #[test]
    fn revoke_removes_access() {
        let mut b = boundary();
        b.approve_external("/tmp/data.json");
        assert!(b.check_access("/tmp/data.json").is_ok());
        assert!(b.revoke_external("/tmp/data.json"));
        assert!(b.check_access("/tmp/data.json").is_err());
        assert!(!b.revoke_external("/tmp/data.json"));
    }
    #[test]
    fn parent_traversal_detected() {
        let b = boundary();
        assert!(!b.is_within_project("/work/project/../other-project/file.txt"));
        assert!(b
            .check_access("/work/project/../other-project/file.txt")
            .is_err());
        assert!(!b.is_within_project("../other-project/file.txt"));
        assert!(b.check_access("../other-project/file.txt").is_err());
    }
}
