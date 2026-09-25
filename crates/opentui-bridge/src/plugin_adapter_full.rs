#![forbid(unsafe_code)]
//! Full plugin mount adapter (mirrors `packages/tui/src/plugin/adapters.tsx`
//! `Slot` mount/unmount over `crate::slot_registry::SlotRegistry`).

use crate::slot_registry::{SlotKind, SlotRegistry};

/// Max tracked mounts (fail-closed; TS side unbounded).
pub const MAX_MOUNTED: usize = 16;

/// Adapter tracking mounted plugin ids on the default `Status` slot.
#[derive(Debug, Default, Clone)]
pub struct PluginAdapter {
    pub reg: SlotRegistry,
    pub mounted: Vec<String>,
}

impl PluginAdapter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            reg: SlotRegistry::new(),
            mounted: Vec::with_capacity(MAX_MOUNTED),
        }
    }

    /// Mount `id`. False when empty, duplicate, full, or registry rejects.
    pub fn mount(&mut self, id: &str) -> bool {
        if id.is_empty() || self.mounted.len() >= MAX_MOUNTED {
            return false;
        }
        if self.mounted.iter().any(|m| m == id) {
            return false;
        }
        if !self.reg.mount(SlotKind::Status, id) {
            return false;
        }
        self.mounted.push(id.to_string());
        true
    }

    /// Unmount `id`. False when absent.
    pub fn unmount(&mut self, id: &str) -> bool {
        match self.mounted.iter().position(|m| m == id) {
            Some(i) => {
                self.mounted.remove(i);
                self.reg.unmount(SlotKind::Status, id)
            }
            None => false,
        }
    }

    /// Currently mounted ids in mount order.
    #[must_use]
    pub fn mounted_list(&self) -> Vec<String> {
        self.mounted.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_ok_lists() {
        let mut a = PluginAdapter::new();
        assert!(a.mount("p-a"));
        assert_eq!(a.mounted_list(), vec!["p-a".to_string()]);
    }

    #[test]
    fn dup_mount_false() {
        let mut a = PluginAdapter::new();
        assert!(a.mount("p"));
        assert!(!a.mount("p"));
        assert_eq!(a.mounted_list().len(), 1);
    }

    #[test]
    fn empty_mount_false() {
        let mut a = PluginAdapter::new();
        assert!(!a.mount(""));
        assert!(a.mounted_list().is_empty());
    }

    #[test]
    fn cap_16_false() {
        let mut a = PluginAdapter::new();
        for i in 0..MAX_MOUNTED {
            assert!(a.mount(&format!("p{i}")));
        }
        assert!(!a.mount("one-more"));
        assert_eq!(a.mounted_list().len(), MAX_MOUNTED);
    }

    #[test]
    fn unmount_ok_missing_false() {
        let mut a = PluginAdapter::new();
        assert!(!a.unmount("ghost"));
        assert!(a.mount("d1"));
        assert!(a.unmount("d1"));
        assert!(!a.unmount("d1"));
        assert!(a.mounted_list().is_empty());
    }

    #[test]
    fn remount_after_unmount() {
        let mut a = PluginAdapter::new();
        assert!(a.mount("x"));
        assert!(a.unmount("x"));
        assert!(a.mount("x"));
        assert_eq!(a.mounted_list(), vec!["x".to_string()]);
    }
}
