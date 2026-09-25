# BRIDGE-PAR-250 scratchpad

Claim: BRIDGE-PAR-250 via completion_claims, session ses_par250.
Source evidence:
- crates/opentui-bridge/src/theme_assets.rs:43 is_known, 48 fallback_chain, 6 ASSET_NAMES (33, sorted).
- crates/opentui-bridge/src/theme_picker.rs:7 ThemePicker (read only, not edited).
Target boundary: ONE new file crates/opentui-bridge/src/theme_assets_full.rs. No lib.rs/Cargo.toml/theme_assets.rs/theme_picker.rs edits. No cargo, no commit.
Tests: 6 inline unit tests (register ok, reject known, reject empty/long, dup+cap16, resolve known/fallback, list 8+custom).
Decisions:
- register rejects known names via fallback_chain(name)[0] != name check (known returns itself, unknown returns opencode). So unknown custom accepted, known rejected. Empty/len>64 rejected. Dup/cap16 rejected.
- resolve: custom hit as-is, else fallback_chain(name)[0] (known as-is, unknown opencode).
- list: ASSET_NAMES first 8 + custom.
- ponytail comment included. forbid(unsafe_code). std-only.
Remaining: rustfmt --check raw exit code; line count under 120.
