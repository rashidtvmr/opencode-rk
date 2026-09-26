#![forbid(unsafe_code)]
//! Session permission state (mirrors `permission.tsx` ask/resolve).
/// Max chars for [`SessPerm::id`].
pub const ID_CAP: usize = 128;
fn trunc(s: &str) -> String {
    s.chars().take(ID_CAP).collect()
}
/// One pending session permission, tri-state decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessPerm {
    id: String,
    decision: Option<bool>,
}
impl SessPerm {
    /// Empty id, no decision.
    pub fn new() -> Self {
        Self {
            id: String::new(),
            decision: None,
        }
    }
    /// Ask new id (cap 128), reset to pending.
    pub fn ask(&mut self, id: &str) {
        self.id = trunc(id);
        self.decision = None;
    }
    /// Resolve pending; last call wins.
    pub fn resolve(&mut self, allow: bool) {
        self.decision = Some(allow);
    }
    /// True while undecided.
    pub fn pending(&self) -> bool {
        self.decision.is_none()
    }
    /// Current id.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Current decision.
    pub fn decision(&self) -> Option<bool> {
        self.decision
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_pending_capped() {
        let mut s = SessPerm::new();
        assert!(s.pending());
        s.ask(&"x".repeat(200));
        assert_eq!(s.id().chars().count(), ID_CAP);
        assert!(s.pending());
    }
    #[test]
    fn resolve_sets_decision() {
        let mut s = SessPerm::new();
        s.ask("p1");
        s.resolve(true);
        assert!(!s.pending());
        assert_eq!(s.decision(), Some(true));
        s.resolve(false);
        assert_eq!(s.decision(), Some(false));
    }
    #[test]
    fn ask_resets_pending() {
        let mut s = SessPerm::new();
        s.ask("a");
        s.resolve(true);
        s.ask("b");
        assert_eq!(s.id(), "b");
        assert!(s.pending());
    }
}
