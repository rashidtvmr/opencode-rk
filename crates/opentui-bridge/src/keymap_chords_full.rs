#![forbid(unsafe_code)]
//! Chord buf FIX-16 LIFO-only, one pending; upgrade via pending_count (cf KeymapTsx).

const MAX_CHORD_KEYS: usize = 8;

#[derive(Debug, Clone, Default)]
pub struct ChordBuf {
    keys: Vec<String>,
}

impl ChordBuf {
    pub fn new() -> Self {
        Self {
            keys: Vec::with_capacity(MAX_CHORD_KEYS),
        }
    }
    /// Push lowercased key; false on overflow (unchanged).
    pub fn push(&mut self, key: &str) -> bool {
        if self.keys.len() >= MAX_CHORD_KEYS {
            return false;
        }
        self.keys.push(key.to_ascii_lowercase());
        true
    }
    pub fn pending_of(&self) -> &[String] {
        &self.keys
    }
    pub fn take(&mut self) -> Vec<String> {
        std::mem::take(&mut self.keys)
    }
    /// Full chord=>action, esc=>cancel, empty=>none, else pending.
    pub fn resolve(&self) -> &'static str {
        match self.keys.as_slice() {
            [a, b] if a == "ctrl" && b == "t" => "palette",
            [a, b] if a == "ctrl" && b == "g" => "chat",
            [a, b] if a == "ctrl" && b == "x" => "context",
            [a] if a == "escape" || a == "esc" => "cancel",
            [] => "none",
            _ => "pending",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn buf(keys: &[&str]) -> ChordBuf {
        let mut b = ChordBuf::new();
        for k in keys {
            assert!(b.push(k));
        }
        b
    }
    #[test]
    fn empty_resolves_none() {
        let b = ChordBuf::new();
        assert_eq!(b.resolve(), "none");
        assert!(b.pending_of().is_empty());
    }
    #[test]
    fn full_chords_resolve() {
        assert_eq!(buf(&["ctrl", "t"]).resolve(), "palette");
        assert_eq!(buf(&["ctrl", "g"]).resolve(), "chat");
        assert_eq!(buf(&["ctrl", "x"]).resolve(), "context");
    }
    #[test]
    fn esc_cancels_and_drains() {
        assert_eq!(buf(&["escape"]).resolve(), "cancel");
        assert_eq!(buf(&["ESC"]).resolve(), "cancel");
        let mut b = buf(&["escape"]);
        assert_eq!(b.take(), ["escape"]);
        assert_eq!(b.resolve(), "none");
    }
    #[test]
    fn partial_is_pending() {
        let mut b = ChordBuf::new();
        assert!(b.push("ctrl"));
        assert_eq!(b.resolve(), "pending");
        assert_eq!(b.pending_of(), ["ctrl"]);
    }
    #[test]
    fn overflow_and_drain() {
        let mut b = ChordBuf::new();
        for _ in 0..MAX_CHORD_KEYS {
            assert!(b.push("k"));
        }
        assert!(!b.push("overflow"));
        assert_eq!(b.take().len(), MAX_CHORD_KEYS);
    }
}
