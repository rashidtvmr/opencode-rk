# RED-VALIDITY-ACPSDK2 — ACP/WSX/SDK/HEAD/RUN fresh RED/GREEN receipts

Base rev: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`. Impl + frozen tests pre-exist
as untracked lane files (prior RED-VALIDITY-ACPSDK + WSX-SDK-HEAD-GATE). This lane
re-proves suite bite with fresh behavior-only temp stubs: signatures intact,
compiles, no frozen-test / lib.rs / ralph.json touch. Restored byte-identical
via `cp /tmp/opencode/xB-bak-*.rs` + `cmp IDENTICAL`. Serial, one heavy cmd at a
time, `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `timeout 120` per run.
`free -h` start: 6.2G total / ~2.6G avail. End: 6.2G total / ~2.8G avail.

Claim: all 9 suites bite on behavior-stubbed impls (RED below) and return to full
GREEN after restore. Zero product delta from this lane (all 9 `cmp IDENTICAL`).

## Impl hashes (pre == post)

- `crates/server/src/acp_bridge.rs` `2f38d37093efdf9bdfaa1a9720b2a34e40ade5bac5bcee8e6569b5a5d1f29cf9`
- `crates/server/src/acp_files.rs` `bd64b42082357c6ac2136eb152e0ff28b09e36b291ad9f8ceeebe031c62b6b58`
- `crates/server/src/workspace_proxy.rs` `66dc480459a88bb3123158770d70739243733f28eb0973b94fd3804086177964`
- `crates/server/src/remote_sync.rs` `af0731885753753af768dc622aedc6f3b822522b7dbdb2d77405ea27e6a7d3d7`
- `crates/server/src/sdk_client.rs` `3a362f7fe0d4d54a52d6a32722562141a56b846287f3787397d122f7768044e9`
- `crates/server/src/sdk_spawns.rs` `fa737acb9acfe48146664623d51a20e90da3e00969e3af200086294c21b20b07`
- `crates/cli/src/run_headless.rs` `77b355582ec81f4b821e433061ba5d1bcb4360294f9f350b5797371bfa7c8f71`
- `crates/cli/src/session_export.rs` `c3d8aaa77351ccef9e6cabe88de399903a50ee34d8b5a3e418930b5e73e59113`
- `crates/sessions/src/runner.rs` `c8ecbde176c318d53dfafe1903374ef977e4ecc0a80a0f90968e0a89faa01306`

## Frozen test hashes (untouched)

- `crates/server/tests/acp_bridge.rs` `e5eb31def91fe3be8b8ae2e5de8f0d45b140f4fa61971ec6907f076eaf22394d`
- `crates/server/tests/acp_files.rs` `8c248471caa82230ae682256fbe1703fe06adbb5d921f6ef86847873296717b4`
- `crates/server/tests/workspace_proxy.rs` `427c13d62fb05e4f6dc2817800ea7d4c195dadb4330f1205f8f5c7bb802dcc35`
- `crates/server/tests/remote_sync.rs` `ac8bf34e2975efc2073b32c4bab24823d9212bae06c281d588cab21a0c059ef4`
- `crates/server/tests/sdk_client.rs` `ccc0d2551bb95ed745736f8259a0e47c47df4f95777b5c912767fbc08faf36f2`
- `crates/server/tests/sdk_spawns.rs` `a8561295cb63469fe1c923386d2e2b6a9bf05acd229e9d0cefeb55d9a783a9f3`
- `crates/cli/tests/run_headless.rs` `85652c80693ebca9ec7fcf8ee5c8ad54c1be41a6ab5a4527e7a5e00f0026550b`
- `crates/cli/tests/session_export.rs` `e213d1bde7b749ef9a39926085393044ca0471d733f87adbcc8710085247f7bd`
- `crates/sessions/tests/runner.rs` `eb01782c59569ed34176ec74fc7c39dbe2de937eed38dd2a63394a5fcd92c1b8`

## Receipts (stub -> RED fails -> restore IDENTICAL -> GREEN)

| id | stub (behavior-only, compiles) | RED log | GREEN log | restore |
|----|-------------------------------|---------|-----------|---------|
| ACP-001 `acp_bridge` | `encode_frame_into`: drop `line.push(b'\n')` | `/tmp/opencode/xB-acp-red.log`: 3 pass / 2 fail; T01 panics `tests/acp_bridge.rs:45` (`ends_with(b"\n")`), T02 panics `:75`; T03-T05 pass | `/tmp/opencode/xB-acp-green.log`: 5 passed | cmp IDENTICAL |
| ACP-002 `acp_files` | `is_supported` also accepts `Terminal` | `/tmp/opencode/xB-acpfiles-red.log`: 3 pass / 2 fail; T03 `Terminal must be unsupported` (`tests/acp_files.rs:121`), T05 `unwrap_err on Ok` (`:243`); T01/T02/T04 pass | `/tmp/opencode/xB-acpfiles-green.log`: 5 passed | cmp IDENTICAL |
| WSX-001 `workspace_proxy` | `route`: bridge path w/o endpoint returns `Local` not `BadEndpoint` | `/tmp/opencode/xB-wsx1-red.log`: 4 pass / 1 fail; T02 panics `tests/workspace_proxy.rs:141` (`Ok(Local)` vs `Err(BadEndpoint)`); T01/T03/T04/T05 pass | `/tmp/opencode/xB-wsx1-green.log`: 5 passed | cmp IDENTICAL |
| WSX-002 `remote_sync` | `backoff_secs` returns `0` always | `/tmp/opencode/xB-wsx2-red.log`: 4 pass / 1 fail; T02 panics `tests/remote_sync.rs:99` (`[0,0,0,0,0]` vs `[2,4,8,16,30]`); T01/T03/T04/T05 pass | `/tmp/opencode/xB-wsx2-green.log`: 5 passed | cmp IDENTICAL |
| SDK-001 `sdk_client` | `SdkClient::new` stores empty base URL, skips validation | `/tmp/opencode/xB-sdk1-red.log`: 6 pass / 6 fail; neg-bad-base (`tests/sdk_client.rs:315` base `""`), determinism (`:397` `/api/x` vs full URL), T01 (`:116`), T02 (`:159`), rewrite (`:138`), redaction (`:268` `new("ftp:")` wrongly Ok); timeout/abort/input-gate/neg-status/io-matrix pass | `/tmp/opencode/xB-sdk1-green.log`: 12 passed | cmp IDENTICAL |
| SDK-002 `sdk_spawns` | `spawn` short-circuits valid requests to `Err(InvalidInput)` after `validate` | `/tmp/opencode/xB-sdk2-red.log`: 1 pass / 10 fail; only `neg_invalid_inputs` passes; T01 `:93`, T02 `:127`, T03 `:160`, T04 `:185`, T05 `:226`, kill `:366`, drop `:386`, double-wait `:339`, refusal `:328`, determinism `:411` fail | `/tmp/opencode/xB-sdk2-green.log`: 11 passed | cmp IDENTICAL |
| HEAD-001 `run_headless` | `ok()` returns code `99` not `0` | `/tmp/opencode/xB-head1-red.log`: 2 pass / 6 fail; T01 `:173`, T02 `:203`, T03 `:240`, T04 `:286`, T05, determinism `:446` fail on code; absolute-path + spill-failure pass (assert 1/2) | `/tmp/opencode/xB-head1-green.log`: 8 passed | cmp IDENTICAL |
| HEAD-002 `session_export` | `is_secret_key`+`is_secret_value` always `false` | `/tmp/opencode/xB-head2-red.log`: 7 pass / 1 fail; T04 panics `tests/session_export.rs:172` (`sk-abc-value` leaks); lifecycle/oversize/determinism pass | `/tmp/opencode/xB-head2-green.log`: 8 passed | cmp IDENTICAL |
| RUN-001 `runner` | `start` short-circuits valid sessions to `Err(InvalidInput)` after `valid` | `/tmp/opencode/xB-runner-red.log`: 0 pass / 5 fail; T01 `:8`, T02 `:27`, T03 `:45`, T04 `:69`, T05 `:84` fail at `start().unwrap()` | `/tmp/opencode/xB-runner-green.log`: 5 passed | cmp IDENTICAL |

Notes:
- Partial-fail REDs are valid suite-bite receipts: each stub hits one entry
  point; tests not touching it stay green while every touching test fails.
- Two transient sibling-lane breakages observed, neither touched by this lane:
  (a) `crates/providers/src/share_descriptor.rs` TEMP-STUB broke `sdk_spawns --no-run`
  with `unexpected closing delimiter` analogue (`E0425`-class provider lib error;
  `M` in status, sibling-owned); cleared without edit here, serial `--no-run`
  exit 0 before SDK-002 RED. (b) `crates/server/src/sync_log.rs` untracked
  sibling WIP broke `cli run_headless` build once (`unexpected closing delimiter:
  }` at `sync_log.rs:165`); cleared without edit here, serial `--no-run` exit 0
  before HEAD-001 RED. Own-file verify each step: `cmp IDENTICAL` post-restore.
- First SDK-002 stub attempt (`_req` rename) failed to compile (`E0425`); replaced
  with compiling short-circuit stub; non-compiling attempt log kept as
  `/tmp/opencode/xB-sdk2-norun.log`. First RUN-001 stub likewise non-compiling;
  replaced with compiling short-circuit; error captured in
  `/tmp/opencode/xB-runner-red.log` head (warnings section). Counted REDs are the
  compiling runs in the same files.
- No frozen test, lib.rs, Cargo.toml, schema, or ralph.json edited. Backups:
  `/tmp/opencode/xB-bak-*.rs` (9 files).
