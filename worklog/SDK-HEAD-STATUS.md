# SDK/WSX/ACP/HEAD status — 2026-09-15

## Verify run
`cargo test -p opencode-rk-server --test sdk_client --test remote_sync --test acp_bridge`
=> 22 passed, 0 failed (3 suites).
Extra: `--test acp_files` => 5 passed. Total 4 suites: 27 passed.

| suite | tests | result |
|---|---|---|
| sdk_client | 12 (T01..T05 + negatives) | GREEN |
| remote_sync | 5 (WSX-002-T01..T05) | GREEN |
| acp_bridge | 5 | GREEN |
| acp_files | 5 (bonus, ACP-002) | GREEN |

No `todo!`/`unimplemented!`/stub in any of the 4 impl files.

## 8-task status table

| task | owned path | impl | tests | status |
|---|---|---|---|---|
| SDK-001 typed client | crates/server/src/sdk_client.rs (269 L) | yes | yes (tests/sdk_client.rs, 397 L) | DONE on disk, GREEN |
| WSX-002 remote sync | crates/server/src/remote_sync.rs (204 L) | yes | yes (tests/remote_sync.rs, 211 L) | DONE on disk, GREEN |
| ACP-001 acp bridge | crates/server/src/acp_bridge.rs (286 L) | yes | yes (tests/acp_bridge.rs, 218 L) | DONE on disk, GREEN |
| ACP-002 acp files | crates/server/src/acp_files.rs (140 L) | yes | yes (tests/acp_files.rs, 251 L) | DONE on disk, GREEN (extra) |
| SDK-002 spawns | crates/server/src/sdk_spawns.rs | MISSING | MISSING | not started |
| WSX-001 ws proxy | crates/server/src/workspace_proxy.rs | MISSING | MISSING | not started |
| HEAD-001 headless | crates/cli/src/run_headless.rs (cli/src holds only main.rs) | MISSING | MISSING | not started |
| HEAD-002 export | crates/cli/src/session_export.rs | MISSING | MISSING | not started |

Counts: 4/8 impl+tests on disk (all GREEN); 4/8 missing.

## Integrator notes
- `crates/server/src/lib.rs` wires `acp_bridge` + `acp_files` but NOT `sdk_client` / `remote_sync`. Those tests compile via `#[path = "../src/..."]`; integrator must add `pub mod sdk_client;` + `pub mod remote_sync;`.
- Missing lanes each own exactly one file per task card; no shared-file pre-wire needed except lib.rs adds above.
