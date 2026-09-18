# TUI-004 worklog (composer types: buffer, keymap, paste gate, queue)

## Claim
`crates/cli/src/native_composer.rs`: char-safe `EditBuffer`, `SubmitKeymap`
(Enter/Ctrl-J), bracketed-paste gate (`unwrap_bracketed` + `apply_paste`,
never submits), busy `VecDeque` queue cap 32, reject consts 32 KiB
(`MAX_PASTE_BYTES`, `MAX_DRAFT_BYTES`). `forbid(unsafe_code)`, std only.

## Source evidence (HEAD 5af7884)
- `tasks/completion/tui.json:7` (TUI-004): owned path = this file only;
  tests: Tamil/combining/emoji/wide safe, paste never auto-executes,
  Enter/Ctrl-J + undo + multiline, busy queue bounded, large paste rejected.
- `crates/sessions/src/tui_state.rs:9-20,25-149`: sibling contract
  (`MAX_QUEUED=8`, `MAX_DRAFT_BYTES=32_768`, Enter/CtrlJ keymap, submit/queue/
  interrupt/finish_turn). This lane is the CLI-native composer: same 32 KiB
  draft budget, queue widened to 32 per task lease, plus real char-safe
  cursor editing the sibling lacks.
- `crates/cli/src/terminal_host.rs:36-40`: `MAX_PASTE_BYTES=32_768` policy
  mirrored here; admitted pastes fit the draft budget.
- `crates/cli/src/tui_entry.rs:87-102`: keymap resolves flag > env > Enter
  default; this file provides the submit/newline decision consumed by such
  entry code (`decide_key`, `handle_key`).
- Current code: file did not exist before this lane.

## Observed scenario
New file. GREEN 14/14 via mandated command. Stub RED (bounds disabled +
byte-step cursor): 8 pass, 6 fail. Paste-submit stub: paste test fails.

## Target boundary
Owned file only: `crates/cli/src/native_composer.rs`. No `mod` wiring,
no edits elsewhere. Integrator adds `mod native_composer;` and reruns.

## Tests (frozen in-file `#[cfg(test)]`, 14)
1. `emoji_edit_never_splits_code_point` — crab insert/move/delete byte-safe.
2. `zwj_emoji_deletes_scalar_by_scalar_without_corruption` — ZWJ seq drains clean.
3. `wide_cjk_and_tamil_edit_safe` — CJK 3-byte steps, Tamil full drain safe.
4. `combining_mark_never_orphans_bytes` — e+acute composes, backspace safe.
5. `bracketed_paste_never_submits_or_queues` — hostile paste inert, no submit.
6. `unframed_input_is_not_a_paste` — framing required, None otherwise.
7. `oversize_paste_rejected_without_mutation` — >32KiB rejected, state kept.
8. `paste_capped_by_total_draft_budget` — paste+buffer capped at 32KiB.
9. `keymap_enter_and_ctrlj_mirror` — Enter/CtrlJ submit/newline mirrored.
10. `busy_queue_bounded_fifo_at_32` — 32 admit, 33rd QueueFull, FIFO drain.
11. `interrupt_preserves_draft_and_queue` — draft+queue survive, resubmit sends.
12. `multiline_up_down_keeps_char_column` — up/down column, top no-op.
13. `undo_restores_last_edit` — snapshot restore, boundary held.
14. `empty_and_oversize_drafts_rejected` — blank/oversize denied, draft kept.
RED proof: stub A (cap checks `> usize::MAX`, `cursor -= 1`) fails 6/14
(emoji, zwj, wide, combining, oversize-paste + 1 more); stub B
(paste calls `handle_key(Enter)`) fails paste test. Frozen hash (GREEN):
sha256 `95395aa405e5c0e954930bc5a059cc9f21cae415f0426f4c3eb7ce59e823e7c4`.

## Decisions
- Cursor moves per `char` (scalar), not grapheme cluster: still byte-safe
  for ZWJ sequences; noted `ponytail:` upgrade path to unicode-segmentation.
- Paste has no submit path by construction (returns `PasteOutcome::Inserted`
  only); keymap submit flows only through `handle_key`.
- Undo depth 32; queue `VecDeque` FIFO; all denials leave state untouched.

## Remaining unknowns / gaps
- INTEGRATION: no `mod native_composer;` in `main.rs` (out of lease).
- Duplicate `Composer` concept in `tui_state.rs`/`chat_composer.rs` is
  intentional per lease; dedup is integrator decision.
