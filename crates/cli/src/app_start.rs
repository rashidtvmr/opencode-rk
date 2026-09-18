#![forbid(unsafe_code)]
//! APP-001: default-launch orchestrator types.
//!
//! Decides how a no-subcommand launch proceeds and owns startup-failure
//! cleanup. Raw (interactive) mode is entered only when both stdin and
//! stdout are positively confirmed TTYs; any redirect takes the documented
//! headless path and any unknown probe fails closed to [`LaunchMode::Error`].
//! Neither headless nor error paths touch raw mode, so a redirected launch
//! can never hang waiting for terminal input.
//!
//! Failure cleanup is RAII:
//! - [`TerminalRestoreGuard`] restores terminal state on drop unless disarmed.
//! - [`PendingDescriptorGuard`] removes the temporary daemon descriptor on
//!   drop unless committed (atomic temp+rename publish means the final path
//!   is never left partial).
//!
//! Probes and cleanup actions are caller-provided (bools/closures), so this
//! module performs no TTY syscalls, filesystem access, or process control
//! itself: the integrator wires real probes/cleanup at the call site.
//!
//! Source evidence (HEAD `5af7884`):
//! - `crates/cli/src/main.rs:160-188` — no-subcommand arm calls
//!   `chat::run(&data)`; `Tui` arm calls `tui_entry::run(args)`.
//! - `crates/cli/src/chat.rs:24,67-71` — line-based stdin loop; owned daemon
//!   child killed/waited on chat exit.
//! - `crates/cli/src/tui_entry.rs:11-16` — line-based stdio transport, no TTY
//!   detection; `--once` fails closed on dead origin.
//! - `crates/server/src/daemon.rs:151-172` — atomic descriptor publish via
//!   temp file + rename (`backend.json.<pid>.tmp`).

use std::fmt;

/// Why a launch was routed to the headless path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadlessReason {
    /// stdin is redirected; interactive input cannot be read.
    StdinRedirected,
    /// stdout is redirected; interactive frames cannot be drawn.
    StdoutRedirected,
    /// Both stdin and stdout are redirected.
    BothRedirected,
}

impl fmt::Display for HeadlessReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StdinRedirected => write!(f, "stdin is redirected"),
            Self::StdoutRedirected => write!(f, "stdout is redirected"),
            Self::BothRedirected => write!(f, "stdin and stdout are redirected"),
        }
    }
}

/// Terminal/startup probe failure. Unknown TTY state fails closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchError {
    /// At least one TTY probe returned unknown; raw mode is refused.
    TerminalProbeFailed,
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TerminalProbeFailed => {
                write!(f, "could not determine terminal state; refusing raw mode")
            }
        }
    }
}

impl std::error::Error for LaunchError {}

/// Default-launch routing decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchMode {
    /// Both stdio are TTYs: the native interactive TUI may start.
    NativeTui,
    /// Redirected stdio: take the documented headless path, never raw mode.
    Headless(HeadlessReason),
    /// Probe/startup failure: report the error, never raw mode.
    Error(LaunchError),
}

/// Caller-observed TTY state. `None` means the probe itself failed
/// (unknown), which must fail closed to [`LaunchMode::Error`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TtyProbe {
    pub stdin_is_tty: Option<bool>,
    pub stdout_is_tty: Option<bool>,
}

/// Decide the launch mode. Raw mode requires positive confirmation of both
/// TTYs; anything else routes to headless or error.
#[must_use]
pub fn decide_launch_mode(probe: &TtyProbe) -> LaunchMode {
    match (probe.stdin_is_tty, probe.stdout_is_tty) {
        (Some(true), Some(true)) => LaunchMode::NativeTui,
        (Some(false), Some(false)) => LaunchMode::Headless(HeadlessReason::BothRedirected),
        (Some(false), Some(true)) => LaunchMode::Headless(HeadlessReason::StdinRedirected),
        (Some(true), Some(false)) => LaunchMode::Headless(HeadlessReason::StdoutRedirected),
        _ => LaunchMode::Error(LaunchError::TerminalProbeFailed),
    }
}

/// True only for the interactive path. Headless and error paths must never
/// enter raw mode (a redirected launch would otherwise hang).
#[must_use]
pub fn enters_raw_mode(mode: &LaunchMode) -> bool {
    matches!(mode, LaunchMode::NativeTui)
}

/// Headless exit code: distinct from success (0) so scripts can detect the
/// redirected path. Documented alongside [`headless_message`].
pub const HEADLESS_EXIT_CODE: i32 = 2;

/// Documented message for the headless path. Static text: no secrets, no
/// terminal control sequences.
#[must_use]
pub fn headless_message(reason: HeadlessReason) -> &'static str {
    match reason {
        HeadlessReason::StdinRedirected => "stdin is redirected; interactive TUI requires a terminal and raw mode is refused; pipe a prompt to the headless command or rerun under a TTY",
        HeadlessReason::StdoutRedirected => "stdout is redirected; interactive TUI requires a terminal and raw mode is refused; rerun under a TTY or use the headless/export command for piped output",
        HeadlessReason::BothRedirected => "stdin and stdout are redirected; interactive TUI requires a terminal and raw mode is refused; pipe a prompt to the headless command or rerun under a TTY",
    }
}

/// RAII terminal-restore guard. Runs `restore` on drop while armed.
/// Call [`TerminalRestoreGuard::disarm`] once the new terminal state is
/// intentionally kept (successful handoff); every failure path drops the
/// guard armed so the previous state is restored.
pub struct TerminalRestoreGuard {
    restore: Option<Box<dyn FnOnce() + Send>>,
}

impl TerminalRestoreGuard {
    /// Arm the guard with the restore action.
    pub fn new(restore: impl FnOnce() + Send + 'static) -> Self {
        Self {
            restore: Some(Box::new(restore)),
        }
    }

    /// Disarm: drop becomes a no-op (startup kept the new terminal state).
    pub fn disarm(&mut self) {
        self.restore = None;
    }

    /// True while the restore action will run on drop.
    #[must_use]
    pub fn is_armed(&self) -> bool {
        self.restore.is_some()
    }
}

impl Drop for TerminalRestoreGuard {
    fn drop(&mut self) {
        if let Some(restore) = self.restore.take() {
            restore();
        }
    }
}

/// RAII guard against partial daemon descriptors. The integrator publishes
/// via temp-file + atomic rename (see `daemon.rs:151-172`); this guard owns
/// the temp path and runs `cleanup` (remove the temp file, kill the pending
/// child) on drop unless [`PendingDescriptorGuard::commit`] disarms it after
/// a successful rename. The final descriptor path is therefore never left
/// partial by a failed startup.
pub struct PendingDescriptorGuard {
    temp_path: String,
    final_path: String,
    cleanup: Option<Box<dyn FnOnce() + Send>>,
}

impl PendingDescriptorGuard {
    /// Arm the guard over a pending publish.
    pub fn new(
        temp_path: String,
        final_path: String,
        cleanup: impl FnOnce() + Send + 'static,
    ) -> Self {
        Self {
            temp_path,
            final_path,
            cleanup: Some(Box::new(cleanup)),
        }
    }

    /// Temp path staged for atomic rename.
    #[must_use]
    pub fn temp_path(&self) -> &str {
        &self.temp_path
    }

    /// Final descriptor path (untouched until the integrator renames).
    #[must_use]
    pub fn final_path(&self) -> &str {
        &self.final_path
    }

    /// True while cleanup will run on drop.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.cleanup.is_some()
    }

    /// Commit a successful publish: disarm cleanup. Consumes the guard so
    /// the staged paths cannot be reused after rename.
    pub fn commit(mut self) {
        self.cleanup = None;
    }
}

impl Drop for PendingDescriptorGuard {
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    fn probe(stdin: Option<bool>, stdout: Option<bool>) -> TtyProbe {
        TtyProbe {
            stdin_is_tty: stdin,
            stdout_is_tty: stdout,
        }
    }

    #[test]
    fn both_ttys_launch_native_tui() {
        assert_eq!(
            decide_launch_mode(&probe(Some(true), Some(true))),
            LaunchMode::NativeTui
        );
        assert!(enters_raw_mode(&LaunchMode::NativeTui));
    }

    #[test]
    fn redirected_stdin_routes_headless_without_raw_mode() {
        let mode = decide_launch_mode(&probe(Some(false), Some(true)));
        assert_eq!(
            mode,
            LaunchMode::Headless(HeadlessReason::StdinRedirected)
        );
        assert!(!enters_raw_mode(&mode));
        assert!(headless_message(HeadlessReason::StdinRedirected).contains("raw mode is refused"));
    }

    #[test]
    fn redirected_stdout_routes_headless_without_raw_mode() {
        let mode = decide_launch_mode(&probe(Some(true), Some(false)));
        assert_eq!(
            mode,
            LaunchMode::Headless(HeadlessReason::StdoutRedirected)
        );
        assert!(!enters_raw_mode(&mode));
        assert!(headless_message(HeadlessReason::StdoutRedirected).contains("raw mode is refused"));
    }

    #[test]
    fn both_redirected_reports_both_without_raw_mode() {
        let mode = decide_launch_mode(&probe(Some(false), Some(false)));
        assert_eq!(mode, LaunchMode::Headless(HeadlessReason::BothRedirected));
        assert!(!enters_raw_mode(&mode));
        assert_eq!(HEADLESS_EXIT_CODE, 2);
    }

    #[test]
    fn unknown_probe_fails_closed_to_error_without_raw_mode() {
        for p in [
            probe(None, Some(true)),
            probe(Some(true), None),
            probe(None, None),
            probe(None, Some(false)),
            probe(Some(false), None),
        ] {
            let mode = decide_launch_mode(&p);
            assert_eq!(mode, LaunchMode::Error(LaunchError::TerminalProbeFailed), "{p:?}");
            assert!(!enters_raw_mode(&mode));
        }
    }

    #[test]
    fn restore_guard_runs_on_drop() {
        let calls = Arc::new(AtomicUsize::new(0));
        let moved = Arc::clone(&calls);
        {
            let _guard = TerminalRestoreGuard::new(move || {
                moved.fetch_add(1, Ordering::SeqCst);
            });
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn restore_guard_disarmed_runs_nothing() {
        let calls = Arc::new(AtomicUsize::new(0));
        let moved = Arc::clone(&calls);
        {
            let mut guard = TerminalRestoreGuard::new(move || {
                moved.fetch_add(1, Ordering::SeqCst);
            });
            assert!(guard.is_armed());
            guard.disarm();
            assert!(!guard.is_armed());
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn pending_descriptor_cleans_temp_on_drop_and_not_after_commit() {
        let calls = Arc::new(AtomicUsize::new(0));
        let moved = Arc::clone(&calls);
        {
            let guard = PendingDescriptorGuard::new(
                "backend.json.123.tmp".to_owned(),
                "backend.json".to_owned(),
                move || {
                    moved.fetch_add(1, Ordering::SeqCst);
                },
            );
            assert!(guard.is_pending());
            assert_eq!(guard.temp_path(), "backend.json.123.tmp");
            assert_eq!(guard.final_path(), "backend.json");
            // Drop armed: cleanup runs.
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let calls2 = Arc::new(AtomicUsize::new(0));
        let moved2 = Arc::clone(&calls2);
        {
            let guard = PendingDescriptorGuard::new(
                "backend.json.124.tmp".to_owned(),
                "backend.json".to_owned(),
                move || {
                    moved2.fetch_add(1, Ordering::SeqCst);
                },
            );
            guard.commit();
            // Drop disarmed: no cleanup.
        }
        assert_eq!(calls2.load(Ordering::SeqCst), 0);
    }
}
