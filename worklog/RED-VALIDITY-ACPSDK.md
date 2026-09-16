# RED-VALIDITY-ACPSDK — ACP/WSX/SDK/HEAD/RUN retroactive RED receipts

Base revision: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b` (impl predates lane;
prior worklogs `ACP-001.md`, `ACP-002.md`, `WSX-001.md`, `WSX-002.md`,
`SDK-001.md`, `SDK-002.md`, `HEAD-001.md`, `HEAD-002.md`, `RUN-001.md`
recorded GREEN, most with a valid historical RED; this lane re-proves suite
bite per file with a fresh behavior-only temp stub).
Method: per file, temp behavior-stub impl (signatures intact, compiles, no
test/lib.rs/ralph.json touch), run frozen suite expecting failure, restore
byte-identical (`cmp` + sha256 vs pre-run backup in `/tmp/opencode/bak-*.rs`),
re-run GREEN. Serial, one heavy cmd at a time,
`CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`, `timeout 120` per run.
`free -h` at start: 6.2G total, ~2.8G avail; at end: ~2.5G avail.

Claim: all 9 suites bite on behavior-stubbed impls (RED proven below) and
return to full GREEN after byte-identical restore. No product delta remains
from this lane (all 9 impl files `cmp IDENTICAL` vs backup; lib.rs
`M`-state is pre-existing sibling-lane wiring, untouched here).

## Impl hashes (pre == post, restore verified via cmp against /tmp/opencode/bak-*.rs)

- `crates/server/src/acp_bridge.rs` `2f38d37093efdf9bdfaa1a9720b2a34e40ade5bac5bcee8e6569b5a5d1f29cf9`
- `crates/server/src/acp_files.rs` `bd64b42082357c6ac2136eb152e0ff28b09e36b291ad9f8ceeebe031c62b6b58`
- `crates/server/src/workspace_proxy.rs` `66dc480459a88bb3123158770d70739243733f28eb0973b94fd3804086177964`
- `crates/server/src/remote_sync.rs` `af0731885753753af768dc622aedc6f3b822522b7dbdb2d77405ea27e6a7d3d7`
- `crates/server/src/sdk_client.rs` `3a362f7fe0d4d54a52d6a32722562141a56b846287f3787397d122f7768044e9`
- `crates/server/src/sdk_spawns.rs` `fa737acb9acfe48146664623d51a20e90da3e00969e3af200086294c21b20b07`
- `crates/cli/src/run_headless.rs` `77b355582ec81f4b821e433061ba5d1bcb4360294f9f350b5797371bfa7c8f71`
- `crates/cli/src/session_export.rs` `c3d8aaa77351ccef9e6cabe88de399903a50ee34d8b5a3e418930b5e73e59113`
- `crates/sessions/src/runner.rs` `c8ecbde176c318d53dfafe1903374ef977e4ecc0a80a0f90968e0a89faa01306`

## Frozen test hashes (untouched by this lane)

- `crates/server/tests/acp_bridge.rs` `e5eb31def91fe3be8b8ae2e5de8f0d45b140f4fa61971ec6907f076eaf22394d`
- `crates/server/tests/acp_files.rs` `8c248471caa82230ae682256fbe1703fe06adbb5d921f6ef86847873296717b4`
- `crates/server/tests/workspace_proxy.rs` `427c13d62fb05e4f6dc2817800ea7d4c195dadb4330f1205f8f5c7bb802dcc35`
- `crates/server/tests/remote_sync.rs` `ac8bf34e2975efc2073b32c4bab24823d9212bae06c281d588cab21a0c059ef4`
- `crates/server/tests/sdk_client.rs` `ccc0d2551bb95ed745736f8259a0e47c47df4f95777b5c912767fbc08faf36f2`
- `crates/server/tests/sdk_spawns.rs` `a8561295cb63469fe1c923386d2e2b6a9bf05acd229e9d0cefeb55d9a783a9f3`
- `crates/cli/tests/run_headless.rs` `85652c80693ebca9ec7fcf8ee5c8ad54c1be41a6ab5a4527e7a5e00f0026550b`
- `crates/cli/tests/session_export.rs` `e213d1bde7b749ef9a39926085393044ca0471d733f87adbcc8710085247f7bd`
- `crates/sessions/tests/runner.rs` `eb01782c59569ed34176ec74fc7c39dbe2de937eed38dd2a63394a5fcd92c1b8`

## Per-file receipts (stub -> RED fails -> restore -> GREEN)

| id | stub (behavior-only, compiles) | RED log: pass/fail + witness | GREEN log: count | restore |
|----|-------------------------------|-------------------------------|------------------|---------|
| ACP-001 `acp_bridge` | `encode_frame_into`: drop `line.push(b'\n')` (framing off by one byte) | `/tmp/opencode/rH-acp-bridge-red.log`: 3 pass / 2 fail; `acp001_t01` panics `tests/acp_bridge.rs:45` (`ends_with(b"\n")`), `acp001_t02` panics `:75` same gate; T03-T05 pass (reject/map/purity paths need no encoder newline) | `/tmp/opencode/rH-acp-bridge-green.log`: 5 passed | cmp identical, sha pre==post |
| ACP-002 `acp_files` | `is_supported` widened to also accept `Terminal` | `/tmp/opencode/rH-acp-files-red.log`: 3 pass / 2 fail; `acp002_t03_unsupported_named_no_side_effect`, `acp002_t05_isolation_and_safety` fail (Terminal no longer rejected); T01/T02/T04 pass (read/write/path paths untouched) | `/tmp/opencode/rH-acp-files-green.log`: 5 passed | cmp identical |
| WSX-001 `workspace_proxy` | `route`: bridge path without endpoint returns `Local` instead of `BadEndpoint` | `/tmp/opencode/rH-wsx-proxy-red.log`: 4 pass / 1 fail; `wsx001_t02_routing_table_and_bad_endpoints` fails on the bridge-no-endpoint arm; T01/T03/T04/T05 pass | `/tmp/opencode/rH-wsx-proxy-green.log`: 5 passed | cmp identical |
| WSX-002 `remote_sync` | `backoff_secs` returns `0` always (delay table stubbed) | `/tmp/opencode/rH-wsx-sync-red.log`: 4 pass / 1 fail; `wsx002_t02_backoff_sequence_and_error_state` panics `tests/remote_sync.rs:99` (`[0,0,0,0,0]` vs `[2,4,8,16,30]`); T01/T03/T04/T05 pass (FIFO/caps/dispose/isolation need no backoff) | `/tmp/opencode/rH-wsx-sync-green.log`: 5 passed | cmp identical |
| SDK-001 `sdk_client` | `SdkClient::new` skips validation, stores empty base URL | `/tmp/opencode/rH-sdk-client-red.log`: 6 pass / 6 fail; T01/T02x2/rewrite/determinism/redaction fail (empty base mangles rewrite URL + `new("")`/`ftp:` wrongly Ok); T03/T04x2/negatives/io-matrix pass (timeout/abort/input gates need no base URL) | `/tmp/opencode/rH-sdk-client-green.log`: 12 passed | cmp identical |
| SDK-002 `sdk_spawns` | `Spawner::spawn` always `Err(InvalidInput)`, no port call | `/tmp/opencode/rH-sdk-spawns-red.log`: 1 pass / 10 fail; only `sdk002_neg_invalid_inputs_rejected_without_port_call` passes (it asserts exactly this error); all happy/cap/drop/timeout/kill/determinism arms fail | `/tmp/opencode/rH-sdk-spawns-green.log`: 11 passed | cmp identical |
| HEAD-001 `run_headless` | `ok()` returns code `99` instead of `0` | `/tmp/opencode/rH-head-run-red.log`: 2 pass / 6 fail; T01/T02/T03/T04/T05/determinism fail on exit code; absolute-path + spill-failure pass (both assert code 1/2, never reach `ok()`) | `/tmp/opencode/rH-head-run-green.log`: 8 passed | cmp identical |
| HEAD-002 `session_export` | `is_secret_key`+`is_secret_value` always `false` (redaction off) | `/tmp/opencode/rH-head-export-red.log`: 7 pass / 1 fail; `head_002_t04_secret_shaped_values_redacted_in_output_and_errors` fails (secrets pass through); all lifecycle/oversize/determinism arms pass | `/tmp/opencode/rH-head-export-green.log`: 8 passed | cmp identical |
| RUN-001 `runner` | `RunnerSet::start` always `Err(InvalidInput)` | `/tmp/opencode/rH-runner-red.log`: 0 pass / 5 fail; all of T01-T05 fail at first `start().unwrap()` (`Err(InvalidInput)`) | `/tmp/opencode/rH-runner-green.log`: 5 passed | cmp identical |

Notes:
- Partial-fail REDs are valid suite-bite receipts: each stub exercises one
  entry point, so tests not touching it (negative-path assertions, untouched
  lanes) stay green while every happy-path test touching the stub fails.
- One transient anomaly: during the lane, a parallel sibling-lane edit to
  `crates/tools/` briefly broke the workspace build (unrelated `E0425`
  `scope`/`reference`/`query`/`index` errors in `opencode-rk-tools`), and a
  later sibling edit to `crates/sessions/` briefly broke `HEAD` GREEN with
  `unexpected closing delimiter: }`. Both cleared without any edit from this
  lane (verified: `cargo test ... --no-run` exit 0, then serial rerun GREEN);
  no product file was touched except the 9 owned impls, each restored via
  `cp /tmp/opencode/bak-*.rs` + `cmp IDENTICAL`.
- Extra probe `/tmp/opencode/rH-wsx-sync-red2.log` (combined backoff+push
  stub) hit the same sibling-tools breakage and was superseded by the clean
  single-point backoff stub in `rH-wsx-sync-red.log`; kept on disk, not
  counted as the receipt.
- No frozen test, `lib.rs`, `Cargo.toml`, schema, or `ralph.json` edited.
  `crates/server/src/lib.rs` + `crates/sessions/src/lib.rs` show `M` in
  `git status` from pre-existing sibling-lane wiring, not this lane.
- Backup copies: `/tmp/opencode/bak-acp_bridge.rs`, `bak-acp_files.rs`,
  `bak-workspace_proxy.rs`, `bak-remote_sync.rs`, `bak-sdk_client.rs`,
  `bak-sdk_spawns.rs`, `bak-run_headless.rs`, `bak-session_export.rs`,
  `bak-runner.rs`.
