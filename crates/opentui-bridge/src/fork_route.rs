#![forbid(unsafe_code)]
//! Fork route plan-then-complete gate (mirrors
//! `packages/tui/src/routes/session/dialog-fork-from-timeline.tsx:12`
//! `DialogForkFromTimeline` pick + `sdk.client.session.fork` + `route.navigate`).
//!
//! Divergences:
//! - TS forks + navigates immediately on `onSelect`; Rust splits pick (`plan`)
//!   from navigation target (`complete`) so host drives RPC.
//! - TS `undefined` full-session fork not modeled; `plan` requires message id.
//! - Fork RPC + prompt rebuild are host concerns, not modeled.

/// Max chars for ids (mirrors `session_fork_dialog::MAX_MESSAGE_ID`).
pub const MAX_ID: usize = 64;

/// Plan-then-complete fork navigation state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForkRoute {
    pub source_id: String,
    pub target: Option<String>,
}

impl ForkRoute {
    /// Plan fork from timeline message `source`; errs on empty/blank.
    pub fn plan(&mut self, source: &str) -> Result<(), String> {
        if source.trim().is_empty() {
            return Err("source must not be empty".to_string());
        }
        self.source_id = source.chars().take(MAX_ID).collect();
        self.target = None;
        Ok(())
    }

    /// Record forked session `target`; errs when no plan or empty target.
    pub fn complete(&mut self, target: &str) -> Result<(), String> {
        if self.source_id.is_empty() {
            return Err("no fork planned".to_string());
        }
        if target.trim().is_empty() {
            return Err("target must not be empty".to_string());
        }
        self.target = Some(target.chars().take(MAX_ID).collect());
        Ok(())
    }

    /// Planned navigation target, if completed.
    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }

    /// Clear plan and target.
    pub fn reset(&mut self) {
        self.source_id.clear();
        self.target = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_empty_errs() {
        let mut r = ForkRoute::default();
        assert!(r.plan("").is_err());
    }

    #[test]
    fn plan_blank_errs() {
        let mut r = ForkRoute::default();
        assert!(r.plan("   ").is_err());
    }

    #[test]
    fn complete_before_plan_errs() {
        let mut r = ForkRoute::default();
        assert!(r.complete("ses2").is_err());
    }

    #[test]
    fn roundtrip_plan_complete_target() {
        let mut r = ForkRoute::default();
        r.plan("msg1").unwrap();
        r.complete("ses2").unwrap();
        assert_eq!(r.source_id, "msg1");
        assert_eq!(r.target(), Some("ses2"));
    }

    #[test]
    fn reset_clears_plan_and_target() {
        let mut r = ForkRoute::default();
        r.plan("msg1").unwrap();
        r.complete("ses2").unwrap();
        r.reset();
        assert_eq!(r.source_id, "");
        assert_eq!(r.target(), None);
        assert!(r.complete("ses2").is_err());
    }

    #[test]
    fn ids_truncate_to_cap() {
        let mut r = ForkRoute::default();
        r.plan(&"m".repeat(100)).unwrap();
        assert_eq!(r.source_id.chars().count(), MAX_ID);
        r.complete(&"s".repeat(100)).unwrap();
        assert_eq!(r.target().unwrap().chars().count(), MAX_ID);
    }
}
