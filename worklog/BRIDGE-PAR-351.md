# BRIDGE-PAR-351 (unclaimed, file-only)

Task: crates/opentui-bridge/src/app_root_full.rs (NEW, no lib.rs/Cargo.toml edits).
Status: unclaimed (ledger overflow; orchestrator owns claims.json).

Evidence:
- packages/tui/src/app.tsx:1-60 RouteProvider + startup gate
- packages/tui/src/context/route.tsx:37-39 navigate(route)
- crates/opentui-bridge/src/route_ctx.rs:1-56 RouteCtx prior art (128 cap, stack)

Boundary: AppRoot {route cap 64, ready} + navigate/set_ready/route_of/is_ready.
Decisions: default route "home"; navigate fail-closed empty/>64; Default impl delegates new.
Tests: 4 (default, navigate ok, reject empty/long, ready toggle).
Verify: rustfmt --check (pending).
