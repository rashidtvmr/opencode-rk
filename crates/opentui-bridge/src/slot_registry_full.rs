#![forbid(unsafe_code)]
//! Op-counting wrapper over [`SlotRegistry`].

use crate::slot_registry::{SlotKind, SlotRegistry};

/// [`SlotRegistry`] with successful-op counter.
#[derive(Debug, Default)]
pub struct SlotFlow {
    reg: SlotRegistry,
    ops: u64,
}

impl SlotFlow {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            reg: SlotRegistry::new(),
            ops: 0,
        }
    }

    pub fn mount(&mut self, kind: SlotKind, owner: &str) -> bool {
        let ok = self.reg.mount(kind, owner);
        if ok {
            self.ops += 1;
        }
        ok
    }

    pub fn unmount(&mut self, kind: SlotKind) -> bool {
        let first = self.reg.owners_of(kind).into_iter().next();
        match first {
            Some(o) => {
                let ok = self.reg.unmount(kind, &o);
                if ok {
                    self.ops += 1;
                }
                ok
            }
            None => false,
        }
    }

    #[must_use]
    pub const fn ops(&self) -> u64 {
        self.ops
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_ops_zero() {
        assert_eq!(SlotFlow::new().ops(), 0);
    }

    #[test]
    fn mount_bumps_ops() {
        let mut f = SlotFlow::new();
        assert!(f.mount(SlotKind::Status, "a"));
        assert_eq!(f.ops(), 1);
    }

    #[test]
    fn mount_fail_no_bump() {
        let mut f = SlotFlow::new();
        assert!(!f.mount(SlotKind::Status, ""));
        assert!(f.mount(SlotKind::Status, "a"));
        assert!(!f.mount(SlotKind::Status, "a"));
        assert_eq!(f.ops(), 1);
    }

    #[test]
    fn unmount_first_bumps() {
        let mut f = SlotFlow::new();
        f.mount(SlotKind::Sidebar, "a");
        f.mount(SlotKind::Sidebar, "b");
        assert!(f.unmount(SlotKind::Sidebar));
        assert_eq!(f.ops(), 3);
        assert_eq!(f.reg.owners_of(SlotKind::Sidebar), vec!["b".to_string()]);
    }

    #[test]
    fn unmount_missing_no_bump() {
        let mut f = SlotFlow::new();
        assert!(!f.unmount(SlotKind::Dialog));
        assert_eq!(f.ops(), 0);
        f.mount(SlotKind::Dialog, "d1");
        assert!(f.unmount(SlotKind::Dialog));
        assert!(!f.unmount(SlotKind::Dialog));
        assert_eq!(f.ops(), 2);
    }
}
