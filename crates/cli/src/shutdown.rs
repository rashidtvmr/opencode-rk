#![forbid(unsafe_code)]

//! Shutdown reason and terminal-restore guard (APP-009).
//!
//! Commit `5af7884`. No pre-existing `crates/cli/src/shutdown.rs`; APP-009
//! leases this path for shutdown/terminal-restore types.
//!
//! Contract: the caller supplies all side effects (raw-mode/alternate-screen
//! teardown) as a closure. This module performs no syscalls, installs no
//! signal handlers, touches no FDs. It only owns the *decision* of whether
//! restoration already happened, so double-restore and missed-restore are
//! impossible by construction.
//!
//! Panic path: [`RestoreGuard`] restores from its [`Drop`] impl, which still
//! runs during unwinding. If the process aborts (panic=abort, `SIGKILL`, power
//! loss) no userspace guard can run; the caller must accept that ceiling.
//! The restore closure itself MUST NOT panic: panicking out of `Drop` during
//! unwinding aborts the process.

use std::fmt;

/// Why the CLI is shutting down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShutdownReason {
    /// Ctrl-C / SIGINT.
    Sigint,
    /// SIGTERM / service stop / Windows console cancellation.
    Sigterm,
    /// Recoverable panic: unwinding, terminal state must be restored.
    Panic,
    /// Explicit user/service command (`stop`, `quit`).
    Explicit,
}

impl ShutdownReason {
    /// True for externally delivered signals (SIGINT/SIGTERM class).
    pub fn is_signal(self) -> bool {
        matches!(self, Self::Sigint | Self::Sigterm)
    }

    /// Stable lowercase name for logs/receipts.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sigint => "sigint",
            Self::Sigterm => "sigterm",
            Self::Panic => "panic",
            Self::Explicit => "explicit",
        }
    }
}

impl fmt::Display for ShutdownReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Proof of what happened to terminal state on shutdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestoreReceipt {
    /// What triggered the shutdown.
    pub reason: ShutdownReason,
    /// True if the restore closure ran exactly once via this guard.
    pub restored: bool,
    /// How many suspend/resume cycles preceded shutdown.
    pub suspends: u32,
}

impl RestoreReceipt {
    /// Receipt for the path where the guard ran the restore closure on drop.
    pub fn restored(reason: ShutdownReason, suspends: u32) -> Self {
        Self {
            reason,
            restored: true,
            suspends,
        }
    }

    /// Receipt for the path where the caller took over restoration and
    /// disarmed the guard, so the guard restored nothing.
    pub fn suppressed(reason: ShutdownReason, suspends: u32) -> Self {
        Self {
            reason,
            restored: false,
            suspends,
        }
    }
}

/// RAII terminal-restore guard.
///
/// Holds a caller-supplied `restore` closure and runs it at most once:
/// either on [`Drop`] (normal return or unwinding) or never, if disarmed via
/// [`RestoreGuard::commit`]/[`RestoreGuard::disarm`] because the caller
/// restored the terminal itself and holds its own receipt.
pub struct RestoreGuard<F: FnOnce()> {
    restore: Option<F>,
    reason: ShutdownReason,
    suspends: u32,
}

impl<F: FnOnce()> RestoreGuard<F> {
    /// Arm a guard. `restore` runs on drop unless disarmed.
    pub fn new(reason: ShutdownReason, restore: F) -> Self {
        Self {
            restore: Some(restore),
            reason,
            suspends: 0,
        }
    }

    /// Why this guard will report on its receipt.
    pub fn reason(&self) -> ShutdownReason {
        self.reason
    }

    /// True while the restore closure is still pending.
    pub fn is_armed(&self) -> bool {
        self.restore.is_some()
    }

    /// Suspend/resume marker: record a suspend (e.g. SIGTSTP) while armed.
    pub fn suspend(&mut self) {
        self.suspends = self.suspends.saturating_add(1);
    }

    /// How many suspends were recorded.
    pub fn suspend_count(&self) -> u32 {
        self.suspends
    }

    /// Commit: caller restored the terminal itself. Disarms the guard so
    /// drop restores nothing; returns a `restored:false` receipt.
    pub fn commit(mut self) -> RestoreReceipt {
        self.restore.take();
        RestoreReceipt::suppressed(self.reason, self.suspends)
    }

    /// Alias of [`RestoreGuard::commit`]: disarm without restoring.
    pub fn disarm(self) -> RestoreReceipt {
        self.commit()
    }
}

impl<F: FnOnce()> Drop for RestoreGuard<F> {
    fn drop(&mut self) {
        // Runs on normal return AND during unwinding (panic=unwind).
        // Cannot run on panic=abort / SIGKILL / power loss: documented ceiling.
        // The closure MUST NOT panic: panicking out of Drop aborts the process.
        if let Some(restore) = self.restore.take() {
            restore();
        }
    }
}

/// Standalone suspend/resume marker for callers that track console
/// suspend state without owning a restore closure.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SuspendMarker {
    suspended: bool,
    count: u32,
}

impl SuspendMarker {
    /// Fresh, resumed marker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark suspended (idempotent state, counting total suspends).
    pub fn suspend(&mut self) {
        self.suspended = true;
        self.count = self.count.saturating_add(1);
    }

    /// Mark resumed.
    pub fn resume(&mut self) {
        self.suspended = false;
    }

    /// True between a `suspend` and the next `resume`.
    pub fn is_suspended(&self) -> bool {
        self.suspended
    }

    /// Total suspends recorded.
    pub fn count(&self) -> u32 {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    fn counter() -> (Rc<Cell<u32>>, impl FnOnce() + Clone) {
        let c = Rc::new(Cell::new(0u32));
        let f = {
            let c = Rc::clone(&c);
            move || c.set(c.get() + 1)
        };
        // FnOnce closure that is also Clone so tests can move a copy in
        // while keeping the counter handle.
        (c, f)
    }

    #[test]
    fn drop_restores() {
        let (count, restore) = counter();
        {
            let _guard = RestoreGuard::new(ShutdownReason::Explicit, restore);
        }
        assert_eq!(count.get(), 1, "drop must run restore exactly once");
    }

    #[test]
    fn disarm_suppresses_double() {
        let (count, restore) = counter();
        {
            let guard = RestoreGuard::new(ShutdownReason::Sigint, restore);
            assert!(guard.is_armed());
            let receipt = guard.disarm();
            assert!(!receipt.restored);
            assert_eq!(receipt.reason, ShutdownReason::Sigint);
        }
        assert_eq!(count.get(), 0, "disarmed guard must not restore on drop");
    }

    #[test]
    fn commit_suppresses_drop_restore() {
        let (count, restore) = counter();
        let receipt = {
            let mut guard = RestoreGuard::new(ShutdownReason::Sigterm, restore);
            guard.suspend();
            guard.commit()
        };
        assert!(!receipt.restored);
        assert_eq!(receipt.suspends, 1);
        assert_eq!(count.get(), 0);
    }

    #[test]
    fn panic_path_restores_on_unwind() {
        let (count, restore) = counter();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = RestoreGuard::new(ShutdownReason::Panic, restore);
            panic!("simulated recoverable panic");
        }));
        assert!(result.is_err());
        assert_eq!(
            count.get(),
            1,
            "guard must restore while unwinding (panic=unwind only; \
             panic=abort/SIGKILL cannot run userspace Drop)"
        );
    }

    #[test]
    fn suspend_marker_tracks_state() {
        let mut m = SuspendMarker::new();
        assert!(!m.is_suspended());
        m.suspend();
        m.suspend();
        assert!(m.is_suspended());
        assert_eq!(m.count(), 2);
        m.resume();
        assert!(!m.is_suspended());
        assert_eq!(m.count(), 2, "resume keeps total count");
    }

    #[test]
    fn reason_classification() {
        assert!(ShutdownReason::Sigint.is_signal());
        assert!(ShutdownReason::Sigterm.is_signal());
        assert!(!ShutdownReason::Panic.is_signal());
        assert!(!ShutdownReason::Explicit.is_signal());
        assert_eq!(ShutdownReason::Sigint.as_str(), "sigint");
        assert_eq!(format!("{}", ShutdownReason::Panic), "panic");
    }

    #[test]
    fn sigint_drop_restores() {
        let (count, restore) = counter();
        {
            let guard = RestoreGuard::new(ShutdownReason::Sigint, restore);
            assert!(guard.is_armed());
            assert_eq!(guard.reason(), ShutdownReason::Sigint);
        }
        assert_eq!(count.get(), 1, "SIGINT path must restore exactly once");
    }

    #[test]
    fn sigterm_drop_restores_after_suspend() {
        let (count, restore) = counter();
        {
            let mut guard = RestoreGuard::new(ShutdownReason::Sigterm, restore);
            guard.suspend();
            assert_eq!(guard.suspend_count(), 1);
        }
        assert_eq!(
            count.get(),
            1,
            "SIGTERM path restores after suspend/resume marker"
        );
    }

    #[test]
    fn repeated_arm_drop_cycles_restore_exactly_once() {
        let (count, restore) = counter();
        drop(restore); // per-cycle closures below own their counter clone.
        for _ in 0..50u32 {
            let c = Rc::clone(&count);
            let _guard = RestoreGuard::new(ShutdownReason::Explicit, move || {
                c.set(c.get() + 1)
            });
        }
        assert_eq!(
            count.get(),
            50,
            "50 startup/shutdown cycles must restore exactly once each"
        );
    }
}
