# BRIDGE-PAR-135 scratchpad

Claim: BRIDGE-PAR-135 via ses_par135. Owned file: crates/opentui-bridge/src/home_footer.rs.
Source: packages/tui/src/feature-plugins/home/footer.tsx (100 lines, home_footer slot: Directory/Mcp/spacer/Version).
Boundary: HomeFooter rotator only. No lib.rs/Cargo.toml/home_plugin.rs/system_plugins.rs edits.
Tests: 6 in-file (empty_none, wraps, render_clip, single_stable, cap_trunc_16, tip_len_capped).
Decisions: char-safe trunc via chars().take; new() truncates to 16 tips x 256 chars; next wraps; render clips to width, empty->"".
Evidence: rustfmt --check PASS, 116 lines (<140), forbid(unsafe_code), std-only.
