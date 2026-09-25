#![forbid(unsafe_code)]
//! Adapter registry (mirrors `adapters.tsx:23` `Input` adapter keys; bounded).

/// Max names (TS unbounded array; Rust fail-closed).
pub const MAX_ADAPTS: usize = 16;
/// Max name bytes (mirrors plugin_host MAX_ID style).
pub const MAX_NAME: usize = 64;
#[derive(Debug, Default, Clone)]
pub struct AdaptReg {
    pub names: Vec<String>,
}

fn trunc(s: &str) -> String {
    let mut e = MAX_NAME.min(s.len());
    while !s.is_char_boundary(e) {
        e -= 1;
    }
    s[..e].to_string()
}

impl AdaptReg {
    #[must_use]
    pub fn new() -> Self {
        Self {
            names: Vec::with_capacity(MAX_ADAPTS),
        }
    }
    pub fn register(&mut self, n: &str) -> bool {
        if n.is_empty() || self.names.len() >= MAX_ADAPTS {
            return false;
        }
        let t = trunc(n);
        if self.names.iter().any(|x| x == &t) {
            return false;
        }
        self.names.push(t);
        true
    }
    #[must_use]
    pub fn has(&self, n: &str) -> bool {
        self.names.iter().any(|x| x == n)
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ok_has_len() {
        let mut r = AdaptReg::new();
        assert!(r.register("app"));
        assert!(r.has("app"));
        assert_eq!(r.len(), 1);
    }
    #[test]
    fn dup_empty_false() {
        let mut r = AdaptReg::new();
        assert!(r.register("k"));
        assert!(!r.register("k"));
        assert!(!r.register(""));
        assert_eq!(r.len(), 1);
    }
    #[test]
    fn cap_trunc() {
        let mut r = AdaptReg::new();
        for i in 0..MAX_ADAPTS {
            assert!(r.register(&format!("a{i}")));
        }
        assert!(!r.register("full"));
        assert_eq!(r.len(), MAX_ADAPTS);
        let mut q = AdaptReg::new();
        assert!(q.register(&"n".repeat(100)));
        assert_eq!(q.names[0].len(), MAX_NAME);
    }
}
