#![forbid(unsafe_code)]
//! Compact plugin slot mount registry.
//!
//! Sibling `plugin_slots.rs` covers the 12 host `SlotName`s with a
//! `Result`-based register API. This module is the minimal 4-kind
//! mount/unmount companion used by the TUI plugin surface
//! (`packages/tui/src/plugin/api.ts` register/dispose closure pattern).

/// Max mounted slots (fail-closed; TS side unbounded).
pub const MAX_SLOTS: usize = 32;
/// Max owner id bytes (fail-closed).
pub const MAX_OWNER: usize = 64;

/// Mountable slot kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotKind {
    Status,
    Sidebar,
    Footer,
    Dialog,
}

/// One mounted slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotEntry {
    pub kind: SlotKind,
    pub owner: String,
}

/// Bounded mount registry.
#[derive(Debug, Default, Clone)]
pub struct SlotRegistry {
    slots: Vec<SlotEntry>,
}

impl SlotRegistry {
    #[must_use]
    pub const fn new() -> Self {
        Self { slots: Vec::new() }
    }

    /// Mount `owner` on `kind`. False when duplicate, owner empty/too
    /// long, or registry full.
    pub fn mount(&mut self, kind: SlotKind, owner: &str) -> bool {
        if owner.is_empty() || owner.len() > MAX_OWNER {
            return false;
        }
        if self.slots.len() >= MAX_SLOTS {
            return false;
        }
        if self
            .slots
            .iter()
            .any(|e| e.kind == kind && e.owner == owner)
        {
            return false;
        }
        self.slots.push(SlotEntry {
            kind,
            owner: owner.to_string(),
        });
        true
    }

    /// Unmount `owner` from `kind`. False when absent.
    pub fn unmount(&mut self, kind: SlotKind, owner: &str) -> bool {
        match self
            .slots
            .iter()
            .position(|e| e.kind == kind && e.owner == owner)
        {
            Some(i) => {
                self.slots.remove(i);
                true
            }
            None => false,
        }
    }

    /// Owners mounted on `kind`.
    #[must_use]
    pub fn owners_of(&self, kind: SlotKind) -> Vec<String> {
        self.slots
            .iter()
            .filter(|e| e.kind == kind)
            .map(|e| e.owner.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_ok() {
        let mut r = SlotRegistry::new();
        assert!(r.mount(SlotKind::Status, "plugin-a"));
        assert_eq!(r.owners_of(SlotKind::Status), vec!["plugin-a".to_string()]);
    }

    #[test]
    fn dup_mount_false() {
        let mut r = SlotRegistry::new();
        assert!(r.mount(SlotKind::Footer, "p"));
        assert!(!r.mount(SlotKind::Footer, "p"));
        assert!(r.mount(SlotKind::Footer, "q"));
        assert!(!r.mount(SlotKind::Status, ""));
        assert!(!r.mount(SlotKind::Status, &"x".repeat(MAX_OWNER + 1)));
    }

    #[test]
    fn cap_false() {
        let mut r = SlotRegistry::new();
        let kinds = [
            SlotKind::Status,
            SlotKind::Sidebar,
            SlotKind::Footer,
            SlotKind::Dialog,
        ];
        for i in 0..MAX_SLOTS {
            assert!(r.mount(kinds[i % 4], &format!("owner-{i}")));
        }
        assert!(!r.mount(SlotKind::Status, "one-more"));
    }

    #[test]
    fn unmount_missing_false() {
        let mut r = SlotRegistry::new();
        assert!(!r.unmount(SlotKind::Dialog, "ghost"));
        assert!(r.mount(SlotKind::Dialog, "d1"));
        assert!(r.unmount(SlotKind::Dialog, "d1"));
        assert!(!r.unmount(SlotKind::Dialog, "d1"));
    }

    #[test]
    fn owners_filter() {
        let mut r = SlotRegistry::new();
        r.mount(SlotKind::Sidebar, "a");
        r.mount(SlotKind::Status, "b");
        r.mount(SlotKind::Sidebar, "c");
        assert_eq!(
            r.owners_of(SlotKind::Sidebar),
            vec!["a".to_string(), "c".to_string()]
        );
        assert_eq!(r.owners_of(SlotKind::Dialog), Vec::<String>::new());
    }
}
