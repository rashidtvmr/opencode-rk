#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl ThemeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ThemeMode::Dark => "dark",
            ThemeMode::Light => "light",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeEngine {
    name: String,
    mode: ThemeMode,
    locked: bool,
}

impl Default for ThemeEngine {
    fn default() -> Self {
        Self {
            name: "opencode".to_string(),
            mode: ThemeMode::Dark,
            locked: false,
        }
    }
}

impl ThemeEngine {
    pub fn new(name: &str, mode: ThemeMode) -> Self {
        let mut e = Self::default();
        e.set_mode(mode);
        let _ = e.apply(name);
        e
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn mode(&self) -> ThemeMode {
        self.mode
    }

    pub fn locked(&self) -> bool {
        self.locked
    }

    pub fn apply(&mut self, name: &str) -> bool {
        if self.locked {
            return false;
        }
        let mut n: String = name.chars().take(64).collect();
        if n.is_empty() {
            n.push_str("opencode");
        }
        // ponytail: no validation against registry; add when theme list wired.
        self.name = n;
        true
    }

    pub fn set_mode(&mut self, mode: ThemeMode) {
        self.mode = mode;
    }

    pub fn toggle_lock(&mut self) {
        self.locked = !self.locked;
    }

    pub fn label(&self) -> String {
        if self.locked {
            format!("{} {} locked", self.name, self.mode.as_str())
        } else {
            format!("{} {}", self.name, self.mode.as_str())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_ok() {
        let mut e = ThemeEngine::default();
        assert!(e.apply("tokyo-night"));
        assert_eq!(e.name(), "tokyo-night");
    }

    #[test]
    fn locked_blocks_apply() {
        let mut e = ThemeEngine::default();
        e.toggle_lock();
        assert!(!e.apply("other"));
        assert_eq!(e.name(), "opencode");
    }

    #[test]
    fn truncates_to_64() {
        let mut e = ThemeEngine::default();
        let long = "x".repeat(100);
        assert!(e.apply(&long));
        assert_eq!(e.name().len(), 64);
    }

    #[test]
    fn label_parts() {
        let e = ThemeEngine::new("opencode", ThemeMode::Light);
        assert_eq!(e.label(), "opencode light");
        let mut d = ThemeEngine::default();
        assert_eq!(d.label(), "opencode dark");
        d.toggle_lock();
        assert!(d.label().contains("locked"));
    }

    #[test]
    fn unlock_allows_apply() {
        let mut e = ThemeEngine::default();
        e.toggle_lock();
        assert!(!e.apply("a"));
        e.toggle_lock();
        assert!(e.apply("b"));
        assert_eq!(e.name(), "b");
    }

    #[test]
    fn set_mode_roundtrip() {
        let mut e = ThemeEngine::default();
        e.set_mode(ThemeMode::Light);
        assert_eq!(e.mode(), ThemeMode::Light);
        e.set_mode(ThemeMode::Dark);
        assert_eq!(e.mode(), ThemeMode::Dark);
    }
}
