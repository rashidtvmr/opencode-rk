#![forbid(unsafe_code)]
//! Session route footer (mirrors `session/footer.tsx`).
//!
//! Mode label plus busy flag for the session route footer slot.

/// Session route footer state.
#[derive(Debug, Clone, Default)]
pub struct SessFooter {
    mode: String,
    busy: bool,
}

impl SessFooter {
    #[must_use]
    pub fn new(mode: &str, busy: bool) -> Self {
        let mut footer = Self::default();
        footer.set_mode(mode);
        footer.busy = busy;
        footer
    }

    pub fn set_mode(&mut self, mode: &str) {
        self.mode = mode.chars().take(32).collect();
    }

    pub fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
    }

    #[must_use]
    pub fn mode(&self) -> &str {
        &self.mode
    }

    #[must_use]
    pub const fn is_busy(&self) -> bool {
        self.busy
    }

    #[must_use]
    pub fn status(&self) -> String {
        let state = if self.busy { "busy" } else { "idle" };
        let raw = format!("{} {}", self.mode, state);
        raw.chars().take(128).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_idle_empty() {
        let f = SessFooter::default();
        assert_eq!(f.mode(), "");
        assert!(!f.is_busy());
        assert_eq!(f.status(), " idle");
    }

    #[test]
    fn mode_capped_at_32() {
        let mut f = SessFooter::default();
        f.set_mode(&"m".repeat(40));
        assert_eq!(f.mode().chars().count(), 32);
    }

    #[test]
    fn busy_status() {
        let f = SessFooter::new("prompt", true);
        assert_eq!(f.status(), "prompt busy");
    }

    #[test]
    fn status_capped_at_128() {
        let f = SessFooter::new(&"x".repeat(200), true);
        assert!(f.status().chars().count() <= 128);
    }
}
