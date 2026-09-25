#![forbid(unsafe_code)]
//! Builtin TUI plugin name registry (mirrors `createBuiltinPlugins` ids).

/// Ordered builtin name set: cap 32 entries, each 1..=64 bytes.
#[derive(Debug, Default)]
pub struct Builtins {
    names: Vec<String>,
}

impl Builtins {
    #[must_use]
    pub fn new() -> Self {
        Self { names: Vec::new() }
    }

    pub fn register(&mut self, name: &str) -> bool {
        if name.is_empty() || name.len() > 64 {
            return false;
        }
        if self.names.len() >= 32 || self.has(name) {
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
    pub fn len(&self) -> usize {
        self.names.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_has() {
        let mut b = Builtins::new();
        assert!(b.is_empty());
        assert!(b.register("home-footer"));
        assert!(b.has("home-footer"));
        assert_eq!(b.len(), 1);
    }

    #[test]
    fn rejects_duplicate() {
        let mut b = Builtins::new();
        assert!(b.register("which-key"));
        assert!(!b.register("which-key"));
        assert_eq!(b.len(), 1);
    }

    #[test]
    fn rejects_empty_and_long() {
        let mut b = Builtins::new();
        assert!(!b.register(""));
        assert!(!b.register(&"x".repeat(65)));
        assert!(b.register(&"x".repeat(64)));
        assert_eq!(b.len(), 1);
    }

    #[test]
    fn enforces_cap_32() {
        let mut b = Builtins::new();
        for i in 0..32 {
            assert!(b.register(&format!("p{i:02}")));
        }
        assert!(!b.register("extra"));
        assert_eq!(b.len(), 32);
    }

    #[test]
    fn missing_is_absent() {
        let b = Builtins::new();
        assert!(!b.has("nope"));
    }
}
