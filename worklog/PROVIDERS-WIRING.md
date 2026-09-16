# PROVIDERS-WIRING audit (read-only; fix lane owns claude_oauth.rs)

## Wired in lib.rs (57 mods)
account_status, account_sync, auth, auth_commands, auth_profile, auth_store,
budget, catalog_sync, claude_oauth, codex_oauth, config, cost, debug_export,
fallback, handler_projection, health, int_alias, int_backoff, int_catentry,
int_client, int_events, int_ledger, int_mapping, int_retry, int_routekey,
int_state, int_status, int_token, integration, integration_connection,
integration_probe, integration_sync, local_credential_import, location_ctx,
mcp_transport, metrics, model_route, oauth_flow, provider_dispatch,
proxy_route, rate_limit, recording, refresh_gate, registry, request_profile,
responses, retry, route_compose, router, share_descriptor, streaming, tap,
usage_status, webhook.

## Unwired src files (9, no `pub mod` in lib.rs)
connection.rs, int_connect.rs, int_handler_lane.rs, int_location_lane.rs,
int_mcp_lane.rs, int_methods.rs, int_refresh.rs, int_registry_lane.rs,
int_share_sync_lane.rs.

## Why wiring NOT needed (no lib.rs edit made)
- prov_017..022 tests use crate path (`use opencode_rk_providers::claude_oauth`,
  `local_credential_import`, `request_profile`, `auth_commands`, `usage_status`,
  `auth_store`) — all already wired. prov_023/024 import no crate module
  (docs/fixture paths only).
- All 9 unwired files are consumed via `#[path = "../src/..."]` includes, never
  via crate path: grep `opencode_rk_providers::(connection|int_connect|
  int_methods|int_refresh|int_handler_lane|int_location_lane|int_mcp_lane|
  int_registry_lane|int_share_sync_lane)` across tests+src = zero hits.
- Lane files declare integrator-owned wiring: connection.rs test header
  ("included by path so this lane never edits the shared lib.rs
  (integrator-owned)"); int_mcp_lane.rs ("integrator wires
  `pub mod int_mcp_lane;` into lib.rs later").
- Wiring now would expand public API with duplicate-slice modules
  (int_connect vs integration_connection, int_handler_lane vs
  handler_projection, int_location_lane vs location_ctx, int_share_sync_lane
  vs share_descriptor, int_mcp_lane) with no consumer — YAGNI. Integrator
  wires on lane acceptance.

## cargo check
`cargo check -p opencode-rk-providers` exit 0. Only pre-existing warning:
unused import `Duration` in crates/providers/src/auth.rs:4 (not touched —
out of scope).
