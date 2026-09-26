#![forbid(unsafe_code)]
//! Boot sequence for direct interactive mode.
//!
//! Mirrors `packages/opencode/src/cli/cmd/run/runtime.boot.ts`
//! (config/model/session/diff resolution before first frame).

/// Ordered boot steps; must advance in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootStep {
    Init,
    LoadConfig,
    ConnectDaemon,
    Ready,
}

/// Bounded boot log with failure latch.
#[derive(Debug, Default)]
pub struct BootLog {
    steps: Vec<(BootStep, String)>,
    failed: bool,
}

/// Max recorded steps.
pub const BOOT_LOG_CAP: usize = 32;
/// Max stored note length.
pub const NOTE_CAP: usize = 256;

fn truncate(note: &str) -> String {
    let mut s = note.to_string();
    if s.len() > NOTE_CAP {
        s.truncate(NOTE_CAP);
    }
    s
}

impl BootLog {
    /// Empty log, not failed.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a step; returns false once failed.
    pub fn advance(&mut self, step: BootStep, note: &str) -> bool {
        if self.failed {
            return false;
        }
        if self.steps.len() < BOOT_LOG_CAP {
            self.steps.push((step, truncate(note)));
        }
        true
    }

    /// Latch failure with note; further advance blocked.
    pub fn fail(&mut self, note: &str) {
        if self.steps.len() < BOOT_LOG_CAP {
            let last = self.steps.last().map(|(s, _)| *s).unwrap_or(BootStep::Init);
            self.steps.push((last, truncate(note)));
        }
        self.failed = true;
    }

    /// Ready only when Ready reached and no failure.
    pub fn is_ready(&self) -> bool {
        !self.failed && self.steps.iter().any(|(s, _)| *s == BootStep::Ready)
    }

    /// Recorded steps.
    pub fn steps(&self) -> &[(BootStep, String)] {
        &self.steps
    }

    /// Whether boot failed.
    pub fn failed(&self) -> bool {
        self.failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_ok() {
        let mut log = BootLog::new();
        assert!(log.advance(BootStep::Init, "start"));
        assert!(log.advance(BootStep::LoadConfig, "cfg"));
        assert_eq!(log.steps().len(), 2);
    }

    #[test]
    fn fail_blocks_advance() {
        let mut log = BootLog::new();
        log.fail("boom");
        assert!(log.failed());
        assert!(!log.advance(BootStep::Ready, "late"));
    }

    #[test]
    fn ready_gate() {
        let mut log = BootLog::new();
        log.advance(BootStep::Init, "a");
        log.advance(BootStep::LoadConfig, "b");
        log.advance(BootStep::ConnectDaemon, "c");
        assert!(!log.is_ready());
        log.advance(BootStep::Ready, "d");
        assert!(log.is_ready());
    }

    #[test]
    fn ready_blocked_after_fail() {
        let mut log = BootLog::new();
        log.advance(BootStep::Ready, "ok");
        log.fail("late err");
        assert!(!log.is_ready());
    }

    #[test]
    fn cap_32() {
        let mut log = BootLog::new();
        for i in 0..40 {
            log.advance(BootStep::Init, &format!("n{i}"));
        }
        assert_eq!(log.steps().len(), BOOT_LOG_CAP);
    }

    #[test]
    fn note_truncates_256() {
        let mut log = BootLog::new();
        log.advance(BootStep::Init, &"x".repeat(300));
        assert_eq!(log.steps()[0].1.len(), NOTE_CAP);
    }
}
