# BRIDGE-PAR-413 (unclaimed, file-only per spawn orders; orchestrator owns ledger)

## Claim
- Task BRIDGE-PAR-413. No claim filed (spawn orders: do NOT touch claims.json). Status: unclaimed.

## Source evidence
- TS truth `packages/tui/src/context/permission.tsx:5-23`: `PermissionMode="auto"|"normal"`, init `args.auto ? "auto":"normal"`, `set(mode)`, `toggle()`.
- Pattern `crates/opentui-bridge/src/ctx_args_full.rs:1-48` (forbid unsafe, std-only, Default, must_use).
- Untouched: `lib.rs`, `Cargo.toml`, `permission_ctx.rs`.

## Target boundary
- New file only: `crates/opentui-bridge/src/ctx_perm_full.rs`.
- API: `PermMode{auto:bool}` + `new/from_auto/set_auto/toggle/is_auto`. Std-only, forbid(unsafe_code), 50 lines (<60).

## Tests
- `default_normal`, `from_auto_set`, `toggle_flips` (>=3). Not run via cargo (spawn orders: no cargo).

## Decisions
- Bool not enum (ponytail: 2 variants only; upgrade when third mode lands).

## Unknowns
- Wiring into `lib.rs` left to orchestrator (out of scope).
- Verification: `rustfmt --check` PASS, exit 0. `wc -l` 50.
