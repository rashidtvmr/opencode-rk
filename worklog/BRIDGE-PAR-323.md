# BRIDGE-PAR-323 dialog_alert_full

## Claim
- Attempted `cc.claim(...,'BRIDGE-PAR-323','ses_par323','worklog/BRIDGE-PAR-323.md')`.
- FAILED: `completion_claims.ClaimError: claims must be a bounded mapping`.
- Cause: `tasks/completion/claims.json` holds 501 rows > MAX_ROWS 500
  (`tools/completion_claims.py:39,57`), schemaVersion 1. All cc ops
  (claim/update) fail at `load_ledger`. No session holds BRIDGE-PAR-323
  (verified absent), so no fencing collision. File written as evidence
  for orchestrator to adopt after ledger repair. Status NOT flipped
  (ledger unwritable); honest state = blocked on ledger overflow.

## Source evidence
- `packages/tui/src/ui/dialog-alert.tsx:12` `DialogAlertProps{title,message,onConfirm?}`,
  return-key confirms + `dialog.clear()`.
- Style model: `crates/opentui-bridge/src/dialog_confirm_full.rs:1-80`
  (forbid unsafe, trunc by chars, in-file tests).

## Target boundary
- ONE new file `crates/opentui-bridge/src/dialog_alert_full.rs`, 111 lines, rustfmt PASS exit 0, 6 tests in-file.
- No lib.rs / Cargo.toml edits, no cargo, no commit (per lane scope).
- `AlertDialog{title cap 128 chars, body cap 2048 chars, seen: bool}` +
  `show(&mut self,&str,&str)` (resets seen=false) + `dismiss` (seen=true) +
  `lines(width)->Vec<String>` (word-wrap, width.max(1), cap 16 rows).
- std-only, forbid(unsafe_code). Byte-slice `&rest[take.len()..]` is char-boundary
  safe (take built from leading chars).

## Tests (6 in-file, written, unrun per no-cargo scope)
- show_caps_and_marks_unseen, dismiss_then_reshow_resets_seen,
  lines_start_with_title_and_wrap_body, lines_cap_rows_and_tolerate_zero_width,
  unicode_caps_are_char_based, default_is_unseen_and_blank.

## Verification
- `rustfmt --edition 2021 crates/.../dialog_alert_full.rs` normalize + `--check` -> FMT_OK exit 0. `cc.update` blocked attempt also failed same ClaimError.

## Remaining
- Orchestrator: repair ledger overflow, claim BRIDGE-PAR-323 for ses_par323,
  run `rustc --test` / cargo target for file, flip status.
