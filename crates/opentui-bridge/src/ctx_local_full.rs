#![forbid(unsafe_code)]

/// Local context keys, mirrors `local.tsx` store keys.
/// ponytail: fixed caps (64 keys, 128 bytes); grow when upstream needs more.
#[derive(Debug, Default, Clone)]
pub struct CtxLocal {
    keys: Vec<String>,
}

impl CtxLocal {
    pub const MAX_KEYS: usize = 64;
    pub const MAX_LEN: usize = 128;

    pub fn new() -> Self {
        Self { keys: Vec::new() }
    }

    pub fn add(&mut self, key: &str) -> bool {
        if key.len() > Self::MAX_LEN || self.keys.len() >= Self::MAX_KEYS {
            return false;
        }
        if self.keys.iter().any(|k| k == key) {
            return false;
        }
        self.keys.push(key.to_string());
        true
    }

    pub fn has(&self, key: &str) -> bool {
        self.keys.iter().any(|k| k == key)
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_then_has() {
        let mut c = CtxLocal::new();
        assert!(c.add("theme"));
        assert!(c.has("theme"));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn dup_rejected() {
        let mut c = CtxLocal::new();
        assert!(c.add("a"));
        assert!(!c.add("a"));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn overlong_rejected() {
        let mut c = CtxLocal::new();
        assert!(!c.add(&"k".repeat(129)));
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn cap_enforced() {
        let mut c = CtxLocal::new();
        for i in 0..CtxLocal::MAX_KEYS {
            assert!(c.add(&format!("k{i}")));
        }
        assert!(!c.add("overflow"));
        assert_eq!(c.len(), CtxLocal::MAX_KEYS);
    }
}
