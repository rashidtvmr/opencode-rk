# BRIDGE-GAP-37 scratchpad (ses_gap37)

Claim: BRIDGE-GAP-37 via completion_claims, session ses_gap37.
Source evidence:
- crates/opentui-bridge/src/theme.rs:184 `names()`, :242 `get()`, 52 slots.
- crates/opentui-bridge/src/theme_registry.rs:104 `ThemeRegistry`, :117 `add`, :154 `list`, :20 `MAX_THEMES=64`.
- crates/opentui-bridge/src/buffer.rs:231 `border_chars(BorderStyle)`, :234 single 0x250C, :238 double 0x2554, :242 rounded 0x256D.
Target boundary: ONE new file theme_utils.rs. No lib.rs/Cargo.toml edits. No cargo. No commit/push.
Tests: known_slot_some, unknown_slot_none, pack_sorted_capped, border_single_rounded, fallback_nonempty, registry_roundtrip_unused_rgba.
Decisions: pack_options takes &ThemeRegistry (zero-arg cannot list names); border unknown -> single; fallback unknown -> "text"; ponytail free fns.
Unknowns: lib.rs mod wiring left to integrator (out of scope).
