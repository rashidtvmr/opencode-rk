//! Context plumbing: readiness gate, frozen runtime paths, base-relative format.
//!
//! Mirrors `helper.tsx:15` (`Show when={init.ready === undefined ||
//! init.ready === true}`), `runtime.tsx:25-32` (`provider` freezes value via
//! `Object.freeze`), `runtime.tsx:46-50` (`required` throws when missing),
//! `path-format.tsx:15-24` (`formatPath`: in-base -> relative, else
//! `abbreviateHome`) @ a0d9b6c.
//!
//! Divergences: `ReadyGate::default` is ready (`undefined` => show in TS).
//! `format_in_base` is 2-arg so HOME is unavailable; outside-base falls back
//! to the absolute path (= `tildefy` with empty home). Reuses
//! `context_kv::Project`/`tildefy`; neither is redefined here.

#![forbid(unsafe_code)]

use crate::context_kv::Project;

/// Gate mirroring `helper.tsx` readiness `Show`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadyGate {
    ready: bool,
}

impl ReadyGate {
    #[must_use]
    pub fn new(ready: bool) -> Self {
        Self { ready }
    }

    /// Mirrors the `Show when=` condition.
    #[must_use]
    pub fn check(&self) -> bool {
        self.ready
    }

    /// Fail-closed waiter: `Ok` when ready, `Err` otherwise.
    pub fn wait(&self) -> Result<(), NotReady> {
        if self.ready {
            Ok(())
        } else {
            Err(NotReady)
        }
    }
}

impl Default for ReadyGate {
    /// TS `ready === undefined` renders; default is ready.
    fn default() -> Self {
        Self { ready: true }
    }
}

/// Marker error when [`ReadyGate::wait`] fires before ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotReady;

/// Frozen runtime paths (`runtime.tsx` `Object.freeze` + `required`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePaths {
    config: String,
    data: String,
    cwd: String,
}

impl RuntimePaths {
    /// Fails when any field is empty (TS `required` throws when missing).
    pub fn new(config: &str, data: &str, cwd: &str) -> Result<Self, &'static str> {
        if config.is_empty() {
            return Err("config is empty");
        }
        if data.is_empty() {
            return Err("data is empty");
        }
        if cwd.is_empty() {
            return Err("cwd is empty");
        }
        Ok(Self {
            config: config.to_string(),
            data: data.to_string(),
            cwd: cwd.to_string(),
        })
    }

    #[must_use]
    pub fn config(&self) -> &str {
        &self.config
    }

    #[must_use]
    pub fn data(&self) -> &str {
        &self.data
    }

    #[must_use]
    pub fn cwd(&self) -> &str {
        &self.cwd
    }
}

/// TS `path-format.tsx:15-24`: empty -> `""`; equal -> `"."`; under base ->
/// relative; else absolute (= `tildefy` with empty home).
#[must_use]
pub fn format_in_base(base: &str, path: &str) -> String {
    Project::new(base, "").display(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_ready_passes() {
        let g = ReadyGate::new(true);
        assert!(g.check());
        assert_eq!(g.wait(), Ok(()));
        assert!(ReadyGate::default().check());
    }

    #[test]
    fn gate_not_ready_blocks() {
        let g = ReadyGate::new(false);
        assert!(!g.check());
        assert_eq!(g.wait(), Err(NotReady));
    }

    #[test]
    fn paths_rejects_empty() {
        assert!(RuntimePaths::new("", "/d", "/c").is_err());
        assert!(RuntimePaths::new("/cfg", "", "/c").is_err());
        assert!(RuntimePaths::new("/cfg", "/d", "").is_err());
    }

    #[test]
    fn paths_getters() {
        let p = RuntimePaths::new("/cfg", "/data", "/repo").unwrap();
        assert_eq!(p.config(), "/cfg");
        assert_eq!(p.data(), "/data");
        assert_eq!(p.cwd(), "/repo");
    }

    #[test]
    fn format_relative() {
        assert_eq!(format_in_base("/repo", "/repo/src/a.rs"), "src/a.rs");
    }

    #[test]
    fn format_dot_and_empty() {
        assert_eq!(format_in_base("/repo", "/repo"), ".");
        assert_eq!(format_in_base("/repo", ""), "");
    }

    #[test]
    fn format_outside_returns_absolute() {
        assert_eq!(format_in_base("/repo", "/etc/hosts"), "/etc/hosts");
    }
}
