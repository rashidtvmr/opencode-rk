//! Permission mode (auto|normal); mirrors `permission.tsx:5-23`.
//! ponytail: bool not enum; upgrade when third mode lands.
#![forbid(unsafe_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PermMode {
    auto: bool,
}
impl PermMode {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn from_auto(auto: bool) -> Self {
        Self { auto }
    }
    pub fn set_auto(&mut self, auto: bool) {
        self.auto = auto;
    }
    pub fn toggle(&mut self) {
        self.auto = !self.auto;
    }
    #[must_use]
    pub fn is_auto(&self) -> bool {
        self.auto
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_normal() {
        assert!(!PermMode::new().is_auto());
    }
    #[test]
    fn from_auto_set() {
        let mut m = PermMode::from_auto(true);
        assert!(m.is_auto());
        m.set_auto(false);
        assert!(!m.is_auto());
    }
    #[test]
    fn toggle_flips() {
        let mut m = PermMode::new();
        m.toggle();
        assert!(m.is_auto());
        m.toggle();
        assert!(!m.is_auto());
    }
}
