#![forbid(unsafe_code)]
//! Frame request / host hook / output acknowledgement state machine (FFI-04).
//!
//! EVIDENCE / BOUNDARY
//! - `docs/ui-parity/opencode-ui-parity-kit/audit-opentui-boundary.md` §2
//!   "Checked resources and frame/output protocol" (ref [N7],
//!   `packages/native/include/opentui.h:532-627` at OpenTUI pin
//!   `8d6065a3e60039d16c6ee697692dcfbc46d3c786`) states that a native frame is
//!   a *protocol*: a request ticket, preparation yields, host hook records,
//!   painting, and admitted output — and that the host must acknowledge the
//!   exact request and respect phase restrictions. Ref [N8]
//!   (`packages/core/src/NativeSession.ts:760-812`) adds the second half:
//!   writable-stream callbacks, drain/backpressure, output tickets and
//!   presentation completion are distinct from a successful draw call.
//! - The vendored library exposes only the renderer entry points
//!   (`nm -gU …/libopentui.dylib`: `createRenderer`, `render`,
//!   `getNextBuffer`, `destroyRenderer`, …). It exports **no** frame-ticket,
//!   host-hook or output-queue symbol, so this module is a Rust model of the
//!   documented protocol rather than a binding. [`crate::safe_renderer`]
//!   remains the only caller of the native `render`.
//! - OpenCode's own presentation path is asynchronous in the same way: the
//!   renderer is created with `targetFps: 60` and frames are paced by the
//!   host loop (`packages/tui/src/app.tsx:252-259`, pin `450e7b9`), and
//!   teardown awaits `renderer.idle()` before replacing terminal state
//!   (`packages/tui/src/mini/runtime.lifecycle.ts:259`).

use std::collections::VecDeque;

/// Native output queue depth before [`FrameController::admit_output`] reports
/// backpressure. Mirrors a bounded writable-stream queue: the host must drain
/// before it may admit more.
pub const MAX_OUTSTANDING_OUTPUT: u32 = 8;
/// Bytes one frame may admit in total (fail closed, no unbounded buffering).
pub const MAX_OUTPUT_BYTES_PER_FRAME: usize = 4 * 1024 * 1024;
/// Host hook records retained while a frame is being prepared.
pub const MAX_HOST_HOOKS: usize = 256;

/// The request ticket the host must echo back on every phase call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrameTicket(u64);

impl FrameTicket {
    /// Monotonic ticket value. Strictly increasing until the `u64` counter
    /// wraps (2^64 frames), at which point the host recreates the controller.
    #[must_use]
    pub const fn id(self) -> u64 {
        self.0
    }
}

/// Why the host asked for a frame. Kept so coalescing is observable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameReason {
    /// First frame after renderer creation.
    Initial,
    /// Native state changed (buffer, cursor, scene mutation).
    Dirty,
    /// Terminal geometry changed.
    Resize,
    /// Streaming content produced new bytes.
    Stream,
    /// A terminal mode / capability answer arrived.
    Protocol,
}

/// Protocol phase. `Failed` is terminal for frame requests; `Shutdown` is
/// terminal for the controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePhase {
    Idle,
    Requested,
    Prepared,
    Painting,
    OutputAdmitted,
    Presented,
    ShutdownRequested,
    Shutdown,
    Failed,
}

impl FramePhase {
    /// True once no further frame request may be accepted.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Shutdown | Self::Failed)
    }
}

/// Host hook record kinds. Upstream stages host work during preparation; the
/// model only needs the kind to prove ordering and non-reentrancy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookKind {
    PreLayout,
    PostLayout,
    PrePaint,
    PostPaint,
    /// ScrollBox position is not a signal and is observed during the pass
    /// (`packages/tui/src/component/patch-diff.tsx:161-176`, pin `450e7b9`).
    ObserveScroll,
}

/// One queued host hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostHook {
    id: u32,
    kind: HookKind,
}

impl HostHook {
    #[must_use]
    pub const fn id(self) -> u32 {
        self.id
    }

    #[must_use]
    pub const fn kind(self) -> HookKind {
        self.kind
    }
}

/// Result of [`FrameController::request`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRequest {
    ticket: FrameTicket,
    coalesced: bool,
}

impl FrameRequest {
    #[must_use]
    pub const fn ticket(self) -> FrameTicket {
        self.ticket
    }

    /// True when an earlier request was still pending and this call merged
    /// into it (at most one frame is pending, so producers cannot queue an
    /// unbounded backlog of paints).
    #[must_use]
    pub const fn coalesced(self) -> bool {
        self.coalesced
    }
}

/// Fail-closed protocol errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// Shutdown already completed; no work is accepted.
    Closed,
    /// Output failed earlier; the frame path is terminal.
    Failed,
    /// No request is in flight.
    NoRequest,
    /// A different ticket is in flight.
    TicketMismatch,
    /// The call is not legal in the current phase.
    WrongPhase {
        expected: FramePhase,
        actual: FramePhase,
    },
    /// Host hooks are still queued; call [`FrameController::take_hook`] first.
    HooksPending,
    /// Output queue is full: drain, then retry the same admission.
    Backpressured,
    /// Output was admitted but not yet drained/presented.
    OutputPending,
    /// One frame exceeded [`MAX_OUTPUT_BYTES_PER_FRAME`].
    OutputTooLarge,
    /// Hook queue is full; the new hook is refused rather than dropped.
    HookOverflow,
}

impl core::fmt::Display for FrameError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Closed => f.write_str("frame path closed"),
            Self::Failed => f.write_str("frame output failed"),
            Self::NoRequest => f.write_str("no frame request in flight"),
            Self::TicketMismatch => f.write_str("frame ticket mismatch"),
            Self::WrongPhase { expected, actual } => {
                write!(f, "frame phase {actual:?}, expected {expected:?}")
            }
            Self::HooksPending => f.write_str("host hooks are still pending"),
            Self::Backpressured => f.write_str("output queue is full"),
            Self::OutputPending => f.write_str("admitted output is not drained"),
            Self::OutputTooLarge => f.write_str("frame output exceeds byte budget"),
            Self::HookOverflow => f.write_str("host hook queue is full"),
        }
    }
}

impl std::error::Error for FrameError {}

/// Monotonic protocol counters (diagnostics only; never drive decisions).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameStats {
    pub requested: u64,
    pub coalesced: u64,
    pub hooks_run: u64,
    pub prepared: u64,
    pub painted: u64,
    pub output_admitted: u64,
    pub output_drained: u64,
    pub presented: u64,
    pub output_failures: u64,
}

/// Frame protocol controller. No IO, no clock, no allocation beyond the
/// bounded hook queue — every transition is a pure function of its inputs so
/// the host loop can be tested deterministically.
#[derive(Debug)]
pub struct FrameController {
    phase: FramePhase,
    shutting_down: bool,
    next_ticket: u64,
    ticket: Option<FrameTicket>,
    hooks: VecDeque<HostHook>,
    next_hook: u32,
    outstanding: u32,
    admitted_bytes: usize,
    stats: FrameStats,
    failure: Option<FrameError>,
}

impl Default for FrameController {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameController {
    /// Controller in [`FramePhase::Idle`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            phase: FramePhase::Idle,
            shutting_down: false,
            next_ticket: 0,
            ticket: None,
            hooks: VecDeque::new(),
            next_hook: 0,
            outstanding: 0,
            admitted_bytes: 0,
            stats: FrameStats::default(),
            failure: None,
        }
    }

    #[must_use]
    pub const fn phase(&self) -> FramePhase {
        if self.shutting_down && self.ticket.is_some() {
            FramePhase::ShutdownRequested
        } else {
            self.phase
        }
    }

    /// Ticket currently in flight, if any.
    #[must_use]
    pub const fn in_flight(&self) -> Option<FrameTicket> {
        self.ticket
    }

    /// Output tickets admitted but not yet drained.
    #[must_use]
    pub const fn outstanding_output(&self) -> u32 {
        self.outstanding
    }

    #[must_use]
    pub const fn stats(&self) -> FrameStats {
        self.stats
    }

    /// Terminal failure recorded by [`FrameController::fail_output`].
    #[must_use]
    pub const fn failure(&self) -> Option<FrameError> {
        self.failure
    }

    fn gate(&self) -> Result<(), FrameError> {
        if self.shutting_down {
            return Err(FrameError::Closed);
        }
        match self.phase {
            FramePhase::Shutdown | FramePhase::ShutdownRequested => Err(FrameError::Closed),
            FramePhase::Failed => Err(FrameError::Failed),
            _ => Ok(()),
        }
    }

    fn check(&self, ticket: FrameTicket, expected: &[FramePhase]) -> Result<(), FrameError> {
        // An in-flight frame may still run its hooks, paint, write and present
        // after shutdown was requested (`ShutdownRequested`); only new work
        // (`request`, `push_hook`) is closed off by `gate`.
        match self.phase {
            FramePhase::Shutdown => return Err(FrameError::Closed),
            FramePhase::Failed => return Err(FrameError::Failed),
            _ => {}
        }
        match self.ticket {
            None => Err(FrameError::NoRequest),
            Some(current) if current != ticket => Err(FrameError::TicketMismatch),
            Some(_) if !expected.contains(&self.phase) => Err(FrameError::WrongPhase {
                expected: expected[0],
                actual: self.phase,
            }),
            Some(_) => Ok(()),
        }
    }

    /// Ask for a frame. While one request is pending the call coalesces into
    /// it instead of queueing a second paint.
    pub fn request(&mut self, _reason: FrameReason) -> Result<FrameRequest, FrameError> {
        self.gate()?;
        if let Some(ticket) = self.ticket {
            self.stats.coalesced += 1;
            return Ok(FrameRequest {
                ticket,
                coalesced: true,
            });
        }
        self.next_ticket = self.next_ticket.wrapping_add(1);
        let ticket = FrameTicket(self.next_ticket);
        self.ticket = Some(ticket);
        self.phase = FramePhase::Requested;
        self.admitted_bytes = 0;
        self.stats.requested += 1;
        Ok(FrameRequest {
            ticket,
            coalesced: false,
        })
    }

    /// Queue a host hook record for the pending frame. Refused (not dropped)
    /// once [`MAX_HOST_HOOKS`] records are queued.
    pub fn push_hook(&mut self, kind: HookKind) -> Result<u32, FrameError> {
        self.gate()?;
        if self.ticket.is_none() {
            return Err(FrameError::NoRequest);
        }
        if self.hooks.len() >= MAX_HOST_HOOKS {
            return Err(FrameError::HookOverflow);
        }
        self.next_hook = self.next_hook.wrapping_add(1);
        let hook = HostHook {
            id: self.next_hook,
            kind,
        };
        self.hooks.push_back(hook);
        Ok(hook.id)
    }

    /// Take the next host hook record, or `None` when preparation may
    /// continue. Hooks are only drained while a frame is being prepared.
    pub fn take_hook(&mut self, ticket: FrameTicket) -> Result<Option<HostHook>, FrameError> {
        self.gate()?;
        self.check(ticket, &[FramePhase::Requested, FramePhase::Prepared])?;
        let hook = self.hooks.pop_front();
        if hook.is_some() {
            self.stats.hooks_run += 1;
        }
        Ok(hook)
    }

    /// Finish preparation. Fails closed while host hooks remain queued.
    pub fn prepare(&mut self, ticket: FrameTicket) -> Result<(), FrameError> {
        self.check(ticket, &[FramePhase::Requested])?;
        if !self.hooks.is_empty() {
            return Err(FrameError::HooksPending);
        }
        self.phase = FramePhase::Prepared;
        self.stats.prepared += 1;
        Ok(())
    }

    /// Begin painting into the writable buffer.
    pub fn paint(&mut self, ticket: FrameTicket) -> Result<(), FrameError> {
        self.check(ticket, &[FramePhase::Prepared])?;
        self.phase = FramePhase::Painting;
        self.stats.painted += 1;
        Ok(())
    }

    /// Admit `bytes` of output for the painted frame.
    ///
    /// Returns [`FrameError::Backpressured`] while
    /// [`MAX_OUTSTANDING_OUTPUT`] tickets are undrained; the phase does not
    /// advance, so the host can retry the same admission after a drain.
    pub fn admit_output(
        &mut self,
        ticket: FrameTicket,
        bytes: usize,
    ) -> Result<u32, FrameError> {
        self.check(ticket, &[FramePhase::Painting, FramePhase::OutputAdmitted])?;
        if self.outstanding >= MAX_OUTSTANDING_OUTPUT {
            return Err(FrameError::Backpressured);
        }
        let total = self.admitted_bytes.saturating_add(bytes);
        if total > MAX_OUTPUT_BYTES_PER_FRAME {
            return Err(FrameError::OutputTooLarge);
        }
        self.admitted_bytes = total;
        self.outstanding += 1;
        self.phase = FramePhase::OutputAdmitted;
        self.stats.output_admitted += 1;
        Ok(self.outstanding)
    }

    /// Report that `count` admitted output tickets finished writing.
    /// Saturating: a spurious drain can never underflow the counter.
    pub fn drain_output(&mut self, count: u32) -> u32 {
        let drained = count.min(self.outstanding);
        self.outstanding -= drained;
        self.stats.output_drained += u64::from(drained);
        self.outstanding
    }

    /// Complete presentation of the frame. Requires every admitted output
    /// ticket to be drained first.
    pub fn present(&mut self, ticket: FrameTicket) -> Result<(), FrameError> {
        self.check(ticket, &[FramePhase::OutputAdmitted])?;
        if self.outstanding != 0 {
            return Err(FrameError::OutputPending);
        }
        // Presentation is the last step of an in-flight frame, so it also
        // finishes a pending shutdown request.
        self.phase = if self.shutting_down {
            FramePhase::Shutdown
        } else {
            FramePhase::Presented
        };
        self.ticket = None;
        self.admitted_bytes = 0;
        self.stats.presented += 1;
        Ok(())
    }

    /// Record a terminal output failure (write error, closed stream). The
    /// frame path becomes [`FramePhase::Failed`]; every later [`request`]
    /// returns [`FrameError::Failed`] until [`request_shutdown`] runs.
    ///
    /// [`request`]: FrameController::request
    /// [`request_shutdown`]: FrameController::request_shutdown
    pub fn fail_output(&mut self) -> FrameError {
        self.phase = FramePhase::Failed;
        self.ticket = None;
        self.hooks.clear();
        self.stats.output_failures += 1;
        self.failure = Some(FrameError::Failed);
        FrameError::Failed
    }

    /// Begin shutdown. An in-flight frame is allowed to finish first
    /// ([`FramePhase::ShutdownRequested`]); otherwise shutdown completes
    /// immediately. New frame requests are refused from this point on.
    pub fn request_shutdown(&mut self) -> FramePhase {
        self.hooks.clear();
        self.shutting_down = true;
        if self.ticket.is_none() {
            self.phase = FramePhase::Shutdown;
        }
        self.phase()
    }

    /// Finish shutdown after the in-flight frame presented (or was failed).
    /// Fails closed while a ticket is still in flight.
    pub fn complete_shutdown(&mut self) -> Result<(), FrameError> {
        if self.shutting_down {
            if self.ticket.is_some() {
                return Err(FrameError::OutputPending);
            }
            self.phase = FramePhase::Shutdown;
            return Ok(());
        }
        match self.phase {
            FramePhase::Shutdown => Ok(()),
            FramePhase::Failed => {
                self.phase = FramePhase::Shutdown;
                Ok(())
            }
            _ => Err(FrameError::WrongPhase {
                expected: FramePhase::Shutdown,
                actual: self.phase,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controller_at_painting() -> (FrameController, FrameTicket) {
        let mut frame = FrameController::new();
        let ticket = frame.request(FrameReason::Initial).unwrap().ticket();
        frame.prepare(ticket).unwrap();
        frame.paint(ticket).unwrap();
        (frame, ticket)
    }

    #[test]
    fn happy_path_is_a_strict_phase_sequence() {
        let mut frame = FrameController::new();
        assert_eq!(frame.phase(), FramePhase::Idle);
        let request = frame.request(FrameReason::Initial).unwrap();
        assert!(!request.coalesced());
        let ticket = request.ticket();
        assert_eq!(frame.phase(), FramePhase::Requested);
        assert_eq!(frame.in_flight(), Some(ticket));
        assert_eq!(frame.take_hook(ticket), Ok(None));

        frame.prepare(ticket).unwrap();
        assert_eq!(frame.phase(), FramePhase::Prepared);
        frame.paint(ticket).unwrap();
        assert_eq!(frame.phase(), FramePhase::Painting);
        assert_eq!(frame.admit_output(ticket, 128), Ok(1));
        assert_eq!(frame.phase(), FramePhase::OutputAdmitted);
        assert_eq!(frame.drain_output(1), 0);
        frame.present(ticket).unwrap();
        assert_eq!(frame.phase(), FramePhase::Presented);
        assert_eq!(frame.in_flight(), None);
        assert_eq!(
            frame.stats(),
            FrameStats {
                requested: 1,
                prepared: 1,
                painted: 1,
                output_admitted: 1,
                output_drained: 1,
                presented: 1,
                ..FrameStats::default()
            }
        );
        // A presented controller accepts the next frame with a new ticket.
        let next = frame.request(FrameReason::Dirty).unwrap();
        assert!(next.ticket() > ticket);
    }

    #[test]
    fn requests_coalesce_while_one_is_pending() {
        let mut frame = FrameController::new();
        let first = frame.request(FrameReason::Initial).unwrap();
        let second = frame.request(FrameReason::Dirty).unwrap();
        let third = frame.request(FrameReason::Stream).unwrap();
        assert_eq!(second.ticket(), first.ticket());
        assert_eq!(third.ticket(), first.ticket());
        assert!(second.coalesced() && third.coalesced());
        assert_eq!(frame.stats().requested, 1);
        assert_eq!(frame.stats().coalesced, 2);
    }

    #[test]
    fn out_of_order_calls_are_rejected_without_state_change() {
        let (mut frame, ticket) = controller_at_painting();
        // Painting twice is illegal; there is no implicit restart.
        assert_eq!(
            frame.paint(ticket),
            Err(FrameError::WrongPhase {
                expected: FramePhase::Prepared,
                actual: FramePhase::Painting
            })
        );
        let mut frame2 = FrameController::new();
        let t2 = frame2.request(FrameReason::Dirty).unwrap().ticket();
        assert_eq!(
            frame2.paint(t2),
            Err(FrameError::WrongPhase {
                expected: FramePhase::Prepared,
                actual: FramePhase::Requested
            })
        );
        assert_eq!(frame2.phase(), FramePhase::Requested);
        assert_eq!(
            frame2.admit_output(t2, 1),
            Err(FrameError::WrongPhase {
                expected: FramePhase::Painting,
                actual: FramePhase::Requested
            })
        );
        assert_eq!(frame2.stats().painted, 0);
    }

    #[test]
    fn ticket_mismatch_and_missing_request_fail_closed() {
        let (mut frame, ticket) = controller_at_painting();
        let stale = FrameTicket(ticket.id() + 1);
        assert_eq!(frame.paint(stale), Err(FrameError::TicketMismatch));
        assert_eq!(frame.admit_output(stale, 1), Err(FrameError::TicketMismatch));
        assert_eq!(frame.present(stale), Err(FrameError::TicketMismatch));

        let mut empty = FrameController::new();
        assert_eq!(empty.take_hook(FrameTicket(1)), Err(FrameError::NoRequest));
        assert_eq!(empty.prepare(FrameTicket(1)), Err(FrameError::NoRequest));
        assert_eq!(empty.push_hook(HookKind::PrePaint), Err(FrameError::NoRequest));
    }

    #[test]
    fn host_hooks_must_drain_before_prepare() {
        let mut frame = FrameController::new();
        let ticket = frame.request(FrameReason::Resize).unwrap().ticket();
        let first = frame.push_hook(HookKind::PreLayout).unwrap();
        frame.push_hook(HookKind::PostLayout).unwrap();
        frame.push_hook(HookKind::ObserveScroll).unwrap();
        assert_eq!(frame.prepare(ticket), Err(FrameError::HooksPending));
        assert_eq!(frame.phase(), FramePhase::Requested);

        let hook = frame.take_hook(ticket).unwrap().expect("first hook");
        assert_eq!(hook.id(), first);
        assert_eq!(hook.kind(), HookKind::PreLayout);
        assert_eq!(frame.take_hook(ticket).unwrap().unwrap().kind(), HookKind::PostLayout);
        assert_eq!(frame.take_hook(ticket).unwrap().unwrap().kind(), HookKind::ObserveScroll);
        assert_eq!(frame.take_hook(ticket), Ok(None));
        frame.prepare(ticket).unwrap();
        assert_eq!(frame.stats().hooks_run, 3);
    }

    #[test]
    fn host_hook_queue_is_bounded() {
        let mut frame = FrameController::new();
        let ticket = frame.request(FrameReason::Stream).unwrap().ticket();
        for _ in 0..MAX_HOST_HOOKS {
            frame.push_hook(HookKind::PrePaint).unwrap();
        }
        assert_eq!(
            frame.push_hook(HookKind::PrePaint),
            Err(FrameError::HookOverflow)
        );
        // The existing records are intact and still drain in order.
        assert_eq!(frame.take_hook(ticket).unwrap().map(HostHook::id), Some(1));
    }

    #[test]
    fn output_backpressure_blocks_admission_until_drained() {
        let (mut frame, ticket) = controller_at_painting();
        for expected in 1..=MAX_OUTSTANDING_OUTPUT {
            // The native frame writes chunks; every chunk is admitted in the
            // painting/admitted phase and consumes one output ticket.
            assert_eq!(frame.admit_output(ticket, 16), Ok(expected));
        }
        assert_eq!(
            frame.admit_output(ticket, 16),
            Err(FrameError::Backpressured)
        );
        // Backpressure never advances the phase or the counters.
        assert_eq!(frame.stats().output_admitted, u64::from(MAX_OUTSTANDING_OUTPUT));
        assert_eq!(frame.outstanding_output(), MAX_OUTSTANDING_OUTPUT);
        // Draining frees exactly one slot; the retry then succeeds.
        assert_eq!(frame.drain_output(1), MAX_OUTSTANDING_OUTPUT - 1);
        assert_eq!(frame.admit_output(ticket, 16), Ok(MAX_OUTSTANDING_OUTPUT));
    }

    #[test]
    fn present_requires_every_output_ticket_to_drain() {
        let (mut frame, ticket) = controller_at_painting();
        frame.admit_output(ticket, 10).unwrap();
        assert_eq!(frame.present(ticket), Err(FrameError::OutputPending));
        assert_eq!(frame.phase(), FramePhase::OutputAdmitted);
        // Over-draining saturates instead of underflowing.
        assert_eq!(frame.drain_output(5), 0);
        assert_eq!(frame.stats().output_drained, 1);
        frame.present(ticket).unwrap();
    }

    #[test]
    fn per_frame_output_byte_budget_fails_closed() {
        let (mut frame, ticket) = controller_at_painting();
        assert_eq!(
            frame.admit_output(ticket, MAX_OUTPUT_BYTES_PER_FRAME + 1),
            Err(FrameError::OutputTooLarge)
        );
        assert_eq!(frame.phase(), FramePhase::Painting);
        assert_eq!(frame.outstanding_output(), 0);
        assert_eq!(
            frame.admit_output(ticket, MAX_OUTPUT_BYTES_PER_FRAME),
            Ok(1)
        );
    }

    #[test]
    fn output_failure_is_terminal_for_frames_but_not_for_shutdown() {
        let (mut frame, ticket) = controller_at_painting();
        assert_eq!(frame.fail_output(), FrameError::Failed);
        assert_eq!(frame.phase(), FramePhase::Failed);
        assert_eq!(frame.failure(), Some(FrameError::Failed));
        assert_eq!(frame.stats().output_failures, 1);
        assert_eq!(frame.request(FrameReason::Dirty).unwrap_err(), FrameError::Failed);
        assert_eq!(frame.push_hook(HookKind::PrePaint), Err(FrameError::Failed));
        assert_eq!(frame.prepare(ticket), Err(FrameError::Failed));
        assert!(frame.phase().is_terminal());
        // Teardown still succeeds from a failed frame path.
        assert_eq!(frame.request_shutdown(), FramePhase::Shutdown);
        assert_eq!(frame.complete_shutdown(), Ok(()));
    }

    #[test]
    fn shutdown_waits_for_the_in_flight_frame() {
        let (mut frame, ticket) = controller_at_painting();
        assert_eq!(frame.request_shutdown(), FramePhase::ShutdownRequested);
        // No new work and no new hooks once shutdown is requested.
        assert_eq!(frame.request(FrameReason::Dirty).unwrap_err(), FrameError::Closed);
        assert_eq!(frame.push_hook(HookKind::PrePaint), Err(FrameError::Closed));
        // The in-flight frame still completes its admitted output.
        assert_eq!(frame.admit_output(ticket, 4), Ok(1));
        assert_eq!(frame.complete_shutdown(), Err(FrameError::OutputPending));
        frame.drain_output(1);
        frame.present(ticket).unwrap();
        assert_eq!(frame.complete_shutdown(), Ok(()));
        assert_eq!(frame.phase(), FramePhase::Shutdown);
        assert!(frame.phase().is_terminal());
    }

    #[test]
    fn idle_shutdown_is_immediate_and_idempotent() {
        let mut frame = FrameController::new();
        assert_eq!(frame.request_shutdown(), FramePhase::Shutdown);
        assert_eq!(frame.complete_shutdown(), Ok(()));
        assert_eq!(frame.request(FrameReason::Initial).unwrap_err(), FrameError::Closed);
        assert_eq!(frame.request_shutdown(), FramePhase::Shutdown);
    }

    #[test]
    fn hooks_are_cleared_by_shutdown() {
        let mut frame = FrameController::new();
        frame.request(FrameReason::Stream).unwrap();
        frame.push_hook(HookKind::PostPaint).unwrap();
        frame.request_shutdown();
        let ticket = FrameTicket(1);
        assert_eq!(frame.take_hook(ticket), Err(FrameError::Closed));
        assert_eq!(frame.phase(), FramePhase::ShutdownRequested);
    }
}
