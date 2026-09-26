# BRIDGE-PAR-313 scratchpad

Claim: attempted `cc.claim(..., BRIDGE-PAR-313, ses_par313, worklog/BRIDGE-PAR-313.md)`.
Ledger `load_ledger` raised `ClaimError: claims must be a bounded mapping`
(MAX_ROWS 500 exceeded: 501 claim rows on disk, HEAD claim still fails closed).
No foreign owner for BRIDGE-PAR-313 (absent from ledger). Proceeded file-only,
no ledger mutation. Task unclaimed through no fault of lane.

Source evidence:
- TS truth `/home/rashid/projects/opencode/packages/tui/src/component/dialog-debug.tsx:24`
  `entries` memo label/value rows joined with newline, copied via `copy`.
- Style model `crates/opentui-bridge/src/fork_dialog_full.rs:1-124`
  (`#![forbid(unsafe_code)]`, consts, struct+impl, in-file tests).

Target boundary: ONE new file `crates/opentui-bridge/src/dialog_debug_full.rs`.
No lib.rs, Cargo.toml, cargo, commit.

Tests: 6 in-file (`push_ok_truncates_row`, `push_at_cap_rejected`,
`push_truncates_char_safe`, `lines_clips_width_and_caps_max`,
`lines_empty_when_no_rows`, `clear_drops_all`).

Decisions: `push` rejects at cap (returns false) rather than evicting, matches
fork_dialog_full lane convention; `lines` takes last max rows, char-safe clip.
`MAX_ROWS=64`, `MAX_ROW=512`.

Remaining: ledger claim + completed flip blocked by oversized claims.json
(501 > 500). Orchestrator must prune ledger then claim/complete.
Verification: `rustfmt --check` only per lane scope.
