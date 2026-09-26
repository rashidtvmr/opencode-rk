#![forbid(unsafe_code)]
//! Bounded record-key set; port of `isRecord` guard + key tracking.
//! TS truth (`packages/tui/src/util/record.ts:1`): plain object, non-array.
//! `ponytail:` no value storage; add when caller needs values.
#[derive(Debug, Default, Clone)]
pub struct RecordMem {
    keys: Vec<String>,
}
impl RecordMem {
    pub const CAP: usize = 64;
    pub const KEY_CAP: usize = 128;
    #[must_use]
    pub fn new() -> Self {
        Self { keys: Vec::new() }
    }
    fn norm(key: &str) -> String {
        key.chars().take(Self::KEY_CAP).collect()
    }
    pub fn insert(&mut self, key: &str) -> bool {
        let k = Self::norm(key);
        if k.is_empty() || self.keys.contains(&k) || self.keys.len() >= Self::CAP {
            return false;
        }
        self.keys.push(k);
        true
    }
    #[must_use]
    pub fn has(&self, key: &str) -> bool {
        let k = Self::norm(key);
        !k.is_empty() && self.keys.contains(&k)
    }
    pub fn remove(&mut self, key: &str) -> bool {
        let k = Self::norm(key);
        match self.keys.iter().position(|x| *x == k) {
            Some(i) => {
                self.keys.remove(i);
                true
            }
            None => false,
        }
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn insert_and_has() {
        let mut m = RecordMem::new();
        assert!(m.insert("a") && m.has("a") && m.len() == 1);
    }
    #[test]
    fn duplicate_rejected() {
        let mut m = RecordMem::new();
        assert!(m.insert("a") && !m.insert("a") && m.len() == 1);
    }
    #[test]
    fn cap_at_64() {
        let mut m = RecordMem::new();
        for i in 0..RecordMem::CAP {
            assert!(m.insert(&format!("k{i}")));
        }
        assert!(!m.insert("overflow") && m.len() == 64);
    }
    #[test]
    fn remove_roundtrip() {
        let mut m = RecordMem::new();
        assert!(m.insert("a") && m.remove("a") && !m.has("a") && !m.remove("a"));
    }
    #[test]
    fn key_capped_at_128() {
        let mut m = RecordMem::new();
        let long = "x".repeat(200);
        assert!(m.insert(&long) && m.has(&long) && m.len() == 1);
    }
    #[test]
    fn empty_rejected() {
        let mut m = RecordMem::new();
        assert!(!m.insert("") && !m.has("") && !m.remove(""));
    }
}
