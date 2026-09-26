# BRIDGE-PAR-131 scratchpad

Claim: BRIDGE-PAR-131 via ses_par131. Status: in-progress -> completed on fmt pass.
Source: packages/tui/src/plugin/command-shim.ts:49-65 (toCommand name/desc/run), :85+ createCommandShim.
Boundary: new file only crates/opentui-bridge/src/command_shim.rs. lib.rs untouched (no wiring per scope).
Tests: 7 in-file (register ok/trunc/empty/dup/cap, invoke ok/missing). Spec asked >=5.
Decisions: truncate byte-wise (ASCII ids); empty checked post-truncate; `commands()` accessor extra for inspection.
Unknowns: none. Integration/wiring left to orchestrator.
