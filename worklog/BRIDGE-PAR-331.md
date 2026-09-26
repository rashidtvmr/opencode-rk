# BRIDGE-PAR-331 scratchpad

Claim: BRIDGE-PAR-331, session ses_par331, ledger in-progress.
Source evidence:
- TS truth `packages/tui/src/component/plugin-route-missing.tsx:8` `Unknown plugin route: {props.id}` + `:10` `go home` (14 lines total).
- Style ref `crates/opentui-bridge/src/fork_route.rs:1` (`#![forbid(unsafe_code)]`, trunc-on-boundary, cfg(test)).
Target boundary: ONE new file `crates/opentui-bridge/src/route_missing_full.rs`. No lib.rs, no Cargo.toml, no cargo, no commit.
Tests: formats_id, caps_length, hint_and_detect (frozen).
Decisions: `PREFIX` const; `route_missing_line` trims route, formats, truncates to 256 on char boundary; `missing_hint` static "go home"; `is_missing` prefix check.
Unknowns: none.
