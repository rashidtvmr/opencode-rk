# BRIDGE-053 keymap_format scratchpad

Claim: display-format twin of keymap.tsx:112-134, :180-212 + extras 0.4.3.
Source: opencode @ a0d9b6c keymap.tsx, config/keybind.ts:41, config/index.tsx:110;
vendored @opentui/keymap 0.4.3 tarball (extras/index.js, chunks/index-frk6sdcd.js:90-109).
Observed: stringify canonical = ctrl/shift/meta/super/hyper+name, return shown enter;
formatStroke: token->tokenDisplay, meta->alt, pageup->pgup/pagedown->pgdn/delete->del;
formatKeySequence joins " ", formatCommandBindings joins ", " dedupe default.
Target: crates/opentui-bridge/src/keymap_format.rs only, std only, no cargo run.
Tests: 7 frozen literal-string tests. logically green (RED n/a offline, no cargo per scope).
Decisions: BindingLookup thin map (get/has); format_command_bindings returns Vec not
joined string (palette rows). expand_alias returns String (TS undefined = unchanged).
Unknowns: multi-stroke leader display beyond single token; gather/pick/omit deferred.
