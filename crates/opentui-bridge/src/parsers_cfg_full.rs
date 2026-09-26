#![forbid(unsafe_code)]
//! Full parser-name registry (mirrors parsers-config.ts filetype ids).
//! DIVERGENCE: names only, no wasm/query URLs (see parsers_config.rs).

/// Ordered parser name set: cap 16 entries, each 1..=64 bytes.
#[derive(Debug, Default)]
pub struct ParsersCfg {
    names: Vec<String>,
}

impl ParsersCfg {
    #[must_use]
    pub fn new() -> Self {
        Self { names: Vec::new() }
    }

    pub fn register(&mut self, name: &str) -> bool {
        if name.is_empty() || name.len() > 64 || self.names.len() >= 16 || self.has(name) {
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
        let mut c = ParsersCfg::new();
        assert!(c.is_empty());
        assert!(c.register("rust"));
        assert!(c.has("rust"));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn rejects_dupe_empty_long() {
        let mut c = ParsersCfg::new();
        assert!(c.register("go"));
        assert!(!c.register("go") && !c.register(""));
        assert!(!c.register(&"x".repeat(65)));
        assert!(c.register(&"x".repeat(64)));
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn enforces_cap_16() {
        let mut c = ParsersCfg::new();
        for i in 0..16 {
            assert!(c.register(&format!("p{i:02}")));
        }
        assert!(!c.register("extra"));
        assert_eq!(c.len(), 16);
    }

    #[test]
    fn missing_absent() {
        assert!(!ParsersCfg::new().has("python"));
    }
}
