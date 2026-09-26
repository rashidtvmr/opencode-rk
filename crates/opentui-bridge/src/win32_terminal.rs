#![forbid(unsafe_code)]
//! Win32 input-flush + Ctrl+C guard plan/state (mirrors
//! `packages/tui/src/terminal-win32.ts`).
//!
//! TS checkout at /home/rashid/projects/opencode commit a0d9b6c (NOT the
//! pinned 95daf90; paths/lines cited below are against a0d9b6c).
//! Evidence: terminal-win32.ts:47 (`win32FlushInputBuffer` via
//! `FlushConsoleInputBuffer`); terminal-win32.ts:69
//! (`win32InstallCtrlCGuard`: `setRawMode` hook + 100ms poll keeping
//! processed-input cleared).
//!
//! No win32 syscalls here: this is the portable plan/state layer. The actual
//! mode clear lives in [`crate::terminal::win32_disable_processed_input`]
//! (terminal.rs:48,64; reused, not redefined). Host-side (Windows) wiring
//! performs `FlushConsoleInputBuffer` / `SetConsoleMode` against
//! [`FlushPlan`] / [`CtrlCGuard`] state.
//!
//! `cfg(windows)` divergence: types are portable and compile everywhere;
//! only [`is_supported`] differs (true on Windows, false elsewhere).

use crate::terminal::{TerminalError, win32_disable_processed_input};

/// Pending console-input bytes to discard (plan for `FlushConsoleInputBuffer`,
/// terminal-win32.ts:47).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FlushPlan {
    pub pending_bytes: u32,
}

impl FlushPlan {
    #[must_use]
    pub const fn new(pending_bytes: u32) -> Self {
        Self { pending_bytes }
    }

    /// Queue bytes (saturating).
    pub fn queue(&mut self, n: u32) {
        self.pending_bytes = self.pending_bytes.saturating_add(n);
    }

    #[must_use]
    pub const fn is_drained(&self) -> bool {
        self.pending_bytes == 0
    }

    /// Drain the buffer. Returns true when drained; idempotent (a drained
    /// plan stays drained and keeps returning true).
    pub fn flush(&mut self) -> bool {
        self.pending_bytes = 0;
        true
    }
}

/// Guard keeping `ENABLE_PROCESSED_INPUT` cleared (state for
/// `win32InstallCtrlCGuard`, terminal-win32.ts:69). `poll_count` evidences
/// the poll loop ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CtrlCGuard {
    pub installed: bool,
    pub poll_count: u64,
}

impl CtrlCGuard {
    #[must_use]
    pub const fn new() -> Self {
        Self { installed: false, poll_count: 0 }
    }

    /// Install the guard (idempotent).
    pub fn install(&mut self) {
        self.installed = true;
    }

    /// One poll tick: keeps the guard alive, evidencing the loop via
    /// `poll_count`. No-op before install.
    pub fn poll(&mut self) -> bool {
        if !self.installed {
            return false;
        }
        self.poll_count = self.poll_count.saturating_add(1);
        true
    }
}

/// Re-clear processed input then record one guard tick. The mode clear is
/// [`win32_disable_processed_input`]; the tick evidences the poll backstop
/// (terminal-win32.ts:84-89 `enforce` + 100ms interval).
pub fn enforce(guard: &mut CtrlCGuard) -> Result<(), TerminalError> {
    win32_disable_processed_input()?;
    guard.poll();
    Ok(())
}

/// True only on Windows hosts. Types above stay portable; syscalls are
/// host-side.
#[cfg(windows)]
#[must_use]
pub const fn is_supported() -> bool {
    true
}

/// Non-Windows builds carry the plan/state types but perform no syscalls.
#[cfg(not(windows))]
#[must_use]
pub const fn is_supported() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flush_drains_pending() {
        let mut plan = FlushPlan::new(512);
        assert!(plan.flush());
        assert_eq!(plan.pending_bytes, 0);
        assert!(plan.is_drained());
    }

    #[test]
    fn flush_idempotent_when_empty() {
        let mut plan = FlushPlan::default();
        assert!(plan.flush());
        assert!(plan.flush());
        assert!(plan.is_drained());
    }

    #[test]
    fn queue_saturates_and_flush_clears() {
        let mut plan = FlushPlan::new(u32::MAX);
        plan.queue(1);
        assert_eq!(plan.pending_bytes, u32::MAX);
        assert!(plan.flush());
        assert!(plan.is_drained());
    }

    #[test]
    fn guard_poll_counts_only_when_installed() {
        let mut guard = CtrlCGuard::new();
        assert!(!guard.poll());
        assert_eq!(guard.poll_count, 0);
        guard.install();
        assert!(guard.poll());
        assert!(guard.poll());
        assert_eq!(guard.poll_count, 2);
    }

    #[test]
    fn guard_install_idempotent_keeps_count() {
        let mut guard = CtrlCGuard::new();
        guard.install();
        guard.install();
        assert!(guard.installed);
        guard.poll();
        assert_eq!(guard.poll_count, 1);
    }

    #[test]
    fn enforce_errs_unsupported_off_windows() {
        let mut guard = CtrlCGuard::new();
        guard.install();
        #[cfg(not(windows))]
        assert_eq!(enforce(&mut guard), Err(TerminalError::Unsupported));
        #[cfg(windows)]
        let _ = enforce(&mut guard);
    }

    #[test]
    fn unsupported_off_windows() {
        #[cfg(not(windows))]
        assert!(!is_supported());
        #[cfg(windows)]
        assert!(is_supported());
    }
}
