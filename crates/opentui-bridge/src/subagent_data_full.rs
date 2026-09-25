//! Full subagent progress row (TS `run/subagent-data.ts` read-only ref).
//! [`SubagentDataFull`] is id + capped task + 0..=100 progress with label.

#![forbid(unsafe_code)]

/// Max id chars.
pub const ID_CAP: usize = 64;
/// Max task chars.
pub const TASK_CAP: usize = 512;
/// Max task chars shown in [`SubagentDataFull::label`].
pub const LABEL_TASK_CAP: usize = 50;

fn trunc(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect()
}

/// Subagent id + task + progress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentDataFull {
    id: String,
    task: String,
    progress: u8,
}

impl SubagentDataFull {
    /// New row; id capped 64, task capped 512, progress clamped via reject.
    #[must_use]
    pub fn new(id: &str, task: &str, progress: u8) -> Self {
        Self {
            id: trunc(id, ID_CAP),
            task: trunc(task, TASK_CAP),
            progress: progress.min(100),
        }
    }
    /// Row id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Row task.
    #[must_use]
    pub fn task(&self) -> &str {
        &self.task
    }
    /// Progress 0..=100.
    #[must_use]
    pub fn progress(&self) -> u8 {
        self.progress
    }
    /// Set progress; false when >100.
    pub fn set_progress(&mut self, v: u8) -> bool {
        if v > 100 {
            return false;
        }
        self.progress = v;
        true
    }
    /// Replace task, capped 512.
    pub fn update_task(&mut self, task: &str) {
        self.task = trunc(task, TASK_CAP);
    }
    /// `"id pct% task-abbrev50"`.
    #[must_use]
    pub fn label(&self) -> String {
        format!(
            "{} {}% {}",
            self.id,
            self.progress,
            trunc(&self.task, LABEL_TASK_CAP)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn caps_on_new() {
        let s = SubagentDataFull::new(&"i".repeat(70), &"t".repeat(600), 20);
        assert_eq!(s.id().chars().count(), ID_CAP);
        assert_eq!(s.task().chars().count(), TASK_CAP);
    }
    #[test]
    fn set_progress_bounds() {
        let mut s = SubagentDataFull::new("a", "t", 10);
        assert!(s.set_progress(100));
        assert_eq!(s.progress(), 100);
        assert!(!s.set_progress(101));
        assert_eq!(s.progress(), 100);
    }
    #[test]
    fn label_parts() {
        let s = SubagentDataFull::new("agent1", "do work", 42);
        assert_eq!(s.label(), "agent1 42% do work");
    }
    #[test]
    fn label_abbrev50() {
        let s = SubagentDataFull::new("a", &"t".repeat(80), 5);
        assert_eq!(s.label(), format!("a 5% {}", "t".repeat(50)));
    }
    #[test]
    fn update_task_trunc() {
        let mut s = SubagentDataFull::new("a", "t", 0);
        s.update_task(&"x".repeat(600));
        assert_eq!(s.task().chars().count(), TASK_CAP);
    }
    #[test]
    fn new_clamps_progress() {
        let s = SubagentDataFull::new("a", "t", 250);
        assert_eq!(s.progress(), 100);
    }
}
