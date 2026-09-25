#![forbid(unsafe_code)]

//! Combined TUI contexts: KV store + route nav + runtime dims.
//!
//! Thin facade over [`crate::kv_ctx::KvCtx`], [`crate::route_ctx::RouteCtx`],
//! and [`crate::runtime_ctx::RuntimeCtx`]; bounds and fail-closed
//! semantics live in the delegates.

use crate::kv_ctx::KvCtx;
use crate::route_ctx::RouteCtx;
use crate::runtime_ctx::RuntimeCtx;

/// Max chars kept in [`CtxBundle::summary`].
pub const SUMMARY_CAP: usize = 256;

/// Owned triple of lane-local TUI contexts.
#[derive(Debug, Clone)]
pub struct CtxBundle {
    pub kv: KvCtx,
    pub route: RouteCtx,
    pub rt: RuntimeCtx,
}

impl CtxBundle {
    #[must_use]
    pub fn new(cwd: &str) -> Self {
        let mut kv = KvCtx::new();
        kv.set("cwd", cwd);
        Self {
            kv,
            route: RouteCtx::new("home").expect("home is a valid route"),
            rt: RuntimeCtx::default(),
        }
    }

    pub fn set(&mut self, k: &str, v: &str) -> bool {
        self.kv.set(k, v)
    }

    pub fn go(&mut self, name: &str) -> bool {
        self.route.go(name)
    }

    pub fn ready(&mut self) {
        self.rt.mark_ready();
    }

    #[must_use]
    pub fn summary(&self) -> String {
        let (cols, rows) = self.rt.dims();
        let state = if self.rt.is_ready() {
            "ready"
        } else {
            "not-ready"
        };
        let s = format!("route={} {cols}x{rows} {state}", self.route.current());
        if s.chars().count() > SUMMARY_CAP {
            s.chars().take(SUMMARY_CAP).collect()
        } else {
            s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_seeds_cwd() {
        let b = CtxBundle::new("/tmp");
        assert_eq!(b.kv.get("cwd"), Some("/tmp"));
    }

    #[test]
    fn set_delegates_to_kv() {
        let mut b = CtxBundle::new("/tmp");
        assert!(b.set("a", "1"));
        assert_eq!(b.kv.get("a"), Some("1"));
        assert!(!b.set(&"k".repeat(65), "v"));
    }

    #[test]
    fn go_changes_route() {
        let mut b = CtxBundle::new("/tmp");
        assert!(b.go("session"));
        assert_eq!(b.route.current(), "session");
        assert!(!b.go(""));
    }

    #[test]
    fn ready_flips_summary() {
        let mut b = CtxBundle::new("/tmp");
        assert!(b.summary().ends_with("not-ready"));
        b.ready();
        assert!(b.summary().ends_with(" ready"));
    }

    #[test]
    fn summary_format_and_cap() {
        let b = CtxBundle::new("/tmp");
        assert_eq!(b.summary(), "route=home 80x24 not-ready");
        assert!(b.summary().chars().count() <= SUMMARY_CAP);
    }

    #[test]
    fn summary_caps_long_route() {
        let mut b = CtxBundle::new("/tmp");
        assert!(b.go(&"x".repeat(128)));
        let s = b.summary();
        assert!(s.starts_with("route=xxx"));
        assert!(s.chars().count() <= SUMMARY_CAP);
    }
}
