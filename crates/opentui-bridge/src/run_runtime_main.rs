#![forbid(unsafe_code)]
//! Runtime orchestrator state.
//!
//! Mirrors `packages/opencode/src/cli/cmd/run/runtime.ts`
//! (`runInteractiveRuntime` started/turn/error bookkeeping).

/// Max retained error chars for [`RunMain::last_error`].
pub const ERROR_CAP: usize = 512;

/// Minimal run orchestrator state: started flag, saturating turn
/// counter, and last (512-char capped) error.
#[derive(Debug, Clone, Default)]
pub struct RunMain {
    started: bool,
    turns: u32,
    last_error: Option<String>,
}

/// Fresh started runtime. Mirrors `boot()` in `run_runtime.rs`.
pub fn start() -> RunMain {
    RunMain::start()
}

impl RunMain {
    /// Fresh started runtime with zero turns and no error.
    pub fn start() -> Self {
        Self {
            started: true,
            turns: 0,
            last_error: None,
        }
    }

    /// Whether the runtime has started.
    pub fn started(&self) -> bool {
        self.started
    }

    /// Turn count (saturates at `u32::MAX`).
    pub fn turn_count(&self) -> u32 {
        self.turns
    }

    /// Last error, if any (capped at [`ERROR_CAP`] chars).
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Record one turn, saturating at `u32::MAX`.
    pub fn record_turn(&mut self) {
        self.turns = self.turns.saturating_add(1);
    }

    /// Record an error, truncating to [`ERROR_CAP`] chars.
    pub fn record_error(&mut self, msg: &str) {
        let kept: String = msg.chars().take(ERROR_CAP).collect();
        self.last_error = Some(kept);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_fresh() {
        let r = RunMain::start();
        assert!(r.started());
        assert_eq!(r.turn_count(), 0);
        assert_eq!(r.last_error(), None);
    }

    #[test]
    fn zero_turns() {
        let r = start();
        assert_eq!(r.turn_count(), 0);
        assert!(r.started());
    }

    #[test]
    fn turns_saturate() {
        let mut r = RunMain::start();
        r.turns = u32::MAX - 1;
        r.record_turn();
        assert_eq!(r.turn_count(), u32::MAX);
        r.record_turn();
        assert_eq!(r.turn_count(), u32::MAX);
    }

    #[test]
    fn error_truncates() {
        let mut r = RunMain::start();
        let long = "e".repeat(ERROR_CAP + 100);
        r.record_error(&long);
        assert_eq!(r.last_error().unwrap().len(), ERROR_CAP);
    }

    #[test]
    fn error_overwrites() {
        let mut r = RunMain::start();
        r.record_error("first");
        r.record_error("second");
        assert_eq!(r.last_error(), Some("second"));
    }

    #[test]
    fn turn_increments() {
        let mut r = RunMain::start();
        r.record_turn();
        r.record_turn();
        assert_eq!(r.turn_count(), 2);
    }
}

// --- BRIDGE-GAP-108 additions (append-only; lines above frozen) ---

/// Turn phase for the run-queue orchestrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TurnState {
    #[default]
    Idle,
    Running,
    WaitingPermission,
    WaitingQuestion,
    Done,
}

/// Point-in-time snapshot: current turn, queued prompts, interrupt flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuntimeStatus {
    pub turn: TurnState,
    pub queue_depth: usize,
    pub interrupted: bool,
}

impl RuntimeStatus {
    /// Fresh idle status with empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queued prompt count (pushes counter snapshot).
    pub fn queue_depth(&self) -> usize {
        self.queue_depth
    }

    /// Interrupt: flag set, turn forced back to idle.
    pub fn interrupt(&mut self) {
        self.interrupted = true;
        self.turn = TurnState::Idle;
    }

    /// Record one queued prompt, saturating.
    pub fn push(&mut self) {
        self.queue_depth = self.queue_depth.saturating_add(1);
    }

    /// True while blocked on permission or a question.
    pub fn is_waiting(&self) -> bool {
        matches!(
            self.turn,
            TurnState::WaitingPermission | TurnState::WaitingQuestion
        )
    }
}

/// Bounded prompt-queue counter backing [`RuntimeStatus::queue_depth`].
#[derive(Debug, Clone, Default)]
pub struct RuntimeQueue2 {
    depth: usize,
    cap: usize,
}

impl RuntimeQueue2 {
    /// Empty queue with `cap` bound (`0` = unbounded).
    pub fn new(cap: usize) -> Self {
        Self { depth: 0, cap }
    }

    /// Queue capacity bound (`0` = unbounded).
    pub fn capacity(&self) -> usize {
        self.cap
    }

    /// Current depth.
    pub fn queue_depth(&self) -> usize {
        self.depth
    }

    /// Enqueue one prompt; saturates at `cap` when bounded.
    pub fn push(&mut self) {
        if self.cap == 0 {
            self.depth = self.depth.saturating_add(1);
        } else if self.depth < self.cap {
            self.depth += 1;
        }
    }

    /// Dequeue one prompt; saturates at zero.
    pub fn pop(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// Snapshot as [`RuntimeStatus`] with caller-supplied turn/flag.
    pub fn status(&self, turn: TurnState, interrupted: bool) -> RuntimeStatus {
        RuntimeStatus {
            turn,
            queue_depth: self.depth,
            interrupted,
        }
    }
}

/// Lifetime turn counters: completed vs interrupted.
#[derive(Debug, Clone, Default)]
pub struct TurnTracker {
    completed: u32,
    interrupted_count: u32,
}

impl TurnTracker {
    /// Fresh zeroed counters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one completed turn, saturating at `u32::MAX`.
    pub fn complete_turn(&mut self) {
        self.completed = self.completed.saturating_add(1);
    }

    /// Record one interrupt, saturating at `u32::MAX`.
    pub fn interrupt_count(&mut self) {
        self.interrupted_count = self.interrupted_count.saturating_add(1);
    }

    /// `(completed, interrupted_count)` totals.
    pub fn totals(&self) -> (u32, u32) {
        (self.completed, self.interrupted_count)
    }
}

#[cfg(test)]
mod tests2 {
    use super::*;

    #[test]
    fn turn_default_idle() {
        assert_eq!(TurnState::default(), TurnState::Idle);
        assert_eq!(RuntimeStatus::new().turn, TurnState::Idle);
    }

    #[test]
    fn status_interrupt_clears_turn() {
        let mut s = RuntimeStatus {
            turn: TurnState::Running,
            queue_depth: 2,
            interrupted: false,
        };
        s.interrupt();
        assert!(s.interrupted);
        assert_eq!(s.turn, TurnState::Idle);
    }

    #[test]
    fn status_queue_depth_pushes() {
        let mut s = RuntimeStatus::new();
        assert_eq!(s.queue_depth(), 0);
        s.push();
        s.push();
        assert_eq!(s.queue_depth(), 2);
    }

    #[test]
    fn queue2_cap_bounds() {
        let mut q = RuntimeQueue2::new(2);
        q.push();
        q.push();
        q.push();
        assert_eq!(q.queue_depth(), 2);
        q.pop();
        q.pop();
        q.pop();
        assert_eq!(q.queue_depth(), 0);
        assert_eq!(q.capacity(), 2);
    }

    #[test]
    fn tracker_totals_saturate() {
        let mut t = TurnTracker::new();
        t.complete_turn();
        t.complete_turn();
        t.interrupt_count();
        assert_eq!(t.totals(), (2, 1));
    }

    #[test]
    fn tracker_counts() {
        let mut t = TurnTracker {
            completed: u32::MAX,
            interrupted_count: u32::MAX,
        };
        t.complete_turn();
        t.interrupt_count();
        assert_eq!(t.totals(), (u32::MAX, u32::MAX));
    }

    #[test]
    fn status_from_queue() {
        let mut q = RuntimeQueue2::new(0);
        q.push();
        let s = q.status(TurnState::WaitingPermission, false);
        assert_eq!(s.queue_depth(), 1);
        assert!(s.is_waiting());
        assert!(!s.interrupted);
    }
}
