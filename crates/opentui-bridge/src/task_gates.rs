//! Foreground/background task spawn gates with fail-closed caps.
#![forbid(unsafe_code)]

/// Max concurrent tasks per kind.
pub const MAX_FOREGROUND: u32 = 16;
/// Max concurrent tasks per kind.
pub const MAX_BACKGROUND: u32 = 16;

/// Which lane a task occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    Foreground,
    Background,
}

/// Fail-closed counters for foreground and background tasks.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TaskGate {
    foreground: u32,
    background: u32,
}

impl TaskGate {
    /// Empty gate.
    pub fn new() -> Self {
        Self::default()
    }

    fn cap(kind: TaskKind) -> u32 {
        match kind {
            TaskKind::Foreground => MAX_FOREGROUND,
            TaskKind::Background => MAX_BACKGROUND,
        }
    }

    fn count(&self, kind: TaskKind) -> u32 {
        match kind {
            TaskKind::Foreground => self.foreground,
            TaskKind::Background => self.background,
        }
    }

    /// True when another task of `kind` may spawn without breaching cap.
    pub fn can_spawn(&self, kind: TaskKind) -> bool {
        self.count(kind) < Self::cap(kind)
    }

    /// Reserve one slot; fail-closed with `Err` once at cap.
    pub fn spawn(&mut self, kind: TaskKind) -> Result<u32, String> {
        if !self.can_spawn(kind) {
            return Err(format!("{kind:?} task cap reached"));
        }
        match kind {
            TaskKind::Foreground => {
                self.foreground += 1;
                Ok(self.foreground)
            }
            TaskKind::Background => {
                self.background += 1;
                Ok(self.background)
            }
        }
    }

    /// Release one slot; saturates at zero.
    pub fn finish(&mut self, kind: TaskKind) {
        match kind {
            TaskKind::Foreground => {
                self.foreground = self.foreground.saturating_sub(1);
            }
            TaskKind::Background => {
                self.background = self.background.saturating_sub(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_increments_foreground() {
        let mut g = TaskGate::new();
        assert_eq!(g.spawn(TaskKind::Foreground), Ok(1));
        assert_eq!(g.spawn(TaskKind::Foreground), Ok(2));
    }

    #[test]
    fn spawn_at_cap_errs_fail_closed() {
        let mut g = TaskGate::new();
        for _ in 0..MAX_FOREGROUND {
            assert!(g.spawn(TaskKind::Foreground).is_ok());
        }
        assert!(g.spawn(TaskKind::Foreground).is_err());
        assert_eq!(
            g.spawn(TaskKind::Foreground).unwrap_err(),
            "Foreground task cap reached"
        );
    }

    #[test]
    fn finish_saturates_at_zero() {
        let mut g = TaskGate::new();
        g.finish(TaskKind::Foreground);
        g.finish(TaskKind::Background);
        assert_eq!(g, TaskGate::new());
        assert!(g.spawn(TaskKind::Foreground).is_ok());
        g.finish(TaskKind::Foreground);
        g.finish(TaskKind::Foreground);
        assert_eq!(g, TaskGate::new());
    }

    #[test]
    fn can_spawn_false_at_cap() {
        let mut g = TaskGate::new();
        for _ in 0..MAX_BACKGROUND {
            g.spawn(TaskKind::Background).unwrap();
        }
        assert!(!g.can_spawn(TaskKind::Background));
        assert!(g.can_spawn(TaskKind::Foreground));
    }

    #[test]
    fn fg_bg_independent() {
        let mut g = TaskGate::new();
        g.spawn(TaskKind::Foreground).unwrap();
        g.spawn(TaskKind::Foreground).unwrap();
        g.spawn(TaskKind::Background).unwrap();
        g.finish(TaskKind::Foreground);
        assert_eq!(g.spawn(TaskKind::Background), Ok(2));
        assert_eq!(g.spawn(TaskKind::Foreground), Ok(2));
    }

    #[test]
    fn spawn_after_finish_recovers_slot() {
        let mut g = TaskGate::new();
        for _ in 0..MAX_FOREGROUND {
            g.spawn(TaskKind::Foreground).unwrap();
        }
        assert!(g.spawn(TaskKind::Foreground).is_err());
        g.finish(TaskKind::Foreground);
        assert!(g.can_spawn(TaskKind::Foreground));
        assert!(g.spawn(TaskKind::Foreground).is_ok());
    }
}
