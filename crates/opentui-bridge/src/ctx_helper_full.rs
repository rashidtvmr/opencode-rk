//! Helper readiness gate; mirrors helper.tsx show-gate.
#![forbid(unsafe_code)]
/// Max name chars kept.
pub const MAX_NAME_LEN: usize = 64;
/// Named readiness gate; not ready by default.
#[derive(Debug, Clone, Default)]
pub struct Helper {
    name: String,
    ready: bool,
}
impl Helper {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Set name, truncating to [`MAX_NAME_LEN`] chars.
    pub fn set_name(&mut self, name: &str) {
        self.name = if name.chars().count() > MAX_NAME_LEN {
            name.chars().take(MAX_NAME_LEN).collect()
        } else {
            name.to_string()
        };
    }
    /// Set readiness flag.
    pub fn set_ready(&mut self, ready: bool) {
        self.ready = ready;
    }
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready
    }
    #[must_use]
    pub fn name_of(&self) -> &str {
        &self.name
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_empty_not_ready() {
        let h = Helper::new();
        assert_eq!(h.name_of(), "");
        assert!(!h.is_ready());
    }
    #[test]
    fn set_name_caps_at_64() {
        let mut h = Helper::new();
        h.set_name(&"x".repeat(MAX_NAME_LEN + 10));
        assert_eq!(h.name_of().chars().count(), MAX_NAME_LEN);
    }
    #[test]
    fn set_ready_toggles() {
        let mut h = Helper::new();
        h.set_ready(true);
        assert!(h.is_ready());
        h.set_ready(false);
        assert!(!h.is_ready());
    }
}
