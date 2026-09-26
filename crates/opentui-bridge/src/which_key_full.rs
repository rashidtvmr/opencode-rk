#![forbid(unsafe_code)]
//! Which-key pending-key overlay (port of `which-key.tsx` toggle/panel).

/// Max pending keys held.
pub const KEY_CAP: usize = 32;
/// Max chars per key label.
pub const LEN_CAP: usize = 64;

fn trunc(s: &str, cap: usize) -> String {
    s.chars().take(cap).collect()
}

/// Pending-key overlay: capped key list plus visibility flag.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WhichKey {
    pub keys: Vec<String>,
    pub visible: bool,
}

impl WhichKey {
    /// Empty hidden overlay.
    pub fn new() -> Self {
        Self::default()
    }
    /// Push key label; ignores beyond 32.
    pub fn add(&mut self, key: String) {
        if self.keys.len() >= KEY_CAP {
            return;
        }
        self.keys.push(trunc(&key, LEN_CAP));
    }
    /// Show overlay.
    pub fn show(&mut self) {
        self.visible = true;
    }
    /// Hide overlay.
    pub fn hide(&mut self) {
        self.visible = false;
    }
    /// Visible rows capped by `max`; empty when hidden.
    pub fn visible_lines(&self, max: usize) -> Vec<String> {
        if !self.visible {
            return Vec::new();
        }
        self.keys.iter().take(max).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn add_caps() {
        let mut w = WhichKey::new();
        for i in 0..40 {
            w.add(format!("k{i}"));
        }
        assert_eq!(w.keys.len(), KEY_CAP);
    }
    #[test]
    fn truncates_long() {
        let mut w = WhichKey::new();
        w.add("x".repeat(100));
        assert_eq!(w.keys[0].chars().count(), LEN_CAP);
    }
    #[test]
    fn hidden_gives_empty() {
        let mut w = WhichKey::new();
        w.add("a".into());
        assert!(w.visible_lines(8).is_empty());
    }
    #[test]
    fn shown_respects_max() {
        let mut w = WhichKey::new();
        w.add("a".into());
        w.add("b".into());
        w.show();
        assert_eq!(w.visible_lines(1), vec!["a".to_string()]);
        w.hide();
        assert!(w.visible_lines(8).is_empty());
    }
}
