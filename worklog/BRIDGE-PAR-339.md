# BRIDGE-PAR-339 (unclaimed, ledger overflow, orchestrator owns claims)

- Claim: skipped per task order (no claims.json touch). Status: unclaimed.
- Source: packages/tui/src/component/prompt/index.tsx:1-40 (TextareaRenderable prompt).
- Target: crates/opentui-bridge/src/component_prompt_full.rs (new, lib.rs/Cargo.toml untouched).
- Design: CompPrompt {text, cursor char-offset}; insert char-caps 4096, cursor to end of insert; move_cursor clamps; preview first 128 chars; std-only, forbid(unsafe_code).
- Tests: 4 (insert+cursor, clamp, 4KiB cap, preview len).
- Verify: rustfmt --check only (no cargo per scope).
