#![forbid(unsafe_code)]
//! Full plugin runtime flag (mirrors `createPluginRuntime` load/clear in TS `packages/tui/src/plugin/runtime.tsx`): loaded bit + load count.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PluginRt {
    pub loaded: bool,
    pub count: u32,
}

impl PluginRt {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(&mut self) {
        self.loaded = true;
        self.count = self.count.saturating_add(1);
    }

    pub fn unload(&mut self) {
        self.loaded = false;
    }

    #[must_use]
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state() {
        let rt = PluginRt::new();
        assert!(!rt.is_loaded());
        assert_eq!(rt.count, 0);
    }

    #[test]
    fn load_marks_loaded_and_bumps() {
        let mut rt = PluginRt::new();
        rt.load();
        assert!(rt.is_loaded());
        assert_eq!(rt.count, 1);
    }

    #[test]
    fn load_twice_bumps_twice() {
        let mut rt = PluginRt::new();
        rt.load();
        rt.load();
        assert!(rt.is_loaded());
        assert_eq!(rt.count, 2);
    }

    #[test]
    fn unload_clears_loaded_keeps_count() {
        let mut rt = PluginRt::new();
        rt.load();
        rt.unload();
        assert!(!rt.is_loaded());
        assert_eq!(rt.count, 1);
    }
}
