# SESS-019 worklog

## Claim

Implement distinct session history APIs where `revert` is a free cursor/pointer move over retained history and `rollback` is the explicit truncating operation. Fork remains a separate storage/session operation.

## Source evidence

- Candidate base revision: `991ae5bb550cb102552bb7eec70b9789f1f77d22` plus current task work.
- `tasks/SESS-019.md`: REQ-008, five test obligations.
- `docs/upcoming-features/codex-harness.md:16,45-49`: pinned Codex research records a typed fork boundary and the three-way split: fork copies a branch, revert rewinds a pointer, rollback truncates.
- `docs/upcoming-features/SYNTHESIS.md:21,38`: explicitly requires separate API names and states that revert is free while only fork copies.
- Current code: format-2 fork behavior already lives separately in `crates/storage/src/fork_v2.rs`; `crates/sessions/src/state.rs` is a stub.

## Observed scenario

There is no compiled session-history cursor contract today. Reusing `next_message_seq` as a rewind pointer would conflict with retained immutable message sequence numbers, so SESS-019 is kept as an explicit history-state abstraction instead of mutating the storage schema.

## Target boundary

- Product implementation: `crates/sessions/src/state.rs` only.
- Shared pre-wire: `crates/sessions/src/lib.rs` exports the existing `state` module before test fan-out.
- Independent RED tests: `crates/sessions/tests/history_rewind.rs` only.
- Status/evidence: this worklog and `tasks/SESS-019.md`; `ralph.json` already records SESS-019 as `in-progress`.

## Tests

- Independent RED file: `crates/sessions/tests/history_rewind.rs`.
- Frozen SHA-256: `fcbbc01ac23e4e1b66761640f9e1b465003b51853fb414107ec11f5e125377ac`.
- The worker's first authoring run was blocked at compile time by the separately prewired but absent SESS-020 module; this was authoring feedback only.
- After signature-only scaffolds existed, `cargo test -p opencode-rk-sessions --test history_rewind` compiled and failed behaviorally: 0 passed, 5 failed. This is the frozen RED baseline.
- GREEN: `cargo test -p opencode-rk-sessions --test history_rewind` -> 5 passed, 0 failed.
- Regression: `cargo test -p opencode-rk-sessions --lib` -> 30 passed, 0 failed.
- Frozen hash was rechecked unchanged before GREEN.

## Decisions

- Revert changes only the active head/cursor; retained entries and their identities stay intact.
- Rollback truncates entries after the requested boundary and updates the head.
- Appending after a revert must not silently overwrite retained history; the contract will make branch/rollback intent explicit.
- State is caller-owned and bounded by the supplied history vector; no detached task or queue is introduced.
- Appends are capped at 100,000 retained entries, matching the existing session message scale and preventing unbounded retained history in this abstraction.

## Remaining unknowns

- Formal acceptance remains verifier/controller-owned; this worklog records implementation and test evidence only.
