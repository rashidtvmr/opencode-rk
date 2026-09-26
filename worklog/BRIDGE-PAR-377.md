# BRIDGE-PAR-377 (unclaimed, file-only per orchestrator)

Claim: none. Orchestrator owns claims.json. Proceeding file-only.
Source: packages/tui/src/context/epilogue.tsx:1-6 (createSimpleContext, set(value?: string)).
Existing: crates/opentui-bridge/src/epilogue_ctx.rs (multi-line EpilogueCtx, 16 lines cap) - new file is distinct single-text Epilogue per task spec.
Target: crates/opentui-bridge/src/ctx_epilogue_full.rs - Epilogue {text cap 512} + set/text_of/clear.
Tests: set_then_text_of, truncates_to_512, clear_empties.
Decisions: std-only, forbid(unsafe_code), char-based truncation, Default+new.
Unknowns: lib.rs wiring left to orchestrator (out of scope).
