# BRIDGE-PAR-221 scratchpad

Claim: BRIDGE-PAR-221, session ses_par221.
Source evidence:
- TS truth /home/rashid/projects/opencode/packages/tui/src/util/signal.ts:1-51 (createSignal/createMemo/createEffect via solid-js; createDebouncedSignal, createFadeIn smoothstep 160ms).
- crate::signal_graph crates/opentui-bridge/src/signal_graph.rs:1-140 (graph bookkeeping i64 cells, read-only, NOT edited). Style ref debounce_full.rs:1-144 (forbid unsafe, ponytail note).
Target boundary: ONE new file crates/opentui-bridge/src/solid_signal_full.rs. No lib.rs/Cargo.toml/signal_graph.rs edits. No cargo/commit.
Tests: 6 in-file (new_caps, set_bumps, set_caps, memo_capped, version_wraps, default_empty).
Decisions: owned String cell + u64 version, chars().take(4096) cap (not bytes, unicode-safe); memo pure f()->String capped; wrapping_add version.
Unknowns: wiring into signal_graph/lib.rs left to integrator.
