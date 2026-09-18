# LANE-THEMES scratchpad

## Claim
- Task: LANE-THEMES
- Session: ses_worker_themes
- Owned file: crates/cli/src/native_theme.rs (NEW)
- Status: completed

## Source evidence
- native_palette.rs:233-260 — existing `Rgb` type, `contrast_ratio`, `Theme` enum
- native_status.rs: bounded status state pattern
- native_palette.rs:332-340 — `Selection` with theme field
- Consensus: `#![forbid(unsafe_code)]`, std-only, BTreeMap for sorted keys

## Target boundary
- ThemeDef {name, fg, bg, accent, success, warning, error, muted: Rgba}
- ThemeRegistry bounded MAX_THEMES=32, name<=32 bytes
- ThemeEngine::apply(theme) -> BoundedColorMap (8 semantic keys)
- parse_hex "#RRGGBB" fail-closed
- No external crates, no IO, no threads

## Tests written (frozen)
- T01 hex_parse_valid, hex_parse_missing_hash, hex_parse_invalid_length, hex_parse_invalid_char
- T02 registry_register_and_lookup, registry_bounded_at_max, registry_rejects_duplicate_name, registry_rejects_empty_name, registry_rejects_long_name, registry_names_sorted
- T03 apply_produces_all_semantic_keys, apply_values_match_theme_fields
- T04 unknown_theme_returns_error
- T05 theme_switch_preserves_bounded_memory
- T06 builtins_dark_and_light_present, builtins_apply_complete

## Verification
- 16/16 tests pass, zero warnings
- cmd: rustc --edition 2021 --test src/native_theme.rs

## Decisions
- Used `Rgba` with alpha=0xFF (opaque) since task spec says Rgba
- Border color computed as weighted blend of fg/bg for completeness
- BTreeMap for sorted keys in registry
- Fail-closed: all hex/registry errors are typed enums, no panics

## Remaining
- Integrator must add `pub mod native_theme;` to main.rs and wire into native_palette/native_status render state
