# BRIDGE-GAP-38 scratchpad (ses_gap38)

Claim: BRIDGE-GAP-38 via tools/completion_claims.py, session ses_gap38.
Owned file: crates/opentui-bridge/src/theme_resolve_full.rs (new, only file).
No edits to lib.rs / Cargo.toml. No cargo run. No commit/push (overridden by task scope).

Source evidence: task spec only (zero-context lane). No repo reads besides WORKER.md.

Target boundary: ThemeDef {name, extends, dark, light} + variant_pick + resolve_chain,
8-hop cap, cycle/missing fail-closed None, std-only, forbid(unsafe_code), <200 lines.

Tests (in-file, 7): direct_dark, direct_light, chain_two_hop, child_value_wins,
cycle_none, missing_none, depth_cap_none.

Decisions:
- Empty variant = inherit; first non-empty from start wins.
- Cycle/depth-cap strict None even if early value exists (fail-closed).
- Dangling parent ref strict None via `?` (broken theme).
- Self-extends counts as cycle -> None.

Verification: rustfmt --check only (no cargo per scope).
