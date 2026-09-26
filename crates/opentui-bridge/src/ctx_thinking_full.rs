#![forbid(unsafe_code)]
//! Thinking toggle + depth (TS thinking.ts show|hide, default hide).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Thinking {
    pub on: bool,
    pub depth: u32,
}
impl Thinking {
    #[must_use]
    pub fn new() -> Self {
        Self {
            on: false,
            depth: 0,
        }
    }
    pub fn enable(&mut self) {
        self.on = true;
    }
    pub fn step(&mut self) {
        if self.on {
            self.depth = self.depth.saturating_add(1);
        }
    }
    #[must_use]
    pub fn is_on(&self) -> bool {
        self.on
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_is_off_zero() {
        let t = Thinking::new();
        assert!(!t.is_on());
        assert_eq!(t.depth, 0);
        assert_eq!(t, Thinking::default());
    }
    #[test]
    fn step_bumps_only_when_on() {
        let mut t = Thinking::new();
        t.step();
        assert_eq!(t.depth, 0);
        t.enable();
        t.step();
        t.step();
        assert!(t.is_on());
        assert_eq!(t.depth, 2);
    }
    #[test]
    fn enable_is_idempotent() {
        let mut t = Thinking::new();
        t.enable();
        t.enable();
        t.step();
        assert!(t.is_on());
        assert_eq!(t.depth, 1);
    }
}
