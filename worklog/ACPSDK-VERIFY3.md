# ACPSDK-VERIFY3 — VERIFY-ONLY (no code edits)

- Rev: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`
- Mode: VERIFY-ONLY. No source edits, no stubs, no test edits. Serial `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `timeout 110/115`, `free -h` before each run.
- Frozen files (`frozen/`, `lib.rs`, `ralph.json`): untouched. `git status` shows pre-existing workdir modifications only; this lane wrote no product code.

## Test-file hashes (frozen-input record)

```
e5eb31def91fe3be8b8ae2e5de8f0d45b140f4fa61971ec6907f076eaf22394d  crates/server/tests/acp_bridge.rs
8c248471caa82230ae682256fbe1703fe06adbb5d921f6ef86847873296717b4  crates/server/tests/acp_files.rs
427c13d62fb05e4f6dc2817800ea7d4c195dadb4330f1205f8f5c7bb802dcc35  crates/server/tests/workspace_proxy.rs
ac8bf34e2975efc2073b32c4bab24823d9212bae06c281d588cab21a0c059ef4  crates/server/tests/remote_sync.rs
ccc0d2551bb95ed745736f8259a0e47c47df4f95777b5c912767fbc08faf36f2  crates/server/tests/sdk_client.rs
a8561295cb63469fe1c923386d2e2b6a9bf05acd229e9d0cefeb55d9a783a9f3  crates/server/tests/sdk_spawns.rs
eb01782c59569ed34176ec74fc7c39dbe2de937eed38dd2a63394a5fcd92c1b8  crates/sessions/tests/runner.rs
85652c80693ebca9ec7fcf8ee5c8ad54c1be41a6ab5a4527e7a5e00f0026550b  crates/cli/tests/run_headless.rs
e213d1bde7b749ef9a39926085393044ca0471d733f87adbcc8710085247f7bd  crates/cli/tests/session_export.rs
```

## Impl-file hashes (evidence of what was verified, not modified)

```
2f38d37093efdf9bdfaa1a9720b2a34e40ade5bac5bcee8e6569b5a5d1f29cf9  crates/server/src/acp_bridge.rs
bd64b42082357c6ac2136eb152e0ff28b09e36b291ad9f8ceeebe031c62b6b58  crates/server/src/acp_files.rs
66dc480459a88bb3123158770d70739243733f28eb0973b94fd3804086177964  crates/server/src/workspace_proxy.rs
af0731885753753af768dc622aedc6f3b822522b7dbdb2d77405ea27e6a7d3d7  crates/server/src/remote_sync.rs
3a362f7fe0d4d54a52d6a32722562141a56b846287f3787397d122f7768044e9  crates/server/src/sdk_client.rs
fa737acb9acfe48146664623d51a20e90da3e00969e3af200086294c21b20b07  crates/server/src/sdk_spawns.rs
c8ecbde176c318d53dfafe1903374ef977e4ecc0a80a0f90968e0a89faa01306  crates/sessions/src/runner.rs
77b355582ec81f4b821e433061ba5d1bcb4360294f9f350b5797371bfa7c8f71  crates/cli/src/run_headless.rs
c3d8aaa77351ccef9e6cabe88de399903a50ee34d8b5e73e59113  crates/cli/src/session_export.rs
```

(Note: `crates/server/src/acp.rs` and `crates/server/src/sdk.rs` do not exist; owned impl files are the per-lane modules listed above.)

## Stub scan

`grep -rnE 'todo!\(|unimplemented!\(|stub|placeholder|mock'` over all 9 owned impl files: zero matches.

## Results (all exit 0, serial, JOBS=1 THREADS=1)

| Target | Cmd | Log | Count |
|---|---|---|---|
| acp_bridge (server, ACP-001) | `cargo test -p opencode-rk-server --test acp_bridge` | /tmp/opencode/yB-acp_bridge.log | 5/5 |
| acp_files (server, ACP-002) | `cargo test -p opencode-rk-server --test acp_files` | /tmp/opencode/yB-acp_files.log | 5/5 |
| workspace_proxy (server, WSX-001) | `cargo test -p opencode-rk-server --test workspace_proxy` | /tmp/opencode/yB-workspace_proxy.log | 5/5 |
| remote_sync (server, WSX-002) | `cargo test -p opencode-rk-server --test remote_sync` | /tmp/opencode/yB-remote_sync.log | 5/5 |
| sdk_client (server, SDK-001) | `cargo test -p opencode-rk-server --test sdk_client` | /tmp/opencode/yB-sdk_client.log | 12/12 |
| sdk_spawns (server, SDK-002) | `cargo test -p opencode-rk-server --test sdk_spawns` | /tmp/opencode/yB-sdk_spawns.log | 11/11 |
| runner (sessions, RUN-001) | `cargo test -p opencode-rk-sessions --test runner` | /tmp/opencode/yB-runner.log | 5/5 |
| run_headless (cli, HEAD-001) | `cargo test -p opencode-rk-cli --test run_headless` | /tmp/opencode/yB-run_headless.log | 8/8 |
| session_export (cli, HEAD-002) | `cargo test -p opencode-rk-cli --test session_export` | /tmp/opencode/yB-session_export.log | 8/8 |

Total: 5+5+5+5+12+11+5+8+8 = **64/64 passed, 0 failed**.

## Notes

- First `acp_bridge` invocation compiled (~64 s) then printed only warnings; reran to captured log (`Finished in 4.75s`, `5 passed`). All other targets compiled incrementally and ran to their logs first try.
- `yB-*.log` files exist at `/tmp/opencode/yB-<target>.log` (9 files). No pre-existing `yB-*.log` files were present before this run.
- Memory: ~6.2 GiB host; `free -h` checked before each run, available stayed ~2.6–3.0 GiB, swap pressure stable. Serial execution kept within budget.
- Deviations: none. No edits, no frozen-file touches.
