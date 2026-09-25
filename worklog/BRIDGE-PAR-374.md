# BRIDGE-PAR-374 (unclaimed, file-only; orchestrator owns ledger)

Claim: none (per spawn instructions, do not touch claims.json).
Source: packages/tui/src/context/clipboard.tsx:4-8 (ClipboardContent {data,mime}).
Target: crates/opentui-bridge/src/ctx_clipboard_full.rs, CtxClip, under 90 lines, std-only, forbid(unsafe_code).
Tests: roundtrip, overwrite, truncates_caps, trunc_keeps_utf8_boundary, clear_empties.
Decisions: truncate (not fail-closed); read returns (&str,&str) mime-first per TS field order data/mime... actually (mime,data) for set(mime,data) symmetry.
Unknowns: wiring into lib.rs left to orchestrator.
