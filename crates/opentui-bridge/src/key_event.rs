#![forbid(unsafe_code)]
//! Single-stroke key events: parse/stringify/match.
//!
//! TS sources (local checkout /home/rashid/projects/opencode @ a0d9b6c,
//! diverges from pinned 95daf90; line numbers below are a0d9b6c):
//! - `packages/tui/src/config/keybind.ts:8-15` KeyStroke schema:
//!   name + ctrl/shift/meta/super/hyper (all optional bools).
//! - `keybind.ts:17-25` BindingObject: key (string | KeyStroke),
//!   event `press | release` (optional, default press), preventDefault, fallthrough.
//! - `keybind.ts:41` LeaderDefault `"ctrl+x"`; leader sequences like
//!   `"<leader>q"` (`keybind.ts:48,77+`).
//! - `packages/tui/src/keymap.tsx:20` `LEADER_TOKEN = "leader"`.
//! - `keymap.tsx:180-188` leaderDisplay: leader key shown via
//!   `stringifyKeyStroke` when non-string.
//! - `keymap.tsx:190-204` formatOptions: keyNameAliases
//!   (pageup->pgup, pagedown->pgdn, delete->del), modifierAliases (meta->alt).
//! - `keymap.tsx:112-117` KEY_ALIASES: enter->return, esc->escape,
//!   pgdown->pagedown, pgup->pageup.
//! - `KeyEvent`/`Renderable` types themselves come from `@opentui/core`
//!   (not vendored locally); here `KeyEvent` is the Rust-side stroke struct.
//!
//! Reuses `crate::input::Key` via `From` (meta maps to `alt`;
//! super/hyper dropped); does not duplicate it.

use core::fmt;
use crate::input::Key;

/// Max chars for `KeyEvent.name`. Fail-closed: longer names rejected.
pub const MAX_NAME: usize = 32;
/// Max chars for a whole stroke string accepted by [`parse_stroke`].
pub const MAX_STROKE: usize = 64;
/// Leader prefix/token, mirrors `LEADER_TOKEN` (keymap.tsx:20).
pub const LEADER_TOKEN: &str = "leader";

/// Press vs release. TS: BindingObject `event` (keybind.ts:20), default press.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyPress {
    #[default]
    Press,
    Release,
}

/// One parsed key stroke. TS: KeyStroke schema (keybind.ts:8-15) + event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub name: String,
    pub code: u32,
    pub ctrl: bool,
    pub shift: bool,
    pub meta: bool,
    pub super_: bool,
    pub hyper: bool,
    pub event: KeyPress,
}

impl KeyEvent {
    #[must_use]
    pub fn is_leader(&self) -> bool {
        self.name == LEADER_TOKEN
            && !self.ctrl
            && !self.shift
            && !self.meta
            && !self.super_
            && !self.hyper
    }
}

/// Fail-closed parse/stringify errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventError {
    Empty,
    TooLong,
    EmptyName,
    UnknownModifier,
    UnknownEvent,
}

impl fmt::Display for KeyEventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Empty => "empty key stroke",
            Self::TooLong => "key stroke too long",
            Self::EmptyName => "empty key name",
            Self::UnknownModifier => "unknown modifier",
            Self::UnknownEvent => "unknown event suffix",
        };
        f.write_str(s)
    }
}

impl std::error::Error for KeyEventError {}

/// True when the stroke carries a `<leader>` prefix or is the bare token.
#[must_use]
pub fn is_leader_stroke(stroke: &str) -> bool {
    let s = stroke.trim();
    s == "<leader>" || s.starts_with("<leader>") || s == LEADER_TOKEN
}

/// Strip one leading `<leader>` / `<leader>+` prefix, returning
/// `(was_leader, rest)`. Multi-stroke sequences are the caller's job
/// (ponytail: single-stroke only here; sequences split on `,` upstream).
#[must_use]
pub fn strip_leader_prefix(stroke: &str) -> (bool, &str) {
    let s = stroke.trim();
    if let Some(rest) = s.strip_prefix("<leader>") {
        let rest = rest.strip_prefix('+').unwrap_or(rest);
        return (true, rest);
    }
    (false, s)
}

fn apply_alias(name: &str) -> &str {
    // Mirrors KEY_ALIASES (keymap.tsx:112-117).
    match name {
        "enter" => "return",
        "esc" => "escape",
        "pgdown" => "pagedown",
        "pgup" => "pageup",
        n => n,
    }
}

fn named_code(name: &str) -> u32 {
    // ponytail: obvious control codes only; arrows/f-keys/home/etc map to 0.
    // Upgrade when wiring real terminal key codes.
    match name {
        "return" => 13,
        "tab" => 9,
        "escape" => 27,
        "space" => 32,
        "backspace" => 8,
        "delete" => 127,
        _ => 0,
    }
}

/// Parse one stroke like `"ctrl+c"`, `"shift+return"`, `"<leader>q"`,
/// with optional `:press` / `:release` suffix (TS carries event separately,
/// keybind.ts:20; suffix is this crate's convention). Case-insensitive,
/// `alt`/`option` alias `meta` (keymap.tsx:200-202), `cmd`/`win` alias
/// `super`. Single uppercase ASCII letter implies shift (`"E"` in keybind.ts:64).
/// Fail-closed on empty/over-long/unknown input.
pub fn parse_stroke(stroke: &str) -> Result<KeyEvent, KeyEventError> {
    if stroke.len() > MAX_STROKE {
        return Err(KeyEventError::TooLong);
    }
    let s = stroke.trim();
    if s.is_empty() {
        return Err(KeyEventError::Empty);
    }
    let (leader, s) = strip_leader_prefix(s);
    if leader && s.is_empty() {
        return Ok(KeyEvent {
            name: LEADER_TOKEN.to_string(),
            code: 0,
            ctrl: false,
            shift: false,
            meta: false,
            super_: false,
            hyper: false,
            event: KeyPress::Press,
        });
    }

    let (s, event) = match s.rsplit_once(':') {
        Some((head, tail)) => match tail.trim().to_ascii_lowercase().as_str() {
            "press" => (head, KeyPress::Press),
            "release" => (head, KeyPress::Release),
            _ => return Err(KeyEventError::UnknownEvent),
        },
        None => (s, KeyPress::Press),
    };

    let mut ctrl = false;
    let mut shift = false;
    let mut meta = false;
    let mut super_ = false;
    let mut hyper = false;
    let mut name: Option<&str> = None;
    for part in s.split('+') {
        let p = part.trim().to_ascii_lowercase();
        if p.is_empty() {
            return Err(KeyEventError::EmptyName);
        }
        match p.as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "meta" | "alt" | "option" => meta = true,
            "super" | "cmd" | "command" | "win" => super_ = true,
            "hyper" => hyper = true,
            _ if name.is_none() => name = Some(part.trim()),
            _ => return Err(KeyEventError::UnknownModifier),
        }
    }
    let raw = name.ok_or(KeyEventError::EmptyName)?;
    if raw.is_empty() || raw.len() > MAX_NAME {
        return Err(if raw.is_empty() {
            KeyEventError::EmptyName
        } else {
            KeyEventError::TooLong
        });
    }
    let mut owned = raw.to_ascii_lowercase();
    // Single uppercase ASCII letter implies shift (cf. "E", keybind.ts:64).
    if raw.len() == 1 {
        let b = raw.as_bytes()[0];
        if b.is_ascii_uppercase() {
            shift = true;
            owned = (b.to_ascii_lowercase() as char).to_string();
        }
    }
    let canonical = apply_alias(&owned);
    let code = if canonical.len() == 1 {
        canonical.as_bytes()[0] as u32
    } else {
        named_code(canonical)
    };
    Ok(KeyEvent {
        name: canonical.to_string(),
        code,
        ctrl,
        shift,
        meta,
        super_,
        hyper,
        event,
    })
}

/// Inverse of [`parse_stroke`]: canonical `ctrl+shift+alt+super+hyper+name`,
/// `meta` rendered as `alt` (display alias, keymap.tsx:200-202),
/// `:release` appended only for release.
#[must_use]
pub fn stringify_key_event(ev: &KeyEvent) -> String {
    let mut s = String::new();
    if ev.ctrl {
        s.push_str("ctrl+");
    }
    if ev.shift {
        s.push_str("shift+");
    }
    if ev.meta {
        s.push_str("alt+");
    }
    if ev.super_ {
        s.push_str("super+");
    }
    if ev.hyper {
        s.push_str("hyper+");
    }
    s.push_str(&ev.name);
    if ev.event == KeyPress::Release {
        s.push_str(":release");
    }
    s
}

/// Parse `stroke` and compare to `ev`. False on any parse failure.
#[must_use]
pub fn matches_stroke(ev: &KeyEvent, stroke: &str) -> bool {
    match parse_stroke(stroke) {
        Ok(other) => *ev == other,
        Err(_) => false,
    }
}

impl From<&KeyEvent> for Key {
    /// Maps onto the bridge `Key`: meta->alt; super/hyper dropped
    /// (ponytail: extend `Key` when modifiers needed downstream).
    fn from(ev: &KeyEvent) -> Self {
        Self::new(ev.code, ev.ctrl, ev.meta, ev.shift)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ctrl_c() {
        let ev = parse_stroke("ctrl+c").unwrap();
        assert_eq!(ev.name, "c");
        assert_eq!(ev.code, u32::from(b'c'));
        assert!(ev.ctrl && !ev.shift && !ev.meta && !ev.super_ && !ev.hyper);
        assert_eq!(ev.event, KeyPress::Press);
    }

    #[test]
    fn parse_release_event() {
        let ev = parse_stroke("ctrl+c:release").unwrap();
        assert_eq!(ev.event, KeyPress::Release);
        assert!(ev.ctrl && ev.name == "c");
    }

    #[test]
    fn leader_token() {
        assert!(is_leader_stroke("<leader>"));
        assert!(is_leader_stroke("<leader>q"));
        assert!(!is_leader_stroke("ctrl+c"));
        let (was, rest) = strip_leader_prefix("<leader>q");
        assert!(was && rest == "q");
        let bare = parse_stroke("<leader>").unwrap();
        assert!(bare.is_leader());
    }

    #[test]
    fn roundtrip_three_cases() {
        for stroke in ["ctrl+c", "shift+return", "super+shift+z:release"] {
            let ev = parse_stroke(stroke).unwrap();
            let back = parse_stroke(&stringify_key_event(&ev)).unwrap();
            assert_eq!(ev, back, "{stroke}");
        }
    }

    #[test]
    fn overlong_errs() {
        let long = "a".repeat(MAX_NAME + 1);
        assert_eq!(parse_stroke(&long), Err(KeyEventError::TooLong));
        let stroke = "x".repeat(MAX_STROKE + 1);
        assert_eq!(parse_stroke(&stroke), Err(KeyEventError::TooLong));
    }

    #[test]
    fn empty_errs() {
        assert_eq!(parse_stroke(""), Err(KeyEventError::Empty));
        assert_eq!(parse_stroke("   "), Err(KeyEventError::Empty));
        assert_eq!(parse_stroke("ctrl+"), Err(KeyEventError::EmptyName));
    }

    #[test]
    fn unknown_modifier_and_event_err() {
        assert_eq!(parse_stroke("foo+c"), Err(KeyEventError::UnknownModifier));
        assert_eq!(parse_stroke("c:bogus"), Err(KeyEventError::UnknownEvent));
    }

    #[test]
    fn matches_and_aliases() {
        let ev = parse_stroke("ctrl+c").unwrap();
        assert!(matches_stroke(&ev, "ctrl+c"));
        assert!(!matches_stroke(&ev, "ctrl+d"));
        assert!(!matches_stroke(&ev, "bogus+?!"));
        assert_eq!(parse_stroke("esc").unwrap().name, "escape");
        assert_eq!(parse_stroke("alt+x").unwrap().meta, true);
        assert_eq!(parse_stroke("E").unwrap(), parse_stroke("shift+e").unwrap());
        let k = Key::from(&ev);
        assert_eq!(k, Key::new(u32::from(b'c'), true, false, false));
    }
}
