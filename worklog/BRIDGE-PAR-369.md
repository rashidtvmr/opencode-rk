# BRIDGE-PAR-369 scratchpad (unclaimed lane, file-only)

- Claim: SKIPPED per delegator override. Orchestrator owns `tasks/completion/claims.json`. Proceeded file-only, no ledger touch.
- Task: ONE new file `crates/opentui-bridge/src/ctx_theme_full.rs`. No edits to `lib.rs`, `Cargo.toml`. No cargo, no commit.
- Source evidence:
  - TS truth `packages/tui/src/context/theme.tsx:1-50` (imports, ThemeSource discover/subscribeRefresh).
  - Existing Rust `crates/opentui-bridge/src/context_theme.rs:1-169` (ThemeCtx mode/lock/palette, MAX_PALETTE_LEN=64, set_palette truncate + empty-to-default). New file is minimal name-only subset, no duplication of mode/lock.
- Target boundary: `CtxTheme { name: String cap 64 }` + `set(&mut self, &str)` + `name_of(&self) -> &str`. std-only, `forbid(unsafe_code)`.
- Tests: 3 unit tests in-module (default, roundtrip, truncate-64).
- Decisions: `ponytail:` dropped empty-to-default? No, kept (1 branch, mirrors set_palette). Skipped mode/lock/discover; add when wiring to ThemeCtx.
- Unknowns: lib.rs wiring left to orchestrator (out of scope by instruction).
- Verification: `rustfmt --check` only (below).
