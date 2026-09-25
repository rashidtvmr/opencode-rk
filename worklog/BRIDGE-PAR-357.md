# BRIDGE-PAR-357 scratchpad (UNCLAIMED - orchestrator owns ledger)

Claim: task BRIDGE-PAR-357, no ledger claim per spawn orders (file-only lane).
Source: packages/tui/src/component/prompt/local-attachment.ts:1-48 (mime table + accept rule); type is new list, not in TS.
Target: crates/opentui-bridge/src/comp_prompt_attach_full.rs - AttachList, cap 16, path 512B.
Tests: 4 tests in-file (add_and_len, rejects empty/long, dup+full, remove oob).
Decisions: dedupe reject (cheap, avoids dup attach); byte-len check (paths are bytes); is_empty helper extra (harmless).
Unknowns: wiring into lib.rs left to orchestrator (scope forbids).
Verification: rustfmt --check pass, 89 lines (<90).
