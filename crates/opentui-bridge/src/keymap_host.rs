#![forbid(unsafe_code)]
//! Keymap host: addon set + default keymap + pending sequence.
//!
//! TS sources (local checkout /home/rashid/projects/opencode @ a0d9b6c,
//! diverges from pinned 95daf90; line numbers below are a0d9b6c):
//! - `packages/tui/src/keymap.tsx:3-9` addon imports (`@opentui/keymap/addons/opentui`):
//!   registerBackspacePopsPendingSequence, registerBaseLayoutFallback,
//!   registerCommaBindings, registerEscapeClearsPendingSequence,
//!   registerManagedTextareaLayer, registerTimedLeader.
//! - `keymap.tsx:214-229` `registerOpencodeKeymap` wires all six
//!   (comma, alias-expander local, base-layout, timed-leader, escape, backspace,
//!   managed-textarea layer).
//! - `keymap.tsx:221` timed leader takes `leader_timeout` from config.
//! - `packages/tui/src/config/index.tsx:21` `LeaderTimeoutDefault = 2000`.
//! - `packages/tui/src/app.tsx:3,215` `createDefaultOpenTuiKeymap(renderer)`
//!   from `@opentui/keymap/opentui`.
//! - `keymap.tsx:10,183` `stringifyKeyStroke` (single-stroke display).
//! - `keymap.tsx:11-14,206-211` extras re-export: `formatCommandBindings`,
//!   `formatKeySequence` from `@opentui/keymap/extras`.
//! - `packages/tui/src/config/index.tsx:3,110` `createBindingLookup` from
//!   `@opentui/keymap/extras` (lookup/format live TS-side; not ported here).
//!
//! Reuses `crate::keymap` (`BindingValue`, `ModeStack`, bounds) and
//! `crate::key_event` (`parse_stroke`, `KeyEvent`); does not redefine them.
//! (ponytail: binding lookup + display format stay TS-side in extras;
//! port when Rust-side palette display needs them.)

use crate::key_event::{parse_stroke, KeyEvent, KeyEventError};
use crate::keymap::{KeymapError, LEADER_TIMEOUT_DEFAULT_MS, MAX_STROKES};

/// Pending-sequence cap, mirrors comma-sequence bound [`MAX_STROKES`].
pub const MAX_PENDING: usize = MAX_STROKES;

/// Host-side addon, one per `register*` import (keymap.tsx:3-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Addon {
    BackspacePopsPendingSequence,
    BaseLayoutFallback,
    CommaBindings,
    EscapeClearsPendingSequence,
    ManagedTextareaLayer,
    TimedLeader,
}

impl Addon {
    /// All six evidenced addons, in keymap.tsx:3-9 import order.
    #[must_use]
    pub fn all() -> [Self; 6] {
        [
            Self::BackspacePopsPendingSequence,
            Self::BaseLayoutFallback,
            Self::CommaBindings,
            Self::EscapeClearsPendingSequence,
            Self::ManagedTextareaLayer,
            Self::TimedLeader,
        ]
    }

    /// TS register fn name (keymap.tsx:214-229 call sites).
    #[must_use]
    pub fn as_names(&self) -> &'static str {
        match self {
            Self::BackspacePopsPendingSequence => "registerBackspacePopsPendingSequence",
            Self::BaseLayoutFallback => "registerBaseLayoutFallback",
            Self::CommaBindings => "registerCommaBindings",
            Self::EscapeClearsPendingSequence => "registerEscapeClearsPendingSequence",
            Self::ManagedTextareaLayer => "registerManagedTextareaLayer",
            Self::TimedLeader => "registerTimedLeader",
        }
    }
}

/// Default host keymap: full addon set + leader timeout
/// (default 2000ms per index.tsx:21).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultKeymap {
    pub addons: Vec<Addon>,
    pub leader_timeout_ms: u64,
}

impl DefaultKeymap {
    #[must_use]
    pub fn new() -> Self {
        Self { addons: Addon::all().to_vec(), leader_timeout_ms: LEADER_TIMEOUT_DEFAULT_MS }
    }

    #[must_use]
    pub fn with_timeout(mut self, ms: u64) -> Self {
        self.leader_timeout_ms = ms;
        self
    }

    #[must_use]
    pub fn has(&self, addon: Addon) -> bool {
        self.addons.contains(&addon)
    }

    #[must_use]
    pub fn names(&self) -> Vec<&'static str> {
        self.addons.iter().map(Addon::as_names).collect()
    }
}

impl Default for DefaultKeymap {
    fn default() -> Self {
        Self::new()
    }
}

/// Pending key sequence: backspace pops, escape clears, leader arming +
/// timeout expiry clears (registerTimedLeader / registerEscapeClearsPendingSequence
/// / registerBackspacePopsPendingSequence, keymap.tsx:221-228).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PendingSeq {
    pub keys: Vec<KeyEvent>,
    pub leader_armed: bool,
}

impl PendingSeq {
    #[must_use]
    pub fn new() -> Self {
        Self { keys: Vec::new(), leader_armed: false }
    }

    /// Push stroke; fail-closed past [`MAX_PENDING`].
    pub fn push(&mut self, ev: KeyEvent) -> Result<(), KeymapError> {
        if self.keys.len() >= MAX_PENDING {
            return Err(KeymapError::TooManyStrokes);
        }
        self.keys.push(ev);
        Ok(())
    }

    /// Backspace behavior: pop newest stroke.
    pub fn pop(&mut self) -> Option<KeyEvent> {
        self.keys.pop()
    }

    /// Escape behavior: drop whole sequence + disarm leader.
    pub fn clear(&mut self) {
        self.keys.clear();
        self.leader_armed = false;
    }

    /// Timed-leader expiry: clear when `elapsed_ms >= timeout_ms`.
    /// Returns true when cleared.
    pub fn timeout_pop(&mut self, elapsed_ms: u64, timeout_ms: u64) -> bool {
        if elapsed_ms >= timeout_ms {
            self.clear();
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

/// Split comma-separated sequence (`"ctrl+x,q"` -> `["ctrl+x", "q"]`),
/// mirrors `registerCommaBindings`. Trims; keeps empties so
/// [`parse_stroke`] fail-closes on `"a,,b"`.
#[must_use]
pub fn split_sequence(raw: &str) -> Vec<&str> {
    raw.split(',').map(str::trim).collect()
}

/// Parse comma-separated sequence into strokes.
pub fn parse_sequence(raw: &str) -> Result<Vec<KeyEvent>, KeyEventError> {
    if raw.trim().is_empty() {
        return Err(KeyEventError::Empty);
    }
    raw.split(',').map(parse_stroke).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key_event::stringify_key_event;

    #[test]
    fn addon_all_six_names() {
        let all = Addon::all();
        assert_eq!(all.len(), 6);
        let names: Vec<_> = all.iter().map(Addon::as_names).collect();
        assert_eq!(
            names,
            [
                "registerBackspacePopsPendingSequence",
                "registerBaseLayoutFallback",
                "registerCommaBindings",
                "registerEscapeClearsPendingSequence",
                "registerManagedTextareaLayer",
                "registerTimedLeader",
            ]
        );
    }

    #[test]
    fn default_has_all_addons_and_2000ms() {
        let d = DefaultKeymap::new();
        assert_eq!(d.leader_timeout_ms, 2000);
        assert_eq!(d.leader_timeout_ms, LEADER_TIMEOUT_DEFAULT_MS);
        for a in Addon::all() {
            assert!(d.has(a));
        }
        assert_eq!(d.names().len(), 6);
        assert_eq!(DefaultKeymap::default(), d);
        assert_eq!(d.clone().with_timeout(500).leader_timeout_ms, 500);
    }

    #[test]
    fn pending_push_bounded_8() {
        let mut p = PendingSeq::new();
        for _ in 0..MAX_PENDING {
            p.push(parse_stroke("a").unwrap()).unwrap();
        }
        assert_eq!(p.len(), MAX_PENDING);
        assert_eq!(p.push(parse_stroke("b").unwrap()), Err(KeymapError::TooManyStrokes));
    }

    #[test]
    fn backspace_pops_escape_clears() {
        let mut p = PendingSeq::new();
        p.leader_armed = true;
        p.push(parse_stroke("ctrl+x").unwrap()).unwrap();
        p.push(parse_stroke("q").unwrap()).unwrap();
        assert_eq!(p.pop().unwrap(), parse_stroke("q").unwrap());
        assert_eq!(p.len(), 1);
        p.clear();
        assert!(p.is_empty());
        assert!(!p.leader_armed);
        assert_eq!(p.pop(), None);
    }

    #[test]
    fn timeout_pop_clears_when_expired() {
        let mut p = PendingSeq::new();
        p.leader_armed = true;
        p.push(parse_stroke("ctrl+x").unwrap()).unwrap();
        assert!(!p.timeout_pop(1999, 2000));
        assert_eq!(p.len(), 1);
        assert!(p.timeout_pop(2000, 2000));
        assert!(p.is_empty());
        assert!(!p.leader_armed);
    }

    #[test]
    fn split_sequence_comma() {
        assert_eq!(split_sequence("ctrl+x,q"), ["ctrl+x", "q"]);
        assert_eq!(split_sequence(" a , b ").as_slice(), ["a", "b"]);
        assert_eq!(parse_sequence("ctrl+c,ctrl+d").unwrap().len(), 2);
        assert!(parse_sequence("").is_err());
        assert!(parse_sequence("a,,b").is_err());
    }

    #[test]
    fn stringify_roundtrip_via_pending() {
        let ev = parse_stroke("ctrl+x").unwrap();
        assert_eq!(parse_stroke(&stringify_key_event(&ev)).unwrap(), ev);
    }
}
