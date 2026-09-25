#![forbid(unsafe_code)]
//! Idempotent spinner registry (mirrors `register-spinner.ts:4` registerOpencodeSpinner).

pub const MAX_SPINNERS: usize = 16;
pub const MAX_NAME_LEN: usize = 64;

#[derive(Debug, Clone, Default)]
pub struct SpinnerReg {
    names: Vec<String>,
}

impl SpinnerReg {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, name: &str) -> bool {
        if name.is_empty() || name.len() > MAX_NAME_LEN {
            return false;
        }
        if self.names.iter().any(|n| n == name) {
            return false;
        }
        if self.names.len() >= MAX_SPINNERS {
            return false;
        }
        self.names.push(name.to_string());
        true
    }
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.names.iter().any(|n| n == name)
    }
    #[must_use]
    pub fn count(&self) -> usize {
        self.names.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn register_and_has() {
        let mut r = SpinnerReg::new();
        assert!(r.register("spinner"));
        assert!(r.has("spinner"));
        assert_eq!(r.count(), 1);
    }
    #[test]
    fn duplicate_rejected() {
        let mut r = SpinnerReg::new();
        assert!(r.register("a"));
        assert!(!r.register("a"));
        assert_eq!(r.count(), 1);
    }
    #[test]
    fn bounds_enforced() {
        let mut r = SpinnerReg::new();
        assert!(!r.register(""));
        assert!(!r.register(&"x".repeat(65)));
        for i in 0..MAX_SPINNERS {
            assert!(r.register(&format!("s{i}")));
        }
        assert!(!r.register("overflow"));
        assert_eq!(r.count(), MAX_SPINNERS);
    }
}
