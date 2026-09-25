# BRIDGE-PAR-345 (unclaimed - ledger overflow, file-only)

Claim: none (orchestrator owns claims.json; instructed not to touch).
Source: packages/tui/src/feature-plugins/system/plugins.tsx:1-40 (plugin manager status/meta list); style ref crates/opentui-bridge/src/system_util_full.rs.
Target: crates/opentui-bridge/src/plugin_system_full.rs - PluginSystem {items cap16/128} + push/lines(width clip, cap18)/len.
Tests: 5 in-file (push_and_len, rejects_past_16, clips_item_to_128, lines_clip_width_and_cap, default_empty).
Decisions: std-only, forbid(unsafe_code), 89 lines; is_empty/Default extra (trivial); lines take(18) dead over cap-16 but matches spec.
Unknowns: none. No cargo run per scope; rustfmt only.
