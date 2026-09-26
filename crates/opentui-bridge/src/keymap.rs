#![forbid(unsafe_code)]
//! Keymap bindings + mode stack.
//!
//! TS sources (local checkout /home/rashid/projects/opencode @ a0d9b6c,
//! diverges from pinned 95daf90; line numbers below are a0d9b6c):
//! - `packages/tui/src/config/keybind.ts:8-15` KeyStroke schema.
//! - `keybind.ts:28-34` BindingValueSchema: `false | "none" | item | item[]`.
//! - `keybind.ts:41` LeaderDefault `"ctrl+x"`.
//! - `keybind.ts:45-240` Definitions table (183 entries).
//! - `keybind.ts:439-458` `toBindingConfig` / `parse` (defaults + overrides).
//! - `packages/tui/src/keymap.tsx:20-21` LEADER_TOKEN / OPENCODE_BASE_MODE.
//! - `keymap.tsx:53-108` mode stack: `setData(OPENCODE_MODE_KEY, base)`,
//!   push with dispose-by-id, fallback to base when empty.
//! - `keymap.tsx:214-229` register fns: registerCommaBindings,
//!   registerKeyAliases (local), registerBaseLayoutFallback,
//!   registerTimedLeader, registerEscapeClearsPendingSequence,
//!   registerBackspacePopsPendingSequence, registerManagedTextareaLayer.
//! - `packages/tui/src/config/index.tsx:21` LeaderTimeoutDefault 2000.
//!
//! Single-stroke parsing reused from `crate::key_event` (`parse_stroke`,
//! `KeyEvent::is_leader`); comma-separated sequences split here
//! (cf. `registerCommaBindings`).

use core::fmt;

use crate::key_event::{parse_stroke, KeyEvent, KeyEventError};

/// Default leader key, mirrors `LeaderDefault` (keybind.ts:41).
pub const LEADER_DEFAULT: &str = "ctrl+x";
/// Base mode, mirrors `OPENCODE_BASE_MODE` (keymap.tsx:21).
pub const BASE_MODE: &str = "base";
/// Max modes on the stack (base + pushes). Fail-closed: push beyond errs.
pub const MAX_MODES: usize = 16;
/// Max bindings in a config. Fail-closed: insert beyond errs.
pub const MAX_BINDINGS: usize = 256;
/// Max strokes per comma-separated binding. Fail-closed: longer errs.
pub const MAX_STROKES: usize = 8;
/// Max chars for a mode name. Fail-closed: longer rejected.
pub const MAX_MODE_LEN: usize = 32;
/// Default leader timeout ms, mirrors `LeaderTimeoutDefault` (index.tsx:21).
pub const LEADER_TIMEOUT_DEFAULT_MS: u64 = 2000;

/// Fail-closed keymap errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeymapError {
    Empty,
    TooManyStrokes,
    TooManyBindings,
    TooManyModes,
    EmptyModeName,
    ModeNameTooLong,
    Stroke(KeyEventError),
}

impl fmt::Display for KeymapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty binding"),
            Self::TooManyStrokes => f.write_str("too many strokes in binding"),
            Self::TooManyBindings => f.write_str("too many bindings"),
            Self::TooManyModes => f.write_str("too many modes"),
            Self::EmptyModeName => f.write_str("empty mode name"),
            Self::ModeNameTooLong => f.write_str("mode name too long"),
            Self::Stroke(e) => write!(f, "bad stroke: {e}"),
        }
    }
}

impl std::error::Error for KeymapError {}

impl From<KeyEventError> for KeymapError {
    fn from(e: KeyEventError) -> Self {
        Self::Stroke(e)
    }
}

/// Binding value. TS: BindingValueSchema (keybind.ts:28-34):
/// `false` disables, `"none"` unbinds, else stroke(s).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingValue {
    Disabled,
    Unbound,
    Strokes(Vec<KeyEvent>),
}

impl BindingValue {
    /// Parse `"none"` -> Unbound, `"false"` -> Disabled, else comma-separated
    /// strokes (`"ctrl+c,ctrl+d,<leader>q"`). Case-insensitive specials,
    /// fail-closed on empty/over-long/bad strokes.
    pub fn parse(s: &str) -> Result<Self, KeymapError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "none" => return Ok(Self::Unbound),
            "false" => return Ok(Self::Disabled),
            _ => {}
        }
        if s.trim().is_empty() {
            return Err(KeymapError::Empty);
        }
        let mut out = Vec::new();
        for part in s.split(',') {
            if out.len() >= MAX_STROKES {
                return Err(KeymapError::TooManyStrokes);
            }
            out.push(parse_stroke(part)?);
        }
        if out.is_empty() {
            return Err(KeymapError::Empty);
        }
        Ok(Self::Strokes(out))
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::Strokes(_))
    }
}

/// Mode stack. Bottom is always [`BASE_MODE`]; pop never removes it
/// (mirrors keymap.tsx:66 fallback to base when empty).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeStack {
    stack: Vec<String>,
}

impl ModeStack {
    #[must_use]
    pub fn new() -> Self {
        Self { stack: vec![BASE_MODE.to_string()] }
    }

    pub fn push(&mut self, mode: &str) -> Result<(), KeymapError> {
        let mode = mode.trim();
        if mode.is_empty() {
            return Err(KeymapError::EmptyModeName);
        }
        if mode.len() > MAX_MODE_LEN {
            return Err(KeymapError::ModeNameTooLong);
        }
        if self.stack.len() >= MAX_MODES {
            return Err(KeymapError::TooManyModes);
        }
        self.stack.push(mode.to_string());
        Ok(())
    }

    /// Pop one mode; `None` when only base remains.
    pub fn pop(&mut self) -> Option<String> {
        if self.stack.len() > 1 {
            self.stack.pop()
        } else {
            None
        }
    }

    #[must_use]
    pub fn current(&self) -> &str {
        // ponytail: base is never popped, so non-empty invariant holds.
        self.stack.last().map(String::as_str).unwrap_or(BASE_MODE)
    }

    #[must_use]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}

impl Default for ModeStack {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolved keymap: leader timeout + named bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeymapConfig {
    pub leader_timeout_ms: u64,
    pub bindings: Vec<(String, BindingValue)>,
}

impl KeymapConfig {
    #[must_use]
    pub fn new() -> Self {
        Self { leader_timeout_ms: LEADER_TIMEOUT_DEFAULT_MS, bindings: Vec::new() }
    }

    /// Insert or replace binding `name`. Fail-closed past [`MAX_BINDINGS`].
    pub fn insert(&mut self, name: &str, value: BindingValue) -> Result<(), KeymapError> {
        if let Some(slot) = self.bindings.iter_mut().find(|(n, _)| n == name) {
            slot.1 = value;
            return Ok(());
        }
        if self.bindings.len() >= MAX_BINDINGS {
            return Err(KeymapError::TooManyBindings);
        }
        self.bindings.push((name.to_string(), value));
        Ok(())
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&BindingValue> {
        self.bindings.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

impl Default for KeymapConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Parsed [`LEADER_DEFAULT`] key (`"ctrl+x"`; infallible input).
#[must_use]
pub fn leader_key() -> KeyEvent {
    parse_stroke(LEADER_DEFAULT).expect("LEADER_DEFAULT parses")
}

/// Substitute bare leader tokens with the leader key.
/// Non-leader strokes pass through unchanged.
#[must_use]
pub fn resolve_leader(strokes: &[KeyEvent]) -> Vec<KeyEvent> {
    let leader = leader_key();
    strokes
        .iter()
        .map(|ev| if ev.is_leader() { leader.clone() } else { ev.clone() })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leader_default_parses() {
        let ev = parse_stroke(LEADER_DEFAULT).unwrap();
        assert_eq!(ev.name, "x");
        assert!(ev.ctrl);
        assert_eq!(ev, leader_key());
        let v = BindingValue::parse(LEADER_DEFAULT).unwrap();
        assert!(matches!(v, BindingValue::Strokes(ref s) if s.len() == 1));
    }

    #[test]
    fn none_is_unbound() {
        assert_eq!(BindingValue::parse("none").unwrap(), BindingValue::Unbound);
        assert_eq!(BindingValue::parse(" None ").unwrap(), BindingValue::Unbound);
    }

    #[test]
    fn false_is_disabled() {
        assert_eq!(BindingValue::parse("false").unwrap(), BindingValue::Disabled);
        assert!(!BindingValue::parse("false").unwrap().is_enabled());
        assert!(BindingValue::parse("ctrl+c").unwrap().is_enabled());
    }

    #[test]
    fn multi_stroke_comma_split() {
        let v = BindingValue::parse("ctrl+c,ctrl+d,<leader>q").unwrap();
        match v {
            BindingValue::Strokes(s) => {
                assert_eq!(s.len(), 3);
                assert_eq!(s[0].name, "c");
                assert_eq!(s[1].name, "d");
                assert_eq!(s[2].name, "q");
            }
            _ => panic!("expected strokes"),
        }
        assert_eq!(BindingValue::parse(""), Err(KeymapError::Empty));
        assert_eq!(
            BindingValue::parse("a,,b"),
            Err(KeymapError::Stroke(KeyEventError::Empty))
        );
    }

    #[test]
    fn mode_push_pop() {
        let mut m = ModeStack::new();
        assert_eq!(m.current(), BASE_MODE);
        m.push("dialog").unwrap();
        m.push("palette").unwrap();
        assert_eq!(m.current(), "palette");
        assert_eq!(m.depth(), 3);
        assert_eq!(m.pop().as_deref(), Some("palette"));
        assert_eq!(m.current(), "dialog");
        assert_eq!(m.pop().as_deref(), Some("dialog"));
        assert_eq!(m.current(), BASE_MODE);
        assert_eq!(m.pop(), None);
        assert_eq!(m.current(), BASE_MODE);
    }

    #[test]
    fn mode_overflow_and_bad_names_err() {
        let mut m = ModeStack::new();
        for i in 0..MAX_MODES - 1 {
            m.push(&format!("m{i}")).unwrap();
        }
        assert_eq!(m.push("one-more"), Err(KeymapError::TooManyModes));
        assert_eq!(ModeStack::new().push(""), Err(KeymapError::EmptyModeName));
        assert_eq!(
            ModeStack::new().push(&"x".repeat(MAX_MODE_LEN + 1)),
            Err(KeymapError::ModeNameTooLong)
        );
    }

    #[test]
    fn binding_overflow_errs() {
        let mut c = KeymapConfig::new();
        assert_eq!(c.leader_timeout_ms, LEADER_TIMEOUT_DEFAULT_MS);
        for i in 0..MAX_BINDINGS {
            c.insert(&format!("cmd{i}"), BindingValue::Unbound).unwrap();
        }
        assert_eq!(
            c.insert("extra", BindingValue::Unbound),
            Err(KeymapError::TooManyBindings)
        );
        c.insert("cmd0", BindingValue::Disabled).unwrap();
        assert_eq!(c.len(), MAX_BINDINGS);
        assert_eq!(c.get("cmd0"), Some(&BindingValue::Disabled));
        let many = (0..MAX_STROKES + 1).map(|_| "a").collect::<Vec<_>>().join(",");
        assert_eq!(BindingValue::parse(&many), Err(KeymapError::TooManyStrokes));
    }

    #[test]
    fn leader_substitution() {
        let bare = parse_stroke("<leader>").unwrap();
        assert!(bare.is_leader());
        let q = parse_stroke("q").unwrap();
        let out = resolve_leader(&[bare, q.clone()]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], leader_key());
        assert_eq!(out[1], q);
        assert!(resolve_leader(&[]).is_empty());
    }
}
