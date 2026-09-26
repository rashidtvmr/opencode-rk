#![forbid(unsafe_code)]
//! Session destination route; TS truth `packages/tui/src/routes/home/session-destination.tsx:13-29`.
/// Max destination chars.
pub const MAX_DEST_LEN: usize = 512;
/// Session destination route state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessDestRoute {
    pub dest: String,
}
impl SessDestRoute {
    /// Replace destination, clipped to 512 chars.
    pub fn set(&mut self, dest: &str) {
        self.dest = dest.chars().take(MAX_DEST_LEN).collect();
    }
    /// Current destination.
    #[must_use]
    pub fn dest_of(&self) -> &str {
        &self.dest
    }
    /// True when a destination is set.
    #[must_use]
    pub fn is_set(&self) -> bool {
        !self.dest.is_empty()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_stores_dest() {
        let mut r = SessDestRoute::default();
        r.set("/a");
        assert_eq!(r.dest_of(), "/a");
        assert!(r.is_set());
    }
    #[test]
    fn unset_is_empty() {
        assert!(!SessDestRoute::default().is_set());
    }
    #[test]
    fn set_clips_to_512() {
        let mut r = SessDestRoute::default();
        r.set(&"a".repeat(600));
        assert_eq!(r.dest_of().chars().count(), MAX_DEST_LEN);
    }
}
