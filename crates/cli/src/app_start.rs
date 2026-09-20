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
//! Source evidence (HEAD `95bb156`):
//! - `crates/cli/src/main.rs:187-192` — no-subcommand arm calls
//!   `chat::run(&data)`; `Tui` arm calls `tui_entry::run(args)`.
//! - `crates/cli/src/chat.rs:45-73` — line-based stdin loop; owned daemon
//!   child killed/waited on chat exit.
//! - `crates/cli/src/tui_entry.rs:601-656` — line fallback vs `--native`
//!   renderer; `--once` fails closed on dead origin.
//! - `crates/cli/src/daemon_client.rs:706-735` — `try_become_owner`
//!   election (one owner, rest attachers) and credential-bound
//!   `decide_lifecycle_authed` (reuse only with validated descriptor +
//!   healthy probe; occupied port refuses, never kills, sends nothing).
//! - `crates/cli/src/native_app.rs:63-109` — shell views
//!   (Empty/Loading/Offline/Error/Actionable, only Actionable interactive).
//! - `crates/cli/src/onboarding.rs:413-592` — in-app provider setup
//!   session; cancel leaves no half-authorized account.
//! - `crates/cli/src/shutdown.rs:90-154` — RAII `RestoreGuard` terminal
//!   restore decision (no syscalls).
//! - `crates/server/src/daemon.rs:151-172` — atomic descriptor publish via
//!   temp file + rename (`backend.json.<pid>.tmp`).
//!
//! Remainder contract (APP-001 card): [`plan_default_launch`] routes the
//! whole no-subcommand decision — TTY mode from [`decide_launch_mode`],
//! startup role from [`DaemonPresence`], first view from the credential
//! flag — so a fresh-HOME PTY opens the native view as owner with no
//! serve/session command, a second terminal attaches, missing credentials
//! open in-app [`StartupView::Setup`], and redirected stdio yields a
//! headless/error plan with no role and no view. [`PendingStartup`]
//! couples the two RAII guards so a failed startup restores the terminal
//! and cleans the temp descriptor/child, while a committed startup cleans
//! nothing.

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

/// Caller-validated daemon state feeding [`plan_default_launch`].
/// Validation (schema/PID/loopback/owner/symlink/bearer) stays in
/// `daemon_client`; this enum only routes an already-checked outcome.
/// A refused foreign occupant never reaches the plan: the caller surfaces
/// `daemon_client::LifecycleAction::RefuseOccupiedPort` on the error path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonPresence {
    /// No usable descriptor and no healthy probe: this launch owns the start.
    Absent,
    /// Stale descriptor or dead probe: this launch owns the (re)start.
    Stale,
    /// Validated descriptor plus healthy probe: attach, never start over it.
    Reusable,
}

/// Startup role for a TTY launch. Exactly one launch becomes
/// [`LaunchRole::Owner`]; every later terminal is [`LaunchRole::Attacher`].
/// Off-TTY plans carry no role so a redirected launch can never start a
/// daemon it cannot interact with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchRole {
    Owner,
    Attacher,
}

/// First view the native shell must open once the daemon link is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupView {
    /// Authenticated daemon: the normal session view.
    Main,
    /// Missing credentials: the in-app provider setup flow
    /// (`onboarding`), never a trace or manual server instructions.
    Setup,
}

/// Whole default-launch decision: TTY mode, startup role, first view.
/// Headless/error modes always carry `role: None, view: None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultLaunch {
    pub mode: LaunchMode,
    pub role: Option<LaunchRole>,
    pub view: Option<StartupView>,
}

/// Route a no-subcommand launch. `creds_configured`: `Some(true)` means a
/// usable provider credential exists; `Some(false)`/`None` (unknown) routes
/// a TTY launch to the in-app setup view. Non-TTY launches ignore daemon
/// and credential state: headless/error with no role and no view, so a
/// redirected launch can never start a daemon or hang in raw mode. A refused
/// foreign occupant never reaches this plan: the caller surfaces
/// `daemon_client::LifecycleAction::RefuseOccupiedPort` on the error path.
#[must_use]
pub fn plan_default_launch(
    probe: &TtyProbe,
    presence: DaemonPresence,
    creds_configured: Option<bool>,
) -> DefaultLaunch {
    let mode = decide_launch_mode(probe);
    if mode != LaunchMode::NativeTui {
        return DefaultLaunch {
            mode,
            role: None,
            view: None,
        };
    }
    let role = match presence {
        DaemonPresence::Absent | DaemonPresence::Stale => LaunchRole::Owner,
        DaemonPresence::Reusable => LaunchRole::Attacher,
    };
    let view = match creds_configured {
        Some(true) => StartupView::Main,
        Some(false) | None => StartupView::Setup,
    };
    DefaultLaunch {
        mode,
        role: Some(role),
        view: Some(view),
    }
}

/// True when the plan is an interactive owner startup: the native TUI mode
/// with [`LaunchRole::Owner`]. Testable branch predicate so the no-subcommand
/// call site can route Owner vs Attacher without re-matching `plan.role`.
/// Fails closed: any non-native plan (even a hand-built struct) is false.
#[must_use]
pub fn is_owner_startup(plan: &DefaultLaunch) -> bool {
    plan.mode == LaunchMode::NativeTui && plan.role == Some(LaunchRole::Owner)
}

/// True when the plan is an interactive launch that must open the in-app
/// provider setup flow: the native TUI mode with [`StartupView::Setup`].
/// Covers missing (`Some(false)`) and unknown (`None`) credentials, which
/// [`plan_default_launch`] both route to `Setup`. Fails closed off-TTY.
#[must_use]
pub fn needs_setup(plan: &DefaultLaunch) -> bool {
    plan.mode == LaunchMode::NativeTui && plan.view == Some(StartupView::Setup)
}

/// Documented message for the in-app setup view. Static text: no secrets,
/// no terminal control sequences, no manual server instructions.
#[must_use]
pub fn setup_message() -> &'static str {
    "no provider credentials are configured; opening in-app setup to configure a provider or local endpoint"
}

/// Joint startup-failure guard: an armed terminal restore plus a pending
/// descriptor publish. [`PendingStartup::fail`] drops both armed (terminal
/// restored, temp descriptor/child cleaned); [`PendingStartup::commit`]
/// disarms both after a successful publish.
pub struct PendingStartup {
    terminal: TerminalRestoreGuard,
    descriptor: PendingDescriptorGuard,
}

impl PendingStartup {
    pub fn new(terminal: TerminalRestoreGuard, descriptor: PendingDescriptorGuard) -> Self {
        Self {
            terminal,
            descriptor,
        }
    }

    /// Fail the startup: drop both guards armed (terminal restored, temp
    /// descriptor/child cleaned). Consumes the guard so the staged paths
    /// cannot be reused after the failure.
    pub fn fail(self) {
        drop(self);
    }

    /// Commit a successful startup: disarm both guards (keep the new
    /// terminal state and the published descriptor); clean nothing.
    /// Consumes the guard so the staged paths cannot be reused after the
    /// rename.
    pub fn commit(mut self) {
        self.terminal.disarm();
        self.descriptor.commit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
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
        assert_eq!(mode, LaunchMode::Headless(HeadlessReason::StdinRedirected));
        assert!(!enters_raw_mode(&mode));
        assert!(headless_message(HeadlessReason::StdinRedirected).contains("raw mode is refused"));
    }

    #[test]
    fn redirected_stdout_routes_headless_without_raw_mode() {
        let mode = decide_launch_mode(&probe(Some(true), Some(false)));
        assert_eq!(mode, LaunchMode::Headless(HeadlessReason::StdoutRedirected));
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
            assert_eq!(
                mode,
                LaunchMode::Error(LaunchError::TerminalProbeFailed),
                "{p:?}"
            );
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

    // --- APP-001 remainder RED (frozen before GREEN) ---

    fn tty() -> TtyProbe {
        TtyProbe {
            stdin_is_tty: Some(true),
            stdout_is_tty: Some(true),
        }
    }

    fn pending_pair(calls: &Arc<AtomicUsize>) -> PendingStartup {
        let t = Arc::clone(calls);
        let d = Arc::clone(calls);
        PendingStartup::new(
            TerminalRestoreGuard::new(move || {
                t.fetch_add(1, Ordering::SeqCst);
            }),
            PendingDescriptorGuard::new(
                "backend.json.7.tmp".to_owned(),
                "backend.json".to_owned(),
                move || {
                    d.fetch_add(1, Ordering::SeqCst);
                },
            ),
        )
    }

    #[test]
    fn app001_t1_fresh_home_pty_opens_native_as_owner() {
        // Fresh HOME under PTY: no daemon yet, so this launch owns the
        // start and opens the native view — with no serve/session command.
        let plan = plan_default_launch(&tty(), DaemonPresence::Absent, Some(true));
        assert_eq!(plan.mode, LaunchMode::NativeTui);
        assert!(enters_raw_mode(&plan.mode));
        assert_eq!(plan.role, Some(LaunchRole::Owner));
        assert_eq!(plan.view, Some(StartupView::Main));
    }

    #[test]
    fn app001_t2_second_terminal_attaches_same_daemon() {
        // Reusable daemon: second terminal attaches, never starts a
        // second store owner.
        let plan = plan_default_launch(&tty(), DaemonPresence::Reusable, Some(true));
        assert_eq!(plan.mode, LaunchMode::NativeTui);
        assert_eq!(plan.role, Some(LaunchRole::Attacher));
        assert_eq!(plan.view, Some(StartupView::Main));
        let stale = plan_default_launch(&tty(), DaemonPresence::Stale, Some(true));
        assert_eq!(stale.role, Some(LaunchRole::Owner));
    }

    #[test]
    fn app001_t3_missing_creds_opens_in_app_setup_not_trace() {
        // Missing (or unknown) credentials open in-app setup, never a
        // stack trace or manual server instructions.
        for creds in [Some(false), None] {
            let plan = plan_default_launch(&tty(), DaemonPresence::Reusable, creds);
            assert_eq!(plan.mode, LaunchMode::NativeTui);
            assert_eq!(plan.role, Some(LaunchRole::Attacher));
            assert_eq!(plan.view, Some(StartupView::Setup), "{creds:?}");
        }
        let text = setup_message();
        assert!(text.contains("setup"));
        assert!(!text.contains("Traceback") && !text.contains("traceback"));
        assert!(
            !text.contains("serve"),
            "must not name manual server commands"
        );
    }

    #[test]
    fn app001_t4_redirected_stdio_never_raw_never_owner() {
        // Redirected stdio takes the headless/error path and never hangs
        // in raw mode; off-TTY plans carry no startup role and no view.
        for p in [
            probe(Some(false), Some(false)),
            probe(Some(false), Some(true)),
            probe(Some(true), Some(false)),
            probe(None, Some(true)),
            probe(Some(true), None),
            probe(None, None),
        ] {
            let plan = plan_default_launch(&p, DaemonPresence::Absent, Some(true));
            assert!(!enters_raw_mode(&plan.mode), "{p:?}");
            assert_ne!(plan.mode, LaunchMode::NativeTui, "{p:?}");
            assert_eq!(
                plan.role, None,
                "redirected launch must never own/start: {p:?}"
            );
            assert_eq!(plan.view, None, "redirected launch opens no view: {p:?}");
        }
    }

    #[test]
    fn app001_t5_startup_failure_restores_and_cleans() {
        // Failure drops armed: terminal restored + temp descriptor/child
        // cleaned (2 actions); success commits: nothing cleaned.
        let calls = Arc::new(AtomicUsize::new(0));
        pending_pair(&calls).fail();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let kept = Arc::new(AtomicUsize::new(0));
        pending_pair(&kept).commit();
        assert_eq!(kept.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn none_arm_headless_both_non_tty_carries_no_role_or_view() {
        let plan = plan_default_launch(
            &probe(Some(false), Some(false)),
            DaemonPresence::Absent,
            Some(true),
        );
        assert_eq!(plan.mode, LaunchMode::Headless(HeadlessReason::BothRedirected));
        assert!(!enters_raw_mode(&plan.mode));
        assert_eq!(plan.role, None);
        assert_eq!(plan.view, None);
        assert_eq!(HEADLESS_EXIT_CODE, 2);
        assert!(headless_message(HeadlessReason::BothRedirected).contains("raw mode is refused"));
    }

    #[test]
    fn none_arm_error_probe_passes_through_with_no_role_or_view() {
        for p in [probe(None, None), probe(None, Some(true)), probe(Some(true), None)] {
            let plan = plan_default_launch(&p, DaemonPresence::Reusable, Some(true));
            assert_eq!(plan.mode, LaunchMode::Error(LaunchError::TerminalProbeFailed), "{p:?}");
            assert!(!enters_raw_mode(&plan.mode), "{p:?}");
            assert_eq!(plan.role, None, "{p:?}");
            assert_eq!(plan.view, None, "{p:?}");
        }
    }

    #[test]
    fn none_arm_native_tty_routes_owner_main() {
        let plan = plan_default_launch(&tty(), DaemonPresence::Absent, Some(true));
        assert_eq!(plan.mode, LaunchMode::NativeTui);
        assert!(enters_raw_mode(&plan.mode));
        assert_eq!(plan.role, Some(LaunchRole::Owner));
        assert_eq!(plan.view, Some(StartupView::Main));
    }
}
