#![forbid(unsafe_code)]
//! Pending/granted permission id sets backing permission prompts.

/// Bounded permission id context.
#[derive(Clone, Debug, Default)]
pub struct PermissionCtx {
    pending: Vec<String>,
    granted: Vec<String>,
}

impl PermissionCtx {
    pub const MAX_PENDING: usize = 32;
    pub const MAX_GRANTED: usize = 64;
    pub const MAX_PENDING_LEN: usize = 128;
    pub const MAX_GRANTED_LEN: usize = 64;

    pub fn new() -> Self {
        Self::default()
    }

    fn valid(id: &str, max_len: usize) -> bool {
        !id.is_empty() && id.len() <= max_len
    }

    /// Queue an id for approval. False if invalid, duplicate, or full.
    pub fn request(&mut self, id: &str) -> bool {
        if !Self::valid(id, Self::MAX_PENDING_LEN) {
            return false;
        }
        if self.pending.iter().any(|p| p == id) || self.granted.iter().any(|g| g == id) {
            return false;
        }
        if self.pending.len() >= Self::MAX_PENDING {
            return false;
        }
        self.pending.push(id.to_string());
        true
    }

    /// Approve a pending id, moving it to granted. False if missing/invalid/full.
    pub fn grant(&mut self, id: &str) -> bool {
        let pos = match self.pending.iter().position(|p| p == id) {
            Some(p) => p,
            None => return false,
        };
        if !Self::valid(id, Self::MAX_GRANTED_LEN) || self.granted.len() >= Self::MAX_GRANTED {
            return false;
        }
        self.pending.remove(pos);
        self.granted.push(id.to_string());
        true
    }

    /// Reject a pending id. False if not pending.
    pub fn deny(&mut self, id: &str) -> bool {
        match self.pending.iter().position(|p| p == id) {
            Some(p) => {
                self.pending.remove(p);
                true
            }
            None => false,
        }
    }

    /// True if the id was granted.
    pub fn is_granted(&self, id: &str) -> bool {
        self.granted.iter().any(|g| g == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_then_grant() {
        let mut ctx = PermissionCtx::new();
        assert!(ctx.request("bash"));
        assert!(ctx.grant("bash"));
        assert!(ctx.is_granted("bash"));
    }

    #[test]
    fn deny_removes_pending() {
        let mut ctx = PermissionCtx::new();
        assert!(ctx.request("edit"));
        assert!(ctx.deny("edit"));
        assert!(!ctx.deny("edit"));
        assert!(!ctx.is_granted("edit"));
    }

    #[test]
    fn missing_grant_deny_false() {
        let mut ctx = PermissionCtx::new();
        assert!(!ctx.grant("nope"));
        assert!(!ctx.deny("nope"));
        assert!(!ctx.is_granted("nope"));
    }

    #[test]
    fn duplicate_request_false() {
        let mut ctx = PermissionCtx::new();
        assert!(ctx.request("read"));
        assert!(!ctx.request("read"));
        assert!(ctx.grant("read"));
        assert!(!ctx.request("read"));
    }

    #[test]
    fn caps_enforced() {
        let mut ctx = PermissionCtx::new();
        assert!(!ctx.request(""));
        assert!(!ctx.request(&"x".repeat(129)));
        for i in 0..PermissionCtx::MAX_PENDING {
            assert!(ctx.request(&format!("p{i}")));
        }
        assert!(!ctx.request("overflow"));
        let mut g = PermissionCtx::new();
        for i in 0..PermissionCtx::MAX_GRANTED {
            assert!(g.request(&format!("g{i}")));
            assert!(g.grant(&format!("g{i}")));
        }
        assert!(g.request("extra"));
        assert!(!g.grant("extra"));
    }
}
