#![forbid(unsafe_code)]
//! Permission gate over [`PermissionCtx`]: queue, resolve, summarize.

use crate::permission_ctx::PermissionCtx;

/// Gate wrapping a [`PermissionCtx`].
#[derive(Debug, Default)]
pub struct PermGate {
    pub ctx: PermissionCtx,
}

impl PermGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// True if already granted; else queues `id` and returns false.
    pub fn ask(&mut self, id: &str) -> bool {
        if self.ctx.is_granted(id) {
            return true;
        }
        self.ctx.request(id);
        false
    }

    /// Resolve pending: allow grants, deny rejects. False if missing.
    pub fn resolve(&mut self, id: &str, allow: bool) -> bool {
        if allow {
            self.ctx.grant(id)
        } else {
            self.ctx.deny(id)
        }
    }

    /// `"id granted|pending"`, id part capped so the status word survives.
    pub fn gate_summary(&self, id: &str) -> String {
        let word = if self.ctx.is_granted(id) {
            "granted"
        } else {
            "pending"
        };
        let cap = 128usize.saturating_sub(word.len() + 1);
        let head: String = id.chars().take(cap).collect();
        format!("{head} {word}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ask_queues_pending_false() {
        let mut g = PermGate::new();
        assert!(!g.ask("bash"));
        assert_eq!(g.gate_summary("bash"), "bash pending");
    }

    #[test]
    fn resolve_allow_grants() {
        let mut g = PermGate::new();
        assert!(!g.ask("bash"));
        assert!(g.resolve("bash", true));
        assert!(g.ask("bash"));
        assert_eq!(g.gate_summary("bash"), "bash granted");
    }

    #[test]
    fn resolve_deny_rejects() {
        let mut g = PermGate::new();
        assert!(!g.ask("edit"));
        assert!(g.resolve("edit", false));
        assert!(!g.ask("edit"));
        assert_eq!(g.gate_summary("edit"), "edit pending");
    }

    #[test]
    fn resolve_missing_false() {
        let mut g = PermGate::new();
        assert!(!g.resolve("nope", true));
        assert!(!g.resolve("nope", false));
    }

    #[test]
    fn ask_invalid_false() {
        let mut g = PermGate::new();
        assert!(!g.ask(""));
        assert!(!g.resolve("", true));
        assert_eq!(g.gate_summary(""), " pending");
    }

    #[test]
    fn summary_caps_128() {
        let g = PermGate::new();
        let s = g.gate_summary(&"x".repeat(200));
        assert!(s.chars().count() <= 128);
        assert!(s.ends_with("pending"));
    }
}
