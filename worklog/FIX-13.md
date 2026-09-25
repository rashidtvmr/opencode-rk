# FIX-13 worklog

## Claim
util_owners_full.rs canonical owner map, no logic moves.

## Source evidence
- crates/opentui-bridge/src/util_owners_full.rs:1-86 (forbid unsafe, enum UtilOwner 4 variants, canonical_of, owner_of, 3 tests)
- crates/opentui-bridge/src/json_persist.rs:29-49 write/read_json_atomic + validate_json_shape (brace-scan shape check lives here, canonical json_persist)
- crates/opentui-bridge/src/tool_meta.rs:19-69 ToolMeta + truncate/prefixed prefix-hack (canonical tool_meta)
- crates/opentui-bridge/src/error_format.rs:21-53 NativeError (canonical error_format)
- crates/opentui-bridge/src/debounce.rs:12 Debounced<T: Clone+PartialEq> NEW logic per its header
- crates/opentui-bridge/src/debounce_signal.rs:27 Debounced<T: Clone> canonical per task card; header docs dual at :14-20

## Observed scenario
File already meets spec on disk: Debounced -> debounce_signal, persist aliases -> json_persist, brace_scan/prefix_hack/structured_map aliases mapped, owner_of exhaustive over canonicals.

## Target boundary
Map only. No logic moved. std-only, no deps.

## Tests
Frozen tests in-file (3): canonicalizes_duplicate_implementations, maps_duplicate_symbols_to_their_owner, canonical_names_are_idempotent. No cargo run per scope (no cargo). rustfmt --check PASS.

## Decisions
Verify, no repair needed. 86 lines <= 90. No edit made.

## Remaining unknowns
None.
