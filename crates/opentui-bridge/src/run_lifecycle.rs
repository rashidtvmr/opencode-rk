#![forbid(unsafe_code)]
//! Turn lifecycle machine.
//!
//! Mirrors `runtime.lifecycle.ts` boot/ready/busy/idle/close phases
//! (see `crates/opentui-bridge/src/run_runtime.rs` for queue/shutdown side).

/// Lifecycle phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifePhase {
    Boot,
    Ready,
    Busy,
    Idle,
    Closed,
}

/// Turn lifecycle with saturating turn counter.
#[derive(Debug)]
pub struct Lifecycle {
    phase: LifePhase,
    turns: u32,
}

/// Fresh lifecycle in `Ready` phase.
pub fn boot_ok() -> Lifecycle {
    Lifecycle {
        phase: LifePhase::Ready,
        turns: 0,
    }
}

impl Lifecycle {
    pub fn phase(&self) -> LifePhase {
        self.phase
    }
    pub fn turns(&self) -> u32 {
        self.turns
    }
    /// Ready|Idle -> Busy.
    pub fn begin_turn(&mut self) -> bool {
        match self.phase {
            LifePhase::Ready | LifePhase::Idle => {
                self.phase = LifePhase::Busy;
                true
            }
            _ => false,
        }
    }
    /// Busy -> Idle, turns+1 saturating.
    pub fn end_turn(&mut self) -> bool {
        if self.phase != LifePhase::Busy {
            return false;
        }
        self.phase = LifePhase::Idle;
        self.turns = self.turns.saturating_add(1);
        true
    }
    /// Terminal close, once.
    pub fn close(&mut self) -> bool {
        if self.phase == LifePhase::Closed {
            return false;
        }
        self.phase = LifePhase::Closed;
        true
    }
    pub fn is_closed(&self) -> bool {
        self.phase == LifePhase::Closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_is_ready() {
        let lc = boot_ok();
        assert_eq!(lc.phase(), LifePhase::Ready);
        assert_eq!(lc.turns(), 0);
    }

    #[test]
    fn begin_turn_ok() {
        let mut lc = boot_ok();
        assert!(lc.begin_turn());
        assert_eq!(lc.phase(), LifePhase::Busy);
    }

    #[test]
    fn begin_turn_busy_false() {
        let mut lc = boot_ok();
        assert!(lc.begin_turn());
        assert!(!lc.begin_turn());
        assert_eq!(lc.phase(), LifePhase::Busy);
    }

    #[test]
    fn end_turn_counts() {
        let mut lc = boot_ok();
        assert!(lc.begin_turn());
        assert!(lc.end_turn());
        assert_eq!(lc.phase(), LifePhase::Idle);
        assert_eq!(lc.turns(), 1);
        assert!(lc.begin_turn());
        assert!(lc.end_turn());
        assert_eq!(lc.turns(), 2);
    }

    #[test]
    fn close_once() {
        let mut lc = boot_ok();
        assert!(lc.close());
        assert!(!lc.close());
        assert!(lc.is_closed());
    }

    #[test]
    fn closed_is_terminal() {
        let mut lc = boot_ok();
        assert!(lc.close());
        assert!(!lc.begin_turn());
        assert!(!lc.end_turn());
        assert!(!lc.close());
        assert_eq!(lc.phase(), LifePhase::Closed);
    }
}

/// Why a lifecycle closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseReason {
    Done,
    Error(String),
    Interrupted,
}

/// Close-aware lifecycle wrapper tracking a single terminal reason.
#[derive(Debug, Clone, Default)]
pub struct Lifecycle2 {
    closed: bool,
    reason: Option<CloseReason>,
}

impl Lifecycle2 {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn is_closed(&self) -> bool {
        self.closed
    }
    /// Close once; stores reason. False if already closed.
    pub fn close_with(&mut self, reason: CloseReason) -> bool {
        if self.closed {
            return false;
        }
        let reason = match reason {
            CloseReason::Error(s) => {
                let mut t = s;
                // ponytail: char-boundary cap; byte slicing would panic on multibyte.
                while t.len() > 256 {
                    t.pop();
                }
                CloseReason::Error(t)
            }
            other => other,
        };
        self.reason = Some(reason);
        self.closed = true;
        true
    }
    pub fn close_reason(&self) -> Option<&CloseReason> {
        self.reason.as_ref()
    }
    pub fn reason_label(&self) -> &'static str {
        match &self.reason {
            None => "open",
            Some(CloseReason::Done) => "done",
            Some(CloseReason::Error(_)) => "error",
            Some(CloseReason::Interrupted) => "interrupted",
        }
    }
}

#[cfg(test)]
mod tests2 {
    use super::*;

    #[test]
    fn open_default() {
        let lc = Lifecycle2::new();
        assert!(!lc.is_closed());
        assert_eq!(lc.close_reason(), None);
        assert_eq!(lc.reason_label(), "open");
    }

    #[test]
    fn close_done_once() {
        let mut lc = Lifecycle2::new();
        assert!(lc.close_with(CloseReason::Done));
        assert!(lc.is_closed());
        assert_eq!(lc.close_reason(), Some(&CloseReason::Done));
        assert_eq!(lc.reason_label(), "done");
    }

    #[test]
    fn second_close_false_keeps_first() {
        let mut lc = Lifecycle2::new();
        assert!(lc.close_with(CloseReason::Done));
        assert!(!lc.close_with(CloseReason::Interrupted));
        assert_eq!(lc.close_reason(), Some(&CloseReason::Done));
        assert_eq!(lc.reason_label(), "done");
    }

    #[test]
    fn error_cap_256() {
        let mut lc = Lifecycle2::new();
        let long = "e".repeat(300);
        assert!(lc.close_with(CloseReason::Error(long)));
        match lc.close_reason() {
            Some(CloseReason::Error(s)) => assert_eq!(s.len(), 256),
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(lc.reason_label(), "error");
    }

    #[test]
    fn interrupted_label() {
        let mut lc = Lifecycle2::new();
        assert!(lc.close_with(CloseReason::Interrupted));
        assert_eq!(lc.reason_label(), "interrupted");
    }

    #[test]
    fn default_is_open() {
        let lc = Lifecycle2::default();
        assert!(!lc.is_closed());
        assert_eq!(lc.reason_label(), "open");
    }
}
