# FIX-18 scratchpad

Claim: Workflow schema = named skill (V2 design).
Source: sources/req017-extensibility-ownership-gap.json:189 (workflows under skills, no separate user command config).
Target: crates/opentui-bridge/src/workflow_schema_full.rs (created, 99 lines).
Contract: WfStep{name cap64, kind cap32}, Workflow{name cap64, steps cap32}, add_step fail-closed (blank/dup/cap -> false), validate (non-empty name, >=1 step, no dup).
Bounds: char-safe trunc (chars().take), std-only, forbid(unsafe_code), 5 tests.
Verify: rustfmt --edition 2021 --check -> FMT_OK. No cargo per scope.
