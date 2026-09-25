#![forbid(unsafe_code)]
//! Project context (std-only).
//!
//! Evidence (TS checkout):
//! - `packages/tui/src/context/project.tsx` project/worktree/mainDir + instance path directory
//! - `packages/tui/src/context/directory.ts:12-15` directory + `:` + vcs branch label
//! - `packages/tui/src/context/args.tsx:3-11` optional model/agent/prompt/continue/session/fork/auto

/// Max root chars.
pub const MAX_ROOT: usize = 512;
/// Max args entries.
pub const MAX_ARGS: usize = 32;
/// Max chars per arg.
pub const MAX_ARG_LEN: usize = 256;
/// Max vcs chars.
pub const MAX_VCS: usize = 32;

/// Project context: work root + CLI args + vcs name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectCtx {
    pub root: String,
    pub args: Vec<String>,
    pub vcs: String,
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

impl ProjectCtx {
    #[must_use]
    pub fn new(root: &str) -> Self {
        Self {
            root: truncate(root, MAX_ROOT),
            args: Vec::new(),
            vcs: truncate("git", MAX_VCS),
        }
    }

    pub fn set_root(&mut self, path: &str) -> bool {
        if path.is_empty() {
            return false;
        }
        self.root = truncate(path, MAX_ROOT);
        true
    }

    pub fn add_arg(&mut self, arg: String) -> bool {
        if self.args.len() >= MAX_ARGS {
            return false;
        }
        self.args.push(truncate(&arg, MAX_ARG_LEN));
        true
    }

    #[must_use]
    pub fn label(&self) -> String {
        format!("{} [{}] +{} args", self.root, self.vcs, self.args.len())
    }
}

impl Default for ProjectCtx {
    fn default() -> Self {
        Self::new("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_root_false() {
        let mut c = ProjectCtx::new("/a");
        assert!(!c.set_root(""));
        assert_eq!(c.root, "/a");
    }

    #[test]
    fn args_cap() {
        let mut c = ProjectCtx::new("/a");
        for i in 0..MAX_ARGS {
            assert!(c.add_arg(format!("a{i}")));
        }
        assert!(!c.add_arg("extra".to_string()));
        assert_eq!(c.args.len(), MAX_ARGS);
    }

    #[test]
    fn label_format() {
        let mut c = ProjectCtx::new("/repo");
        c.add_arg("x".to_string());
        assert_eq!(c.label(), "/repo [git] +1 args");
    }

    #[test]
    fn vcs_default_git() {
        assert_eq!(ProjectCtx::new("/a").vcs, "git");
    }

    #[test]
    fn arg_truncates() {
        let mut c = ProjectCtx::new("/a");
        let long = "a".repeat(MAX_ARG_LEN + 10);
        assert!(c.add_arg(long));
        assert_eq!(c.args[0].chars().count(), MAX_ARG_LEN);
    }

    #[test]
    fn root_truncates() {
        let long = "r".repeat(MAX_ROOT + 5);
        assert!(ProjectCtx::new(&long).root.chars().count() <= MAX_ROOT);
        let mut c = ProjectCtx::new("/a");
        assert!(c.set_root(&long));
        assert_eq!(c.root.chars().count(), MAX_ROOT);
    }
}
