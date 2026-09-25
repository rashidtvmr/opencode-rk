# BRIDGE-PAR-386 (unclaimed, file-only lane)

Claim: NOT claimed via claims.json per orchestrator order (proceed file-only, no ledger touch).
Source: TS truth `packages/tui/src/context/sdk.tsx:23-33` (createSDK client init); pattern `crates/opentui-bridge/src/ctx_sync_full.rs:1-57` (forbid unsafe, tiny struct, const new, #[cfg(test)]).
Target: ONE new file `crates/opentui-bridge/src/ctx_sdk_full.rs`, no lib.rs/Cargo.toml edits.
Tests: 3 in-file (down-call-fails, connect-enables, saturate); frozen here, implementation matches.
Decisions: pub fields per spec literal; saturating_add (ponytail: no reconnect/disconnect, add when TS abort/recreate mapped).
Unknowns: none. Verification: `rustfmt --check` only, no cargo/commit.
