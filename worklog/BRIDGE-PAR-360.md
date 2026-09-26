# BRIDGE-PAR-360 (unclaimed, file-only)

Task: BRIDGE-PAR-360. No ledger claim per orchestrator override (claims.json untouched).
Scope: ONE new file `crates/opentui-bridge/src/route_sess_index_full.rs`.

## Claim
New standalone SessIndex route state; no lib.rs/Cargo.toml edits.

## Source evidence
- TS truth: `/home/rashid/projects/opencode/packages/tui/src/routes/session/index.tsx:1-60` (SolidJS session route: Prompt, theme, route/project/sync contexts, sdk types SessionStatus etc).
- Style model: `crates/opentui-bridge/src/session_index.rs` (forbid unsafe, caps, char-truncation); `session_title.rs`, `session_header.rs` (header fallbacks).

## Observed scenario
Existing `session_index.rs` covers foreground tasks/actions/kv flag, not id/title route state. `session_shared_full::header()` overlaps but tied to other struct. New file fills gap.

## Target boundary
- File: `crates/opentui-bridge/src/route_sess_index_full.rs`, 84 lines (<90), std-only, `#![forbid(unsafe_code)]`.
- API: `SessIndex { id, title }`, `new`, `set_id`, `set_title` (128-char char-safe caps), `header()` (256-char cap; title-only / id-only / `title (id)`).
- No lib.rs, Cargo.toml edits. No cargo run.

## Tests
- 5 unit tests in-file: empty, id-only, title+id, title-only, 128/256 caps.
- Verification: `rustfmt --check` only (per task; cargo not run, lib.rs not wired so crate build untouched).

## Decisions
- Char-based (not byte) truncation for unicode safety, matching sibling files.
- `header()`: title preferred; `title (id)` only when both set; empty when neither.
- `ponytail:` skipped Default-title regex detection (session_title.rs covers it); add when route needs generated-title filtering.

## Remaining unknowns
- Wiring into lib.rs/route registry left to integrator (out of scope).
