# BRIDGE-PAR-302 scratchpad

claim: BRIDGE-PAR-302 via cc.claim session ses_par302 OK
source: packages/tui/src/util/record.ts:1 `isRecord` (plain object, non-array)
target: crates/opentui-bridge/src/record_util_full.rs, RecordMem only
tests: 6 (insert_and_has, duplicate_rejected, cap_at_64, remove_roundtrip, key_capped_at_128, empty_rejected)
decisions: Vec<String> set, norm truncates 128 chars, empty rejected, dup/full false
unknowns: none
