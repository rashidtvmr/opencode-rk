# WSX-SDK-HEAD gate — 2026-09-16

Verify-only. No new impl (all files exist). No frozen-test edits.

## 1. File existence + hash

| file | exists | sha256 (prefix) | expected prefix | match |
|---|---|---|---|---|
| crates/server/src/workspace_proxy.rs | y | 66dc4804… | (none given) | n/a |
| crates/server/tests/workspace_proxy.rs | y | 427c13d6… | — | n/a |
| crates/server/src/sdk_spawns.rs | y | fa737acb… | fa737acb | YES |
| crates/server/tests/sdk_spawns.rs | y | a8561295… | a8561295 | YES |
| crates/server/src/sdk_client.rs | y | 3a362f7f… | — | n/a |
| crates/server/tests/sdk_client.rs | y | ccc0d255… | — | n/a |
| crates/server/src/remote_sync.rs | y | af073188… | — | n/a |
| crates/server/tests/remote_sync.rs | y | ac8bf34e… | — | n/a |
| crates/server/src/acp_bridge.rs | y | 2f38d370… | — | n/a |
| crates/server/tests/acp_bridge.rs | y | e5eb31de… | — | n/a |
| crates/server/src/acp_files.rs | y | bd64b420… | — | n/a |
| crates/server/tests/acp_files.rs | y | 8c248471… | — | n/a |
| crates/cli/src/run_headless.rs | y | 77b35558… | 77b35558 | YES |
| crates/cli/tests/run_headless.rs | y | 85652c80… | 85652c80 | YES |
| crates/cli/src/session_export.rs | y | c3d8aaa7… | c3d8aaa7 | YES |
| crates/cli/tests/session_export.rs | y | e213d1bd… | — | n/a |

All 8 impl files + all 8 test files exist. All 4 supplied hash prefixes match.

## 2. GREEN spot (serial, JOBS=2 THREADS=2, timeout 120, log /tmp/opencode/w16-spot.log)

| suite | count | result |
|---|---|---|
| --test workspace_proxy | 5 passed, 0 failed | GREEN (EXIT_WP=0) |
| --test sdk_spawns | 11 passed, 0 failed | GREEN (EXIT_SPAWNS=0) |
| cli --test run_headless | 8 passed, 0 failed | GREEN (EXIT_HEAD=0) |

Spot total: 24 passed, 0 failed, 3 suites.
Note: orchestrator asked for `rtk prefix` on cargo; `rtk cargo` wraps output (summary-only lines). Full `test result: ok…` evidence appended via bare `cargo test` lines in same log.

Extra suites (not requested, measured same session): sdk_client 12 GREEN, remote_sync 5 GREEN, session_export 8 GREEN, acp_bridge 5 GREEN, acp_files 5 GREEN. Grand total 8 suites: 59 passed, 0 failed.

No `todo!`/`unimplemented!`/stub/placeholder in any of the 8 impl files.

## 3. Status table (8 tasks)

| task | owned path | impl | tests | status |
|---|---|---|---|---|
| ACP-001 JSON-lines bridge | crates/server/src/acp_bridge.rs | y | y (tests/acp_bridge.rs, 5) | GREEN |
| ACP-002 file caps | crates/server/src/acp_files.rs | y | y (tests/acp_files.rs, 5) | GREEN |
| WSX-001 proxy bridge | crates/server/src/workspace_proxy.rs | y | y (tests/workspace_proxy.rs, 5) | GREEN |
| WSX-002 sync loop | crates/server/src/remote_sync.rs | y | y (tests/remote_sync.rs, 5) | GREEN |
| SDK-001 typed client | crates/server/src/sdk_client.rs | y | y (tests/sdk_client.rs, 12) | GREEN |
| SDK-002 spawns | crates/server/src/sdk_spawns.rs (fa737acb ✓) | y | y (tests/sdk_spawns.rs a8561295 ✓, 11) | GREEN |
| HEAD-001 headless run | crates/cli/src/run_headless.rs (77b35558 ✓) | y | y (tests/run_headless.rs 85652c80 ✓, 8) | GREEN |
| HEAD-002 session export | crates/cli/src/session_export.rs (c3d8aaa7 ✓) | y | y (tests/session_export.rs, 8) | GREEN |

Counts: 8/8 impl+tests on disk, all GREEN. 0 missing.

## 4. Missing-file action

None. No RED/GREEN cycle needed. Nothing implemented, nothing edited.

## Integrator notes

- server lib.rs wires all six server mods (acp_bridge, acp_files, remote_sync, sdk_client, sdk_spawns, workspace_proxy); earlier SDK-HEAD-STATUS.md note about missing sdk_client/remote_sync wiring now stale.
- cli run_headless/session_export live as `crates/cli/src/*.rs` alongside main.rs; no `mod` decl found via grep in main.rs — tests compile via `#[path]`; integrator to confirm wiring.
