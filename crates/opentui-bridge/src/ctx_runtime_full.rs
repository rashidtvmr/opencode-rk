#![forbid(unsafe_code)]
//! CtxRuntime: model id plus busy flag.
//! Mirror of `packages/tui/src/context/runtime.tsx` explicit provider state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CtxRuntime {
    pub model: String,
    pub busy: bool,
}
impl CtxRuntime {
    pub fn new() -> Self {
        Self {
            model: String::new(),
            busy: false,
        }
    }
    pub fn set_model(&mut self, m: &str) {
        self.model = m.chars().take(128).collect();
    }
    pub fn set_busy(&mut self, b: bool) {
        self.busy = b;
    }
    pub fn status(&self) -> String {
        let s = if self.busy {
            format!("{}:busy", self.model)
        } else {
            format!("{}:idle", self.model)
        };
        s.chars().take(256).collect()
    }
}
impl Default for CtxRuntime {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_idle() {
        let c = CtxRuntime::default();
        assert!(c.model.is_empty());
        assert!(!c.busy);
    }
    #[test]
    fn model_caps() {
        let mut c = CtxRuntime::new();
        c.set_model(&"x".repeat(200));
        assert_eq!(c.model.chars().count(), 128);
    }
    #[test]
    fn busy_status_caps() {
        let mut c = CtxRuntime::new();
        c.set_model("gpt");
        c.set_busy(true);
        assert_eq!(c.status(), "gpt:busy");
        c.set_model(&"y".repeat(200));
        assert!(c.status().chars().count() <= 256);
    }
}
