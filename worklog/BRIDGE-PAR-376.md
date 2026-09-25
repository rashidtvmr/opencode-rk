# BRIDGE-PAR-376 (unclaimed, file-only per spawn orders)

No ledger claim: orchestrator owns tasks/completion/claims.json; told to proceed file-only.
No commit/cargo: spawn orders forbid.

## Claim
New file crates/opentui-bridge/src/ctx_editor_full.rs.

## Source evidence
- packages/tui/src/context/editor.ts:9 `const MCP_PROTOCOL_VERSION = "2025-11-25"`
- Did not edit lib.rs, Cargo.toml.

## Target boundary
- `CtxEditor { proto: String cap 32 }`, `new()` (proto 2025-11-25), `is_current(&self,&str)->bool`.
- std-only, forbid(unsafe_code), under 70 lines (61 incl tests).

## Tests
- 4 unit tests in-file: new_sets_protocol, proto_within_cap, is_current match/mismatch.
- Verification: rustfmt --check only per orders.

## Decisions
- `Default` impl delegates to `new()`; no extra API (ponytail: skipped version negotiation, add when bridge needs handshake).

## Unknowns
- lib.rs wiring left to orchestrator/integrator.
