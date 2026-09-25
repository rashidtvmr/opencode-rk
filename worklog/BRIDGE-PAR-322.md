# BRIDGE-PAR-322 scratchpad

Claim: BLOCKED - ledger `tasks/completion/claims.json` holds 501 rows > `MAX_ROWS=500`, so `load_ledger` raises `ClaimError: claims must be a bounded mapping` for any claim/update. No session holds this task; fence not the issue, schema bound is. Proceeded with owned file only per lane scope; orchestrator must reclaim/repair ledger.

Source evidence:
- TS truth `packages/tui/src/ui/dialog-help.tsx:6` `DialogHelp`: esc/enter bindings close help, ok button dismisses, static text only (40 lines total).
- Sibling pattern `crates/opentui-bridge/src/fork_dialog_full.rs:26` struct + add/confirm, in-file tests, `forbid(unsafe_code)`.

Target boundary: ONE new file `crates/opentui-bridge/src/dialog_help_full.rs`. No lib.rs, no Cargo.toml, no cargo, no commit.

Tests: 5 in-file (add_ok, blank-key reject, cell truncate 64, cap-32 reject, lines clip width/34). Verification: `rustfmt --check` only per scope.

Decisions:
- `add` rejects blank key, allows blank desc; truncates both cells to 64 chars.
- `lines(width)` formats `{key}  {desc}`, clips to `min(width, 34)`; width 0 yields empty strings.
- `ponytail:` no width-aware padding/alignment; add when dialog needs column layout.

Unknowns: none. Ledger claim pending orchestrator repair.
