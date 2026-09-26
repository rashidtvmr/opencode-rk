# BRIDGE-PAR-415 (unclaimed, file-only)

- Task: `crates/opentui-bridge/src/tips_view_full.rs` (ONE new file; `lib.rs`/`Cargo.toml` untouched).
- Ledger: NOT claimed (orchestrator owns `tasks/completion/claims.json` per delegation; proceeding file-only).
- Source evidence: `packages/tui/src/feature-plugins/home/tips-view.tsx:1-40` (TipPart/Shortcuts, rotating tips view), `tips.tsx:1-30` (`tips.toggle`, `Show when={show}` + `Tips`), local pattern `crates/opentui-bridge/src/toast_single.rs:1-116` (forbid unsafe, caller-clock-free state, std-only).
- Target: `TipsView { tips: Vec<String> cap 16 each 256 chars, idx: usize }` + `push` + `next(&mut self)` + `current(&self) -> Option<&str>`, `#![forbid(unsafe_code)]`, std-only, <90 lines.
- Tests: 5 in-file (`empty_has_no_current`, `push_shows_first`, `truncates_to_256_chars`, `cap_drops_oldest`, `next_wraps`).
- Decisions: `push` truncates by chars to 256, drops oldest at cap 16 (`idx` saturating_sub); `next` wraps modulo len, no-op empty; `current` indexes `idx`.
- Verification: `rustfmt --check` only (no cargo/commit per scope).
