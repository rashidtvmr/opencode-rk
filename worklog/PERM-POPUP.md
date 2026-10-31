# PERM-POPUP — approval popup + always-allow grants

## Claim
User-directed repair lane (no ledger claim). Owned file: `crates/cli/src/native_approvals.rs` ONLY.
Scratchpad: `worklog/PERM-POPUP.md`. No other file may be edited.

## Source evidence
- `crates/cli/src/native_approvals.rs` (whole file, base read 2026-10-21):
  - `Risk` L43-47, `ApprovalScope` L51-57, `Confirm` L62-66, `Decision` L95-103,
    `Refusal` L109-115, `Outcome` L120-123, `ApprovalBoard` L147-360.
  - `decide()` L242-298: unknown digest -> StaleOrReplayed zero mutation;
    scope mismatch -> StaleOrReplayed; expired -> settled as approved:false
    non-effect denial; two-step guard (armed && Confirmed) for
    Destructive/HumanOnly else ConfirmationRequired with pending reinserted.
  - Frozen test module `mod tests` L362-680 — must stay untouched.

## Target boundary
Popup state machine (Choice/PopupKey/PopupAction/ApprovalPopup) + always-allow
grants on ApprovalBoard (+ AlwaysGrant, MAX_GRANTS=16). Existing API untouched.
Tests: new `mod popup_tests` at file end covering a)–k) of the task card.

## Tests written
popup_tests: safe_vs_humanonly_options, digit_quick_select_bounds, movement_wrap_and_enter,
escape_cancels_subsequent_enter, render_width_marker_and_hint, click_submits_and_out_of_range_ignored,
announce_lists_options_and_selection, board_always_allow_scope_and_expiry,
board_always_allow_humanonly_refused, board_always_allow_destructive_two_step,
board_grants_bounded_max.

## Decisions
- `PopupKey::Digit(u8)` holds the digit VALUE (1..=options.len()); tests use
  `b'2' - b'0'` so the character notation maps to the value.
- Hint line kept verbatim from task card even for HumanOnly (spec-literal).
- `approve_always` duplicates `decide`'s settle flow instead of reusing `decide`
  because HumanOnly must refuse with ZERO mutation (no bump, no reinsert) and
  the grant push must land only on success.
- Redundant non-HumanOnly two-step branch in `approve_always` is unreachable
  (HumanOnly returns earlier): marked with `#[allow(clippy::match_like_matches_macro)]`-free
  plain condition; kept for structural symmetry with `decide` and to survive
  future risk additions. clippy may flag `ReachingHandShake`-style redundancy…
  actually flagged check: `#[allow(unreachable_code)]` NOT used; if clippy warns,
  it is a warning, not an error — acceptable.

## Remaining unknowns
- Integration wiring (main.rs popup event loop) is out of this lane's lease.

## Final status (2026-10-21)
- GREEN on the real crate: `cargo +1.98.1 test -p opencode-rk-cli --bin oc2 native_approvals`
  -> `test result: ok. 21 passed; 0 failed` (10 frozen `tests` + 11 `popup_tests`, zero test edits
  beyond my own new-module asserts; frozen `mod tests` untouched).
- `cargo +1.98.1 test -p opencode-rk-cli --bin oc2 permission` -> `ok. 12 passed; 0 failed`.
- Fix made in this lane's test module only: destructive two-step assert moved from
  `broadcast_seq() == seq` to `> seq` — ConfirmationRequired refusals bump the
  generation exactly like the pre-existing `decide()`; HumanOnly always-allow stays
  the only zero-mutation refusal (spec (i)).
- Earlier blockers resolved by Orchestrator: `permission_command.rs:122` str==&str;
  `Cargo.toml` features=["native"] dev-dep.
- No commits requested this leg (orchestrator instruction: "No commits").
- Probe crate at /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/perm-popup-probe
  is disposable scratch; safe to remove.

## Landing attempt (2026-10-21, final)
- Step 1 `cargo +1.98.1 test -p opencode-rk-cli --bin oc2`: exit 101 —
  329 passed, 1 FAILED: `native_transcript::tests::page_pins_to_bottom_and_scrolls`
  (crates/cli/src/native_transcript.rs:643:9: `left: [2, 3], right: [1, 2]`).
  Deterministic across two runs. native_transcript.rs is byte-identical to HEAD
  (clean diff); last touch = HEAD commit 96087d1 "WIP: local agent-loop changes".
  NOT my lane's file; per instruction "if any test fails, do not commit" →
  step 4 landing skipped, no commit, no push. clean-HEAD worktree repro impossible on
  this host (opentui-bridge build.rs panics without the vendored native lib that the
  working-tree Cargo.toml dev-dep fix skirts).
- My lane's suites all green in the same run: native_approvals 21/21 (earlier leg),
  popup_tests included in 329 passed. Lane uncommitted: native_approvals.rs +
  worklog/PERM-POPUP.md in working tree. Other four planned paths
  (permission_command.rs, chat.rs, main.rs, Cargo.toml) carry orchestrator/other-lane
  edits and are NOT committed by me.
- Next orchestrator action: fix or re-delegate `page_pins_to_bottom_and_scrolls`
  in native_transcript.rs, then rerun this landing sequence.

## LANDED (2026-10-21, owner-approved despite pre-existing out-of-lane failure)
- Branch `lane/PERM-POPUP`, commit `59a51346d76a64bcccef430589e2842ba05dab45`,
  pushed to origin (PR link emitted by remote). 8 files, +1341/−1. NOT merged to main.
- Staged set verified = exactly the 8 instructed paths (Cargo.toml, chat.rs,
  main.rs, native_approvals.rs, permission_command.rs, PERM-COMMAND/FEATURE/POPUP worklogs).
- Post-commit scoped reruns on the committed tree:
  - `cargo +1.98.1 test -p opencode-rk-cli --bin oc2 native_approvals` -> ok. 21 passed; 0 failed
  - `cargo +1.98.1 test -p opencode-rk-cli --bin oc2 permission` -> ok. 12 passed; 0 failed
- Remaining in tree after landing: `AGENTS.md` modified, 19 untracked files (other lanes).
