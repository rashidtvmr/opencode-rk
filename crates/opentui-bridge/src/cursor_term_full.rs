#![forbid(unsafe_code)]
//! Cursor/color/clear/suspend ops pending wiring (FIX-31).
//!
//! Unwired native ops (renderer.rs:46-114): `setCursorPosition`,
//! `setBackgroundColor`, `setCursorColor`, `clearTerminal`, `suspendRenderer`,
//! `resumeRenderer`, `getBuildOptions`, `setTerminalEnvVar`.
//! Wired, excluded here: title/resize/mouse/kitty/close.
//! `draw_text` pins white fg (safe_renderer.rs:426-432); no bg/cursor plumbing.

/// Terminal op not yet routed through the permission broker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermOp {
    SetCursor(u16, u16),
    SetBg(u8),
    Clear,
    Suspend,
    Resume,
}

/// Number of terminal op variants.
#[must_use]
pub const fn op_count() -> usize {
    5
}

/// True until every [`TermOp`] is wired to its native op.
#[must_use]
pub const fn needs_wiring() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_count_matches_variants() {
        assert_eq!(op_count(), 5);
    }

    #[test]
    fn wiring_still_pending() {
        assert!(needs_wiring());
    }

    #[test]
    fn ops_clone_and_compare() {
        assert_eq!(TermOp::SetCursor(1, 2), TermOp::SetCursor(1, 2));
        assert_ne!(TermOp::Clear, TermOp::Suspend);
        assert_eq!(TermOp::SetBg(7), TermOp::SetBg(7));
        assert_eq!(TermOp::Resume, TermOp::Resume);
    }
}
