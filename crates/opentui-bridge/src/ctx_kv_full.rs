#![forbid(unsafe_code)]

//! Bounded bool KV (TUI `kv.tsx` signal toggles projection).
//! ponytail: fixed cap 64, no delete; add when eviction needed.

/// Bool-only KV; insertion-ordered pairs, capped at 64 entries.
#[derive(Debug, Clone, Default)]
pub struct CtxKv {
    pairs: Vec<(String, bool)>,
}

impl CtxKv {
    #[must_use]
    pub fn new() -> Self {
        Self { pairs: Vec::new() }
    }

    pub fn set(&mut self, key: &str, value: bool) {
        if let Some(slot) = self.pairs.iter_mut().find(|(k, _)| k == key) {
            slot.1 = value;
            return;
        }
        if self.pairs.len() >= 64 {
            return;
        }
        self.pairs.push((key.to_string(), value));
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<bool> {
        self.pairs.iter().find(|(k, _)| k == key).map(|(_, v)| *v)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut kv = CtxKv::new();
        kv.set("a", true);
        assert_eq!(kv.get("a"), Some(true));
    }

    #[test]
    fn overwrite() {
        let mut kv = CtxKv::new();
        kv.set("a", true);
        kv.set("a", false);
        assert_eq!(kv.get("a"), Some(false));
        assert_eq!(kv.len(), 1);
    }

    #[test]
    fn missing_none() {
        assert_eq!(CtxKv::new().get("nope"), None);
    }

    #[test]
    fn cap_64() {
        let mut kv = CtxKv::new();
        for i in 0..70 {
            kv.set(&format!("k{i}"), true);
        }
        assert_eq!(kv.len(), 64);
        assert_eq!(kv.get("k69"), None);
    }

    #[test]
    fn empty_len() {
        let kv = CtxKv::new();
        assert!(kv.is_empty());
        assert_eq!(kv.len(), 0);
    }
}
