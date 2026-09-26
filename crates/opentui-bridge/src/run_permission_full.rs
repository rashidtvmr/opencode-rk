#![forbid(unsafe_code)]
//! Queued allow/reject flow over [`PermGate`] (mirrors footer.permission.tsx).
use crate::perm_gate::PermGate;
/// Max queued ids.
pub const PENDING_CAP: usize = 8;
/// Max id bytes (matches `PermissionCtx::MAX_PENDING_LEN`).
pub const ID_CAP: usize = 128;
/// Allow-once/reject queue wrapping [`PermGate`].
#[derive(Debug, Default)]
pub struct PermissionFlow {
    pub gate: PermGate,
    pending: Vec<String>,
}
impl PermissionFlow {
    pub fn new() -> Self {
        Self::default()
    }
    fn valid(id: &str) -> bool {
        !id.is_empty() && id.len() <= ID_CAP
    }
    /// Queue `id`. False if invalid, granted, duplicate, or full.
    pub fn request(&mut self, id: &str) -> bool {
        if !Self::valid(id) || self.gate.ctx.is_granted(id) {
            return false;
        }
        if self.pending.iter().any(|p| p == id) || self.pending.len() >= PENDING_CAP {
            return false;
        }
        self.gate.ask(id);
        self.pending.push(id.to_string());
        true
    }
    /// Allow queued `id`. False if not queued.
    pub fn allow(&mut self, id: &str) -> bool {
        if !self.pending.iter().any(|p| p == id) || !self.gate.resolve(id, true) {
            return false;
        }
        self.pending.retain(|p| p != id);
        true
    }
    /// Reject queued `id`. False if not queued.
    pub fn deny(&mut self, id: &str) -> bool {
        if !self.pending.iter().any(|p| p == id) || !self.gate.resolve(id, false) {
            return false;
        }
        self.pending.retain(|p| p != id);
        true
    }
    /// Queued ids in request order.
    pub fn pending_list(&self) -> Vec<String> {
        self.pending.clone()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn request_queues_pending() {
        let mut f = PermissionFlow::new();
        assert!(f.request("bash"));
        assert_eq!(f.pending_list(), vec!["bash".to_string()]);
    }
    #[test]
    fn request_rejects_bad() {
        let mut f = PermissionFlow::new();
        assert!(!f.request(""));
        assert!(!f.request(&"x".repeat(129)));
        assert!(f.pending_list().is_empty());
    }
    #[test]
    fn request_dup_or_granted_false() {
        let mut f = PermissionFlow::new();
        assert!(f.request("edit"));
        assert!(!f.request("edit"));
        assert!(f.allow("edit"));
        assert!(!f.request("edit"));
    }
    #[test]
    fn request_caps_eight() {
        let mut f = PermissionFlow::new();
        for i in 0..PENDING_CAP {
            assert!(f.request(&format!("p{i}")));
        }
        assert!(!f.request("overflow"));
        assert_eq!(f.pending_list().len(), 8);
    }
    #[test]
    fn allow_grants_clears() {
        let mut f = PermissionFlow::new();
        assert!(f.request("bash"));
        assert!(f.allow("bash"));
        assert!(f.pending_list().is_empty());
        assert!(f.gate.ctx.is_granted("bash"));
        assert!(!f.allow("bash"));
    }
    #[test]
    fn deny_removes_not_granted() {
        let mut f = PermissionFlow::new();
        assert!(f.request("rm"));
        assert!(f.deny("rm"));
        assert!(f.pending_list().is_empty());
        assert!(!f.gate.ctx.is_granted("rm"));
        assert!(!f.deny("rm"));
        assert!(!f.allow("nope") && !f.deny("nope"));
    }
}
