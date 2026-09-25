# BRIDGE-PAR-399 scratchpad (UNCLAIMED - orchestrator owns ledger, no claim made)

Claim: bounded TSX adapter registry mirroring adapters.tsx Input keys.
Source: packages/tui/src/plugin/adapters.tsx:23 (Input), :173 (createTuiApiAdapters).
Pattern: crates/opentui-bridge/src/plugin_adapter_full.rs (MAX_MOUNTED 16), plugin_adapters.rs (MAX_NAME 64, trunc).
Target: crates/opentui-bridge/src/plugin_adapt_tsx_full.rs only. No lib.rs/Cargo.toml edits.
Tests: ok_has_len, dup_empty_false, cap_trunc (3 tests).
Decisions: struct AdaptReg {names} + register/has/len; cap 16, trunc 64 char-boundary safe; dup/empty/full false.
Verify: rustfmt --check PASS, 79 lines (<80), forbid(unsafe_code), std-only.
Status: file-only complete, ledger untouched per instruction.
