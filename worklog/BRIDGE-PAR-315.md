# BRIDGE-PAR-315 scratchpad (session ses_par315)

## Claim
- `cc.claim(..., 'BRIDGE-PAR-315', 'ses_par315', ...)` FAILED: `ClaimError: claims must be a bounded mapping` (ledger has 501 rows > MAX_ROWS 500, pre-existing repo-wide bound; `load_ledger` rejects before fencing check). No row exists so `cc.update` impossible. Status: **blocked-on-ledger** (not a task collision; no foreign session holds it).
- Proceeded to write owned file only (no lib.rs/Cargo.toml/commit/cargo per scope) so orchestrator can integrate once ledger repaired.

## Source evidence
- `packages/tui/src/component/dialog-skill.tsx:13` `DialogSkill(props: DialogSkillProps)` skill-name pick list, `onSelect(skill.name)` + `dialog.clear()` (lines 13-49 read).
- Style mirror: `crates/opentui-bridge/src/fork_dialog_full.rs:1-124` (MAX_ID/MAX_PICKS consts, new/add/confirm/selected-id, in-file tests).

## Observed scenario
- TS dialog = async skill load + select-one-name + clear. Rust slice = owned cursor list only; load-error/empty states stay with caller.

## Target boundary
- ONE new file `crates/opentui-bridge/src/dialog_skill_full.rs`, std-only, `forbid(unsafe_code)`, <110 lines.

## Tests
- 6 in-file tests: push_ok / push_blank_rejected / push_truncates_name / push_at_cap_rejected / cursor_clamps / cursor_empty_noop. Verification: `rustfmt --check` only (no cargo per scope).

## Decisions
- `move_cursor` clamps (not wraps): DialogSelect wrap unknown from 50-line truth slice; clamp is total, OOB-safe, minimal.
- `push` truncates to MAX_NAME chars (char-boundary safe via `chars().take`), keeps cursor valid (append-only, cursor starts 0).

## Remaining unknowns
- Ledger bound blocks claim/update; orchestrator must prune ledger or bump MAX_ROWS, then lane can flip completed.
