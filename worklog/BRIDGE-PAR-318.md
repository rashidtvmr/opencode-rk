# BRIDGE-PAR-318 scratchpad (ses_par318)

## Claim
- Task BRIDGE-PAR-318 leased by orchestrator. Claim attempted via
  `tools/completion_claims.py::claim` BEFORE any file write.
- Result: FAIL CLOSED. `ClaimError: claims must be a bounded mapping`
  (tools/completion_claims.py:58, MAX_ROWS=500, ledger holds 501 rows).
- Ledger `tasks/completion/claims.json` top-level keys: claims, schemaVersion.
  BRIDGE-PAR-318 absent (no live owner, no collision). Blocker is repo-wide
  ledger overflow, not a fence. No ledger write attempted (out of scope;
  shared file). Status unrecordable via API (update() shares load_ledger path).

## Source evidence
- TS truth: /home/rashid/projects/opencode/packages/tui/src/component/dialog-tag.tsx:8
  `DialogTag` Solid component, filter store + find.files resource sliced to 5,
  DialogSelect onSelect -> onSelect(value) + dialog.clear().
- Port boundary: dialog selection state only (tag list + cursor + select).
  No SDK, no Solid, no async; std-only.

## Target boundary
- ONE new file: crates/opentui-bridge/src/dialog_tag_full.rs. No lib.rs,
  Cargo.toml, commit, cargo per scope.

## Tests
- 6 in-file tests (accepted, reject empty/overlong, cap 32, wrap move,
  selected-none-empty, move-empty-stays-zero). Zero test edits.

## Decisions
- `forbid(unsafe_code)`, std-only, byte-length cap 64 documented.
- push trims + rejects empty (blank-tag injection), rejects overlong/over-cap.
- move_cursor uses rem_euclid (no panic/underflow); empty resets 0.
- selected returns Option, no unwrap.

## Remaining unknowns
- Ledger overflow repair is orchestrator/integration authority work.
- lib.rs wiring left to integrator (out of scope).
