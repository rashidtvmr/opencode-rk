#![forbid(unsafe_code)]
//! Focus router over [`FocusRing`] (BRIDGE-GAP-35).
//!
//! Maps raw ring ids to [`FocusTarget`] regions. Style resolution stays in
//! [`crate::render_focus`]; this module only routes which region holds focus.
//! Empty ring is fail-closed: `focus_next`/`focus_prev`/`current_target`
//! return `None`.

use crate::input::{FocusError, FocusRing, MAX_FOCUS};

/// Focusable UI region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Composer,
    Sidebar,
    Palette,
    Dialog,
}

/// Router: ring of ids plus id-to-target map.
#[derive(Debug, Default)]
pub struct FocusWire {
    pub ring: FocusRing,
    pub map: Vec<(u32, FocusTarget)>,
}

impl FocusWire {
    #[must_use]
    pub fn new() -> Self {
        Self {
            ring: FocusRing::new(),
            map: Vec::new(),
        }
    }

    /// Register `id` for `target`. Errs on 0, duplicate, or full map.
    pub fn register(&mut self, id: u32, target: FocusTarget) -> Result<(), FocusError> {
        if id == 0 || self.map.iter().any(|(i, _)| *i == id) {
            return Err(FocusError::Invalid);
        }
        if self.map.len() >= MAX_FOCUS {
            return Err(FocusError::Full);
        }
        self.ring.push(id)?;
        self.map.push((id, target));
        Ok(())
    }

    fn lookup(&self, id: u32) -> Option<FocusTarget> {
        self.map.iter().find(|(i, _)| *i == id).map(|(_, t)| *t)
    }

    /// Currently focused target; `None` when empty or id unmapped.
    #[must_use]
    pub fn current_target(&self) -> Option<FocusTarget> {
        self.lookup(self.ring.current()?)
    }

    /// Advance with wrap; `None` when empty or newly focused id unmapped.
    pub fn focus_next(&mut self) -> Option<FocusTarget> {
        let id = self.ring.next().ok()?;
        self.lookup(id)
    }

    /// Retreat with wrap; `None` when empty or newly focused id unmapped.
    pub fn focus_prev(&mut self) -> Option<FocusTarget> {
        let id = self.ring.prev().ok()?;
        self.lookup(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wired() -> FocusWire {
        let mut w = FocusWire::new();
        w.register(1, FocusTarget::Composer).unwrap();
        w.register(2, FocusTarget::Sidebar).unwrap();
        w.register(3, FocusTarget::Palette).unwrap();
        w
    }

    #[test]
    fn register_and_cycle() {
        let mut w = wired();
        assert_eq!(w.current_target(), Some(FocusTarget::Composer));
        assert_eq!(w.focus_next(), Some(FocusTarget::Sidebar));
        assert_eq!(w.focus_next(), Some(FocusTarget::Palette));
        assert_eq!(w.focus_prev(), Some(FocusTarget::Sidebar));
    }

    #[test]
    fn empty_is_none_fail_closed() {
        let mut w = FocusWire::new();
        assert_eq!(w.current_target(), None);
        assert_eq!(w.focus_next(), None);
        assert_eq!(w.focus_prev(), None);
    }

    #[test]
    fn wrap_around_both_directions() {
        let mut w = wired();
        assert_eq!(w.focus_next(), Some(FocusTarget::Sidebar));
        assert_eq!(w.focus_next(), Some(FocusTarget::Palette));
        assert_eq!(w.focus_next(), Some(FocusTarget::Composer));
        assert_eq!(w.focus_prev(), Some(FocusTarget::Palette));
    }

    #[test]
    fn unknown_id_kept_as_none() {
        let mut w = wired();
        w.ring.push(99).unwrap();
        assert_eq!(w.focus_next(), Some(FocusTarget::Sidebar));
        assert_eq!(w.focus_next(), Some(FocusTarget::Palette));
        assert_eq!(w.focus_next(), None);
        assert_eq!(w.ring.current(), Some(99));
        assert_eq!(w.current_target(), None);
    }

    #[test]
    fn duplicate_register_errs() {
        let mut w = wired();
        assert_eq!(w.register(1, FocusTarget::Dialog), Err(FocusError::Invalid));
        assert_eq!(w.register(0, FocusTarget::Dialog), Err(FocusError::Invalid));
        assert_eq!(w.current_target(), Some(FocusTarget::Composer));
    }

    #[test]
    fn dialog_target_reachable() {
        let mut w = wired();
        w.register(4, FocusTarget::Dialog).unwrap();
        w.focus_next();
        w.focus_next();
        assert_eq!(w.focus_next(), Some(FocusTarget::Dialog));
    }
}
