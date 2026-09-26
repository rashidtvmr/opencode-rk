#![forbid(unsafe_code)]
//! Two-press quit gate (std-only).
//! First `request()` arms, second quits; `cancel()` disarms.

/// Gate requiring two quit presses to exit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuitGate {
    confirming: bool,
}

impl QuitGate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// First call arms (false), second quits (true).
    pub fn request(&mut self) -> bool {
        if self.confirming {
            return true;
        }
        self.confirming = true;
        false
    }

    #[must_use]
    pub fn armed(&self) -> bool {
        self.confirming
    }

    /// Disarm pending quit.
    pub fn cancel(&mut self) {
        self.confirming = false;
    }

    /// True only for `"quit"` while armed.
    #[must_use]
    pub fn quit_on(&self, label: &str) -> bool {
        label == "quit" && self.confirming
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_arms_second_quits() {
        let mut g = QuitGate::new();
        assert!(!g.request());
        assert!(g.armed());
        assert!(g.request());
    }

    #[test]
    fn cancel_disarms() {
        let mut g = QuitGate::new();
        g.request();
        g.cancel();
        assert!(!g.armed());
        assert!(!g.request());
    }

    #[test]
    fn quit_on_quit_when_armed() {
        let mut g = QuitGate::new();
        assert!(!g.quit_on("quit"));
        g.request();
        assert!(g.quit_on("quit"));
    }

    #[test]
    fn quit_on_rejects_other_labels() {
        let mut g = QuitGate::new();
        g.request();
        assert!(!g.quit_on("exit"));
        assert!(!g.quit_on(""));
        assert!(!g.quit_on("QUIT"));
    }

    #[test]
    fn quit_on_false_after_cancel() {
        let mut g = QuitGate::new();
        g.request();
        g.cancel();
        assert!(!g.quit_on("quit"));
    }
}
