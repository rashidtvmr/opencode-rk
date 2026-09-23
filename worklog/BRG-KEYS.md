# BRG-KEYS Worklog

## Claim
- Task: BRG-KEYS
- Session: ses_brg_keys
- Owned file: crates/opentui-bridge/src/input.rs
- Status: in-progress

## Source Evidence
Sources (from task description):
- opentui/packages/core/src/lib/parse.keypress.ts (CSI sequences, meta+character, ctrl keys, fn keys via regex, modifyOtherKeys)
- opentui/packages/core/src/lib/parse.keypress-kitty.ts (Kitty keyboard protocol: CSI unicode-key-code:shifted:base ; modifiers:event-type ; text u)
- KeyHandler.ts (processParsedKey -> emit keypress / keyrelease based on eventType)
- paste.ts (bracketed paste: ESC[200~ start, ESC[201~ end)
- keybinding.internal.ts (KeyBindingLookup with baseCode, getKeyBindingKeys tries name + baseCodeName)

Key observations:
1. parse.keypress.ts line 218-219: single byte > 127 (high-bit set, like Latin-1 accented chars) gets 128 subtracted and prefixed with ESC (meta). This is the "meta high bit" legacy behavior.
2. parse.keypress.ts line 308: `getCtrlKeyName` maps charCodes: 0->space, 1-26->a-z, 28-31->@,\,],^,_
3. parse.keypress.ts line 371-375: backspace handling for \x7f
4. parse.keypress.ts line 398: `s.length === 1 || (s.length === 2 && s.codePointAt(0) > 0xffff)` - handles surrogate pairs (emoji above BMP)
5. parse.keypress-kitty.ts: Kitty protocol format is `\x1b[<unicode-key-code[:shifted:base]>;<mods:event-type>[;text]u`
6. Kitty special keys: `\x1b[1;mods:event LETTER` (arrows, etc.) or `\x1b[number;mods:event~`
7. Kitty modifier mask (line 165-184): bit 1=shift, 2=alt, 4=ctrl, 8=super, 16=hyper, 32=meta, 64=capsLock, 128=numLock. Mod value is 1+mask.
8. parse.keypress.ts line 289: bracketed paste markers `ESC[200~` and `ESC[201~` return null (not keypress)
9. parse.keypress.ts line 280: focus events `ESC[I` and `ESC[O` return null

## Target Boundary
Rewrite input.rs to add:
- `pub struct ParsedKey` (mirrors TS interface)
- `pub fn parse_keypress(s: &str) -> Option<ParsedKey>` (handles CSI, kitty, ctrl, meta, UTF-8 multibyte)
- `pub fn parse_kitty_keypress(s: &str) -> Option<ParsedKey>` (kitsy protocol)
- `pub struct KeyHandler` (dispatches parsed keys, global-first)
- `pub enum KeyDecision` (dispatch result)

Min 120 lines, >= 8 tests, std-only, forbid(unsafe_code).

## Design Decisions
1. ParsedKey: name, ctrl, meta, shift, option, sequence, number, raw, source ("raw"|"kitty"), code, super, hyper, caps_lock, num_lock, base_code, repeated
2. KeyEventType -> enum { Press, Repeat, Release }
3. parse_keypress: Kitty checked first (when prefixed), then modifyOtherKeys, then regular CSI/fn/ctrl/meta logic, with UTF-8 multibyte support (iterate bytes properly, not char::from(b))
4. KeyHandler: holds optional global handler, optional renderable handler. process_parsed_key emits in priority order (global first, then renderable). Returns KeyDecision.
5. KeyDecision: NotHandled, HandledByDefault, Handled, PropagationStopped (or similar)
6. Bracketed paste: parse_keypress returns None for ESC[200~ / ESC[201~

## Remaining Unknowns
- Exact KeyDecision enum variants (task just says "pub enum KeyDecision" - will define reasonable variants)
