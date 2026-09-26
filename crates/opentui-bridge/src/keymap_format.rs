#![forbid(unsafe_code)]
//! Display formatting for keymap bindings (parse/display split).
//!
//! TS sources (local checkout /home/rashid/projects/opencode @ a0d9b6c,
//! diverges from pinned 95daf90; line numbers below are a0d9b6c):
//! - `packages/tui/src/keymap.tsx:112-117` KEY_ALIASES:
//!   enter->return, esc->escape, pgdown->pagedown, pgup->pageup.
//! - `keymap.tsx:119-126` expandKeyAliases: whole-input replace with
//!   `(^|[+,\s>])alias(?=$|[+,\s<])` gi per pair; no-op returns undefined.
//! - `keymap.tsx:128-134` registerKeyAliases via `appendBindingExpander`.
//! - `keymap.tsx:10` `stringifyKeyStroke` from `@opentui/keymap`; canonical
//!   form is `ctrl+shift+meta+super+hyper+name` with `return` shown as
//!   `enter` (vendored 0.4.3 `chunks/index-frk6sdcd.js:90-109`, line 107).
//! - `keymap.tsx:11-14,206-212` extras wrappers: `formatKeySequenceExtra`
//!   / `formatCommandBindingsExtra` from `@opentui/keymap/extras`.
//! - `keymap.tsx:190-204` formatOptions: tokenDisplay `{leader: leaderDisplay}`,
//!   keyNameAliases `{pageup:pgup, pagedown:pgdn, delete:del}`,
//!   modifierAliases `{meta:alt}`.
//! - `keymap.tsx:180-184` leaderDisplay: leader binding key shown raw when
//!   string, else via `stringifyKeyStroke`; falls back to LeaderDefault.
//! - `keymap.tsx:20` LEADER_TOKEN `"leader"`; `keybind.ts:41` LeaderDefault
//!   `"ctrl+x"`; `config/index.tsx:110` `createBindingLookup` (lookup itself
//!   stays a thin map here; extras semantics mirrored in docs).
//! - extras `index.js:257-307` formatStroke: token parts use tokenDisplay,
//!   stroke parts join aliased modifiers in ctrl/shift/meta/super/hyper
//!   order, `return` flips to `enter` before keyNameAliases lookup.
//! - extras `index.js:308-317` formatKeySequence joins strokes with `" "`.
//! - extras `index.js:318-339` formatCommandBindings joins sequences with
//!   `", "`, dedupes by default, skips empties.
//!
//! Reuses `crate::key_event::{KeyEvent, LEADER_TOKEN}`,
//! `crate::keymap::LEADER_DEFAULT`, `crate::keymap_host::Addon` is NOT
//! redefined here (addon set lives in `keymap_host`).
//! (ponytail: Vec-per-command lookup only; full extras gather/pick/omit
//! ported when Rust-side palette needs them.)

use std::collections::HashMap;

use crate::key_event::{KeyEvent, LEADER_TOKEN};
use crate::keymap::LEADER_DEFAULT;

/// Alias -> canonical key, insertion-ordered as TS
/// `Object.entries(KEY_ALIASES)` (keymap.tsx:112-117).
pub const KEY_ALIASES: [(&str, &str); 4] =
    [("enter", "return"), ("esc", "escape"), ("pgdown", "pagedown"), ("pgup", "pageup")];

/// Leader token name, mirrors `LEADER_TOKEN` (keymap.tsx:20).
pub use crate::key_event::LEADER_TOKEN as LEADER;

/// Default leader display, mirrors LeaderDefault (keybind.ts:41).
#[must_use]
pub fn default_leader_display() -> &'static str {
    LEADER_DEFAULT
}

fn is_before(c: char) -> bool {
    c == '+' || c == ',' || c == '>' || c.is_whitespace()
}

fn is_after(c: char) -> bool {
    c == '+' || c == ',' || c == '<' || c.is_whitespace()
}

fn expand_one(s: &str, alias: &str, key: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        if rest.len() >= alias.len()
            && rest[..alias.len()].eq_ignore_ascii_case(alias)
            && (i == 0 || s[..i].chars().next_back().is_some_and(is_before))
            && (i + alias.len() == s.len()
                || s[i + alias.len()..].chars().next().is_some_and(is_after))
        {
            out.push_str(key);
            i += alias.len();
        } else {
            let c = rest.chars().next().unwrap_or('\0');
            out.push(c);
            i += c.len_utf8().max(1);
        }
    }
    out
}

/// Whole-input alias expansion, mirrors `expandKeyAliases`
/// (keymap.tsx:119-126): each pair applied in table order with
/// `(^|[+,\s>])alias(?=$|[+,\s<])` gi boundaries; canonical form is
/// lowercase. Returns the input unchanged when nothing matches
/// (TS returns undefined; here the identical String).
#[must_use]
pub fn expand_alias(input: &str) -> String {
    let mut out = input.to_string();
    for (alias, key) in KEY_ALIASES {
        out = expand_one(&out, alias, key);
    }
    out
}

/// Canonical modifier set for [`stringify_key_stroke`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub meta: bool,
    pub super_: bool,
    pub hyper: bool,
}

impl Modifiers {
    #[must_use]
    pub fn from_event(ev: &KeyEvent) -> Self {
        Self {
            ctrl: ev.ctrl,
            shift: ev.shift,
            meta: ev.meta,
            super_: ev.super_,
            hyper: ev.hyper,
        }
    }
}

/// Canonical stroke string, mirrors `stringifyCanonicalStroke`
/// (vendored 0.4.3 `chunks/index-frk6sdcd.js:90-109`): modifiers in
/// ctrl/shift/meta/super/hyper order, `meta` NOT aliased, `return`
/// shown as `enter`, `+`-joined.
#[must_use]
pub fn stringify_key_stroke(mods: &Modifiers, key: &str) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(6);
    if mods.ctrl {
        parts.push("ctrl");
    }
    if mods.shift {
        parts.push("shift");
    }
    if mods.meta {
        parts.push("meta");
    }
    if mods.super_ {
        parts.push("super");
    }
    if mods.hyper {
        parts.push("hyper");
    }
    parts.push(if key == "return" { "enter" } else { key });
    parts.join("+")
}

fn display_name(name: &str) -> &str {
    // return->enter flip (extras index.js:304) then keyNameAliases
    // (keymap.tsx:195-199).
    let base = if name == "return" { "enter" } else { name };
    match base {
        "pageup" => "pgup",
        "pagedown" => "pgdn",
        "delete" => "del",
        _ => base,
    }
}

/// One display stroke, mirrors `formatStroke` (extras `index.js:257-307`):
/// leader token renders as `leader_display` (tokenDisplay,
/// keymap.tsx:192-194); else modifiers in ctrl/shift/meta/super/hyper
/// order with `meta`->`alt` (modifierAliases, keymap.tsx:200-202).
#[must_use]
pub fn format_stroke(ev: &KeyEvent, leader_display: &str) -> String {
    if ev.name == LEADER_TOKEN
        && !ev.ctrl
        && !ev.shift
        && !ev.meta
        && !ev.super_
        && !ev.hyper
    {
        return leader_display.to_string();
    }
    let mut s = String::new();
    let mut n = 0;
    let mut push = |s: &mut String, n: &mut u32, m: &str| {
        if *n > 0 {
            s.push('+');
        }
        s.push_str(m);
        *n += 1;
    };
    if ev.ctrl {
        push(&mut s, &mut n, "ctrl");
    }
    if ev.shift {
        push(&mut s, &mut n, "shift");
    }
    if ev.meta {
        push(&mut s, &mut n, "alt");
    }
    if ev.super_ {
        push(&mut s, &mut n, "super");
    }
    if ev.hyper {
        push(&mut s, &mut n, "hyper");
    }
    let key = display_name(&ev.name);
    if n == 0 {
        return key.to_string();
    }
    s.push('+');
    s.push_str(key);
    s
}

/// Display sequence, mirrors `formatKeySequenceExtra` with
/// `formatOptions` (keymap.tsx:206-208): strokes joined with `" "`
/// (extras `index.js:311`).
#[must_use]
pub fn format_key_sequence(keys: &[KeyEvent], leader_display: &str) -> String {
    keys.iter().map(|k| format_stroke(k, leader_display)).collect::<Vec<_>>().join(" ")
}

/// Command -> sequences lookup. Thin map over caller-provided data;
/// mirrors `createBindingLookup` get/has surface (config/index.tsx:110,
/// extras `index.js:136-215`) without the extras gather/pick/omit cache.
#[derive(Debug, Clone, Default)]
pub struct BindingLookup {
    map: HashMap<String, Vec<Vec<KeyEvent>>>,
}

impl BindingLookup {
    #[must_use]
    pub fn new(map: HashMap<String, Vec<Vec<KeyEvent>>>) -> Self {
        Self { map }
    }

    #[must_use]
    pub fn get(&self, command: &str) -> &[Vec<KeyEvent>] {
        self.map.get(command).map(Vec::as_slice).unwrap_or(&[])
    }

    #[must_use]
    pub fn has(&self, command: &str) -> bool {
        self.map.contains_key(command)
    }
}

/// Display strings for one command, mirrors `formatCommandBindingsExtra`
/// (keymap.tsx:210-212): one entry per distinct sequence, order-kept
/// dedupe (extras `index.js:322-332`), empties skipped. (TS joins with
/// `", "`; here a Vec keeps entries separable for palette rows.)
#[must_use]
pub fn format_command_bindings(
    lookup: &BindingLookup,
    command: &str,
    leader_display: &str,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for seq in lookup.get(command) {
        let s = format_key_sequence(seq, leader_display);
        if s.is_empty() || out.contains(&s) {
            continue;
        }
        out.push(s);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key_event::parse_stroke;

    #[test]
    fn alias_table_matches_ts_keymap_112_117() {
        assert_eq!(
            KEY_ALIASES,
            [("enter", "return"), ("esc", "escape"), ("pgdown", "pagedown"), ("pgup", "pageup")]
        );
    }

    #[test]
    fn expand_alias_single_sequence_case_insensitive() {
        assert_eq!(expand_alias("esc"), "escape");
        assert_eq!(expand_alias("ESC"), "escape");
        assert_eq!(expand_alias("ctrl+esc,q"), "ctrl+escape,q");
        assert_eq!(expand_alias("pgup,pgdown"), "pageup,pagedown");
        assert_eq!(expand_alias("enter"), "return");
        // '<' is not a before-boundary (keymap.tsx:121), so no expansion.
        assert_eq!(expand_alias("<leader>q"), "<leader>q");
        assert_eq!(expand_alias("ctrl+c"), "ctrl+c");
    }

    #[test]
    fn stringify_canonical_matches_ts() {
        let mods = Modifiers { ctrl: true, ..Modifiers::default() };
        assert_eq!(stringify_key_stroke(&mods, "c"), "ctrl+c");
        let mods = Modifiers { shift: true, ..Modifiers::default() };
        assert_eq!(stringify_key_stroke(&mods, "return"), "shift+enter");
        let mods = Modifiers { meta: true, ..Modifiers::default() };
        assert_eq!(stringify_key_stroke(&mods, "x"), "meta+x");
        let ev = parse_stroke("ctrl+c").unwrap();
        assert_eq!(
            stringify_key_stroke(&Modifiers::from_event(&ev), &ev.name),
            "ctrl+c"
        );
    }

    #[test]
    fn format_display_aliases_match_ts_format_options() {
        // keyNameAliases pageup->pgup, pagedown->pgdn, delete->del
        // (keymap.tsx:195-199); modifierAliases meta->alt (keymap.tsx:200-202).
        assert_eq!(format_stroke(&parse_stroke("pageup").unwrap(), "ctrl+x"), "pgup");
        assert_eq!(format_stroke(&parse_stroke("pagedown").unwrap(), "ctrl+x"), "pgdn");
        assert_eq!(format_stroke(&parse_stroke("delete").unwrap(), "ctrl+x"), "del");
        assert_eq!(
            format_stroke(&parse_stroke("ctrl+alt+b").unwrap(), "ctrl+x"),
            "ctrl+alt+b"
        );
        // return flips to enter on display (extras index.js:304).
        assert_eq!(format_stroke(&parse_stroke("shift+return").unwrap(), "ctrl+x"), "shift+enter");
        // bare leader renders via tokenDisplay (keymap.tsx:192-194).
        assert_eq!(format_stroke(&parse_stroke("<leader>").unwrap(), "ctrl+x"), "ctrl+x");
    }

    #[test]
    fn format_sequence_joins_with_space() {
        let seq = vec![parse_stroke("<leader>").unwrap(), parse_stroke("q").unwrap()];
        assert_eq!(format_key_sequence(&seq, "ctrl+x"), "ctrl+x q");
        let seq = vec![parse_stroke("ctrl+c").unwrap(), parse_stroke("ctrl+d").unwrap()];
        assert_eq!(format_key_sequence(&seq, "ctrl+x"), "ctrl+c ctrl+d");
        assert_eq!(format_key_sequence(&[], "ctrl+x"), "");
    }

    #[test]
    fn lookup_get_has_and_command_bindings() {
        let mut map = HashMap::new();
        map.insert(
            "command.palette.show".to_string(),
            vec![
                vec![parse_stroke("ctrl+p").unwrap()],
                vec![parse_stroke("ctrl+p").unwrap()],
            ],
        );
        let lookup = BindingLookup::new(map);
        assert!(lookup.has("command.palette.show"));
        assert!(!lookup.has("nope"));
        assert!(lookup.get("nope").is_empty());
        // dedupe keeps one "ctrl+p" (extras index.js:322-332).
        assert_eq!(
            format_command_bindings(&lookup, "command.palette.show", "ctrl+x"),
            vec!["ctrl+p".to_string()]
        );
        assert!(format_command_bindings(&lookup, "nope", "ctrl+x").is_empty());
    }

    #[test]
    fn enter_roundtrip_parse_canonical_display_enter() {
        // KEY_ALIASES enter->return at parse; display flips back to enter.
        let ev = parse_stroke("enter").unwrap();
        assert_eq!(ev.name, "return");
        assert_eq!(format_stroke(&ev, default_leader_display()), "enter");
        assert_eq!(default_leader_display(), LEADER_DEFAULT);
    }
}
