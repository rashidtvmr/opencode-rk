# BRIDGE-PAR-327 scratchpad (ses_par327)

## Claim
- Attempted `cc.claim('.', 'BRIDGE-PAR-327', 'ses_par327', 'worklog/BRIDGE-PAR-327.md')` -> `ClaimError: claims must be a bounded mapping` (ledger has 501 rows > MAX_ROWS 500; pre-existing overflow, not caused by this lane). No existing row for BRIDGE-PAR-327. Proceeded to owned-file-only work; ledger NOT modified.

## Source evidence
- TS truth `packages/tui/src/component/dialog-workspace-list.tsx:16` `DialogWorkspaceList`: sorted workspace options via `DialogSelect`, move/select actions, delete/remove RPCs (host concerns).
- Pattern `crates/opentui-bridge/src/session_fork_dialog.rs:1-40` (forbid unsafe, doc divergences, const caps, small struct, in-file tests). Lines 1-99 read.

## Target boundary
- ONE new file `crates/opentui-bridge/src/dialog_workspace_full.rs` only. No lib.rs, Cargo.toml, commit.

## Tests
- 6 in-file tests: push_ok_and_selected, push_blank_rejected, push_full_rejected, push_truncates_to_cap, move_cursor_wraps_both_ways, move_cursor_empty_noop.

## Decisions
- Insertion order (host pre-sorts by name); delete/remove/sync RPCs out of scope; `move_cursor` wraps via rem_euclid; blank rejected; caps MAX_PATHS=32, MAX_PATH_LEN=512.
- Pondtail: no Default derive skipped, provided manually (trivial, keeps struct literal construction open).

## Remaining
- Ledger claim/update impossible until orchestrator prunes ledger below 500 rows; status stays honestly unclaimed. rustfmt --check result below.
