# P1: session compaction interior parity

## Claim

Implement the pinned upstream session-compaction interior as a pure,
caller-owned, bounded decision module in `crates/sessions/src/compaction.rs`:
usable-context / overflow classification, recent-turn preservation budget,
tool-output truncation, and the backward prune plan.

## Source evidence

Pinned upstream `95daf90670b7c039c436c85537da5fbfe2205b41`
(`sources/upstream.lock.json`), read-only checkout at
`/private/.../T/opencode/opencode-ref-95daf`:

- `packages/opencode/src/session/compaction.ts:28-33` constants
  `PRUNE_MINIMUM=20_000`, `PRUNE_PROTECT=40_000`,
  `TOOL_OUTPUT_MAX_CHARS=2_000`, `PRUNE_PROTECTED_TOOLS=["skill"]`,
  `MIN_PRESERVE_RECENT_TOKENS=2_000`, `MAX_PRESERVE_RECENT_TOKENS=15_000`.
- `compaction.ts:115-120` `preserveRecentBudget`: configured value or
  `clamp(floor(usable*0.25), MIN, MAX)`.
- `compaction.ts:51-52` `truncate` appends `"\n[truncated]"` past the char cap.
- `compaction.ts:273-317` `prune`: skip until the 2nd-last user turn, stop at an
  assistant summary or an already-compacted part, ignore protected tools,
  retain `PRUNE_PROTECT` tokens, commit only when `pruned > PRUNE_MINIMUM`.
- `packages/opencode/src/session/overflow.ts:8-34` `COMPACTION_BUFFER=20_000`,
  `usable`, and `isOverflow` (auto disabled or zero context never overflow).

Local gap: `crates/sessions/src/auto_compact.rs` only ports the Claude-Code
threshold/breaker policy; no `packages/opencode` compaction interior existed
(`git log --all -S 'PRUNE_PROTECT'`, `-S 'tail_start_id'` returned nothing).

## Observable contract

`opencode_rk_sessions::compaction`: `usable_context`, `is_overflow`,
`preserve_recent_budget`, `truncate_tool_output`, `plan_prune`, plus the
upstream constants and `PrunePlan`/`PruneTarget`/`MessageRef`/`ToolPartRef`.
Pure functions; caller owns the transcript; `plan_prune` never mutates input.

## Target boundary

- Product: `crates/sessions/src/compaction.rs` (new).
- Pre-wire: `crates/sessions/src/lib.rs` (`pub mod compaction;`).
- Tests: `crates/sessions/tests/compaction_interior.rs` (new).
- Worklog: this file.
- No `crates/providers/**` change. Protected paths
  `crates/providers/src/responses.rs` and
  `crates/providers/tests/responses_stream_semantics.rs` untouched (blob SHAs
  `4ab7dca008fa838cb9fe73ac88caa6708c0f1262`,
  `d6e4db7a5eb750912d4058e0f446145b70ce8089`).

## Tests

- RED compile first: `cargo test -p opencode-rk-sessions --test
  compaction_interior --no-run` -> `error[E0432]: unresolved import
  opencode_rk_sessions::compaction`.
- GREEN: same test binary -> `6 passed; 0 failed`.
- Regression: `cargo test -p opencode-rk-sessions` -> all suites pass, 0 failed.
- Clippy: `cargo clippy -p opencode-rk-sessions --all-targets` reports no
  warning mentioning `compaction`.
- Env: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`, `--offline --locked`, shared
  ABS `CARGO_TARGET_DIR`.

## Decisions

- Character-based truncation, not byte slicing, to avoid splitting UTF-8.
- `plan_prune` returns a plan (targets + totals) instead of mutating, so the
  mutation and any persistence stay with the owning caller.
- Hard `MAX_PRUNE_MESSAGES=100_000` bound; oversized transcripts return
  `PruneError::TooManyMessages` rather than scanning unbounded input.
- Overflow/usable take explicit window/reserved inputs so no config or global
  state is needed and tests stay deterministic.

## Remaining

- Live invocation (server/engine applying `plan_prune` and marking parts
  compacted, publishing the compacted event, auto/overflow compaction flow)
  is the server-owned seam and is out of this package's paths.