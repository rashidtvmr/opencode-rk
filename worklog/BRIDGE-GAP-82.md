# BRIDGE-GAP-82
claim: ses_gap82 in-progress
source: packages/opencode/src/cli/cmd/run/variant.shared.ts:61-76 cycleVariant, 82-92 fitVariant, 97-114 resolveVariant
target: crates/opentui-bridge/src/run_variant_shared.rs only
tests: select_ok_moves_cursor, select_oob_false_keeps_cursor, current_none_when_empty, set_model_ok_truncates, caps_truncate_fields_and_rows
verify: rustfmt --check PASS, 158 lines, forbid unsafe, std-only
