# FIX-11: run_manifest_full.rs verify

## Claim
`crates/opentui-bridge/src/run_manifest_full.rs` already meets contract; no edit needed.

## Evidence
- File: `crates/opentui-bridge/src/run_manifest_full.rs:1` `#![forbid(unsafe_code)]`
- `RUN_MODULES` lines 4-42: counted 37 entries (demo..footer.view)
- `module_count()` lines 44-47 returns `RUN_MODULES.len()`; test `count_is_frozen` asserts 37
- `has_module` lines 49-52 via `contains`; 3 tests (count/present/absent), lines 54-75
- Total 75 lines (<=80); std-only (no `use`, no deps); names only, zero callers by design
- `rtk rustfmt --check crates/opentui-bridge/src/run_manifest_full.rs` exit 0, clean
- `run_footer.rs:15-90`, `run_runtime.rs:10-46`, `run_stream.rs:15-57` read: sibling impls only, no manifest dependency

## Decision
No repair. File frozen as-is.

## Remaining
None for this lane.
