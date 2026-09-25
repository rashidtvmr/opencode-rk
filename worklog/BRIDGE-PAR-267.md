# BRIDGE-PAR-267 scratchpad

claim: in-progress, session ses_par267, scratchpad worklog/BRIDGE-PAR-267.md
source: crates/opentui-bridge/src/permission_view.rs:40 PermissionView (tool/stage/note, trunc char-safe); crates/opentui-bridge/src/perm_gate.rs:8 PermGate{ctx}, gate_summary(id)->"id granted|pending" capped 128
boundary: ONE new file crates/opentui-bridge/src/permission_view_full.rs. No lib.rs/Cargo.toml/permission_view.rs/perm_gate.rs edits. No cargo/commit.
target: PermView{gate:PermGate} + lines(&self,id,width)->Vec<String> (summary+hint, char-safe width clip, cap 4) + count()->usize (=1). std-only, forbid(unsafe_code), <110 lines, >=4 tests.
tests: 5 unit tests in-file (summary row, hint row, width clip char-safe incl multibyte, cap 4, count==1).
decision: hint row "[a]llow [d]eny"; width 0 -> empty strings; count constant 1 (single view).
unknowns: none.
