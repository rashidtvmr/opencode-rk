# BRIDGE-GAP-06

## Claim
- Task: `BRIDGE-GAP-06`
- Session: `ses_gap06`
- Owned file: `crates/opentui-bridge/src/json_persist.rs`
- Scratchpad: `worklog/BRIDGE-GAP-06.md`

## Source evidence
- `BRIDGE_MIGRATION_DETAIL.md:247-252` identifies `writeJsonAtomic` as a missing bridge surface.
- `crates/opentui-bridge/src/persistence.rs:54-118` provides same-directory temporary-file plus rename, 1 MiB bound, and cleanup.
- `crates/opentui-bridge/src/json_persist.rs:2-16` documents the native contract and upstream reference.

## Contract
- `write_json_atomic` validates outer JSON object/array shape before disk mutation, then delegates atomic text persistence.
- `read_json_atomic` reads bounded text and revalidates shape.
- Rejection leaves no target file; failed replacement preserves the original; no stray temporary file remains.

## Observed scenario
- Existing persistence helper already supplies same-directory temporary replacement, bounded reads/writes, and cleanup.
- `BRIDGE_MIGRATION_DETAIL.md:249` names the missing `writeJsonAtomic` surface; this file supplies the bridge API without changing shared persistence.

## Decisions
- Reuse `crate::persistence`; no new dependency, parser, IO implementation, or process side effect.
- Keep JSON validation shape-only, matching the existing std-only bridge boundary.

## Tests
Five in-file tests cover object/array round trips, malformed and trailing input, empty input, oversize input, and failed-write preservation.

## Verification
- `rtk timeout 120 rustc --edition=2021 --test /home/rashid/.cache/bun-tmp/opencode/bridge-gap06-wrapper.rs -o /home/rashid/.cache/bun-tmp/opencode/bridge-gap06 && rtk timeout 120 /home/rashid/.cache/bun-tmp/opencode/bridge-gap06 --test-threads=1`: 13 passed, 0 failed; 5 JSON tests plus 8 persistence tests.
- `rtk rustfmt --check --edition 2021 crates/opentui-bridge/src/json_persist.rs`: clean.
- `write_json_atomic` line 29, `read_json_atomic` line 41.
- Stale validation comment corrected to describe the balance scan accurately.
- No cargo; no lib.rs change.
