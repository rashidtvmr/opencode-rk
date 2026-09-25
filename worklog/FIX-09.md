# FIX-09 scratchpad

Claim: plugin_host_full.rs standalone (no crate::plugin_host import).
Source: crates/opentui-bridge/src/plugin_host_full.rs:1-91; plugin_runtime.rs:41-60 (register bool/result pattern); plugin_slots.rs:101-117 (bounded registry).
Observed: old file imported crate::plugin_host::PluginHost + held PluginHost field + host()/host_mut() borrow API; non-standalone, >90 lines (153).
Target: standalone state machine, own count/state inline, enum HostState{Down,Up}, boot/shutdown/register()->bool cap64, is_booted, count, no-JS doc, forbid unsafe, std-only, <=90 lines, >=4 tests.
Tests: 4 tests in-module (new_is_down, boot_shutdown, register_bool_dup, caps_at_64). rustfmt --check PASS (90 lines).
Decisions: inline Vec<String> names registry (std, bounded dup check); dropped host()/host_mut() (coupling); kept name len cap 128 + cap 64 matching prior behavior; new() non-const (Vec::new const ok but struct non-const fn fine for fmt/compat).
Unknowns: none. No cargo run per scope (rustfmt only); no commit per scope.
