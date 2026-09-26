# BRIDGE-PAR-155 scratchpad

claim: BRIDGE-PAR-155 ses_par155 worklog/BRIDGE-PAR-155.md
source: packages/tui/src/context/runtime.tsx (provider/default pattern, 62L) + crates/opentui-bridge/src/tui_runtime.rs (TuiRuntime flags mirror)
target: crates/opentui-bridge/src/runtime_ctx.rs only
impl: RuntimeCtx {cols,rows,ready} + resize->bool + mark_ready + dims + is_ready; Default 80x24 not ready; forbid(unsafe_code)
tests: 6 in-file (default_not_ready, zero_cols_false, zero_rows_false, resize_ok, ready_flow, dims_roundtrip)
verify: rustfmt --check PASS, 101 lines (<120)
