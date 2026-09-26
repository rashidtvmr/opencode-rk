#![forbid(unsafe_code)]
//! Plugin host state machine (bounded, std-only). No-JS boundary: state + name
//! registry only; no adapter constructs JS, no JS executes, no dynamic loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HostState {
    #[default]
    Down,
    Up,
}
#[derive(Debug, Default, Clone)]
pub struct PluginHostFull {
    state: HostState,
    count: u32,
    names: Vec<String>,
}

impl PluginHostFull {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: HostState::Down,
            count: 0,
            names: Vec::new(),
        }
    }
    pub fn boot(&mut self) {
        self.state = HostState::Up;
    }
    pub fn shutdown(&mut self) {
        self.state = HostState::Down;
        self.count = 0;
        self.names.clear();
    }
    #[must_use]
    pub const fn is_booted(&self) -> bool {
        matches!(self.state, HostState::Up)
    }
    #[must_use]
    pub const fn count(&self) -> u32 {
        self.count
    }
    pub fn register(&mut self, name: &str) -> bool {
        if !self.is_booted() || self.count >= 64 {
            return false;
        }
        if name.is_empty() || name.len() > 128 || self.names.iter().any(|n| n == name) {
            return false;
        }
        self.names.push(name.to_owned());
        self.count += 1;
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_is_down() {
        let h = PluginHostFull::new();
        assert!(!h.is_booted() && h.count() == 0);
    }
    #[test]
    fn boot_shutdown() {
        let mut h = PluginHostFull::new();
        h.boot();
        assert!(h.is_booted());
        h.shutdown();
        assert!(!h.is_booted() && h.count() == 0);
    }
    #[test]
    fn register_bool_dup() {
        let mut h = PluginHostFull::new();
        assert!(!h.register("a"));
        h.boot();
        assert!(h.register("a"));
        assert!(!h.register("a"));
        assert!(!h.register(""));
        assert_eq!(h.count(), 1);
    }
    #[test]
    fn caps_at_64() {
        let mut h = PluginHostFull::new();
        h.boot();
        for i in 0..64 {
            assert!(h.register(&format!("p{i}")));
        }
        assert!(!h.register("overflow"));
        assert_eq!(h.count(), 64);
    }
}
