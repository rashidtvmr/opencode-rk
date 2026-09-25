# BRIDGE-PAR-353

Unclaimed lane (orchestrator owns claims.json; file-only per task).
Claim: skipped (instructed not to touch ledger).
Source: packages/tui/src/component/prompt/autocomplete.tsx:1-50 (options memo).
Existing: crates/opentui-bridge/src/prompt_autocomplete.rs (CompleteList, distinct type, untouched).
Target: crates/opentui-bridge/src/comp_prompt_auto_full.rs - PromptAuto {cands, cursor}, push/move_cursor/selected.
Tests: 5 unit tests in-file. Decisions: wrapping cursor via rem_euclid; char-based 128 truncation; empty push rejected.
Unknowns: none. Integration: lib.rs wiring left to orchestrator (out of scope).
