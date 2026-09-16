# RED-VALIDITY-WEB-PROV — WEB-013..017 + PROV-015/016 retroactive RED receipts

Base revision: `248f519 feat: add native harness feature batches` (impl predates lane;
prior worklog `worklog/WEB-013-PROV-016.md` recorded GREEN-only, no valid RED).
Method: per task, temp behavior-stub impl (signatures intact, no test/lib.rs/ralph.json
touch), run frozen suite expecting compile+fail, restore byte-identical (`cmp`/`diff -q`
+ sha256 vs pre-run hash), re-run GREEN 5/5.
Bounds: serial, one heavy cmd at a time, `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`,
`timeout 115` per run, `rtk` prefix. `free -h` at start: 6.2G total, ~2.5G avail.

Claim: all 7 suites bite on behavior-stubbed impls (RED proven below) and return to
5/5 GREEN after byte-identical restore. No product delta remains from this lane.

## Impl hashes (pre == post, restore verified via cmp against /tmp/opencode/bak-<id>.rs)

- `crates/sessions/src/web_013.rs` `0598e7f7d6c1be5f694e702988257364274da167066c26e1a696b31ac64ffe38`
- `crates/sessions/src/chat_nav_lane.rs` `96f79cef53d40e3276c52562ab3328da7b3e0ae28f5e46d0a047829c68a3317f`
- `crates/sessions/src/project_context.rs` `4670d75ce05ede11df2207c601b43f0bcb9405723af6236ade996c4f99aba319`
- `crates/server/src/voice_capture.rs` `8d2c9688601930c541640cea929c10c60f56614b8cba5bc6603001d8d471c806`
- `crates/server/src/web_artifact.rs` `7f9f1382f83b4b7826a617d224ae30ede51153bbadfa92a1e922275c1bfad386`
- `crates/providers/src/auth_profile.rs` `109e666c81f2e4cb4679c0a912d99d3635b0b51736ea72512a7ce726aa0def32`
- `crates/providers/src/codex_oauth.rs` `e16ef54882ebbe3e4a5578f6ddaa5a05877fbd96420b829997763a3290806af8`

## Frozen test hashes (untouched by this lane)

- `crates/sessions/tests/web_013.rs` `2f66c89f6b4b4ba9186c5387b1253a04c70b7749cb68697dbbb85e4aa06d1bdb`
- `crates/sessions/tests/chat_nav_lane.rs` `83e16f9bb20b6a7f45de19c5bf7b81b3c7726811f41761063c3159bd060249ad`
- `crates/sessions/tests/project_context.rs` `363a622174219430ba10fa7d0e8fe309c4c72e159c8e2a743f1659cdadb8e7f6`
- `crates/server/tests/voice_capture.rs` `773db9117c074996c21f103087e3fd6abf2275e67cb522f3a632f15646efb749`
- `crates/server/tests/web_artifact.rs` `19aa65d4d17bb29dcaf2356e91fbe6a7c3c1735b5b9a36a658a375396d2dbb71`
- `crates/providers/tests/auth_profile.rs` `a23f9f322c959260d88fd0cbf033ce605b064ae8add93b843440a0bbd586756e`
- `crates/providers/tests/codex_oauth.rs` `aa7459f644f45eae1815fe9c5dfc67fcee45f7ff9df5db8d2aa4369514098851`

## Per-task receipts (stub -> RED fails -> restore -> GREEN)

| id | stub (behavior-only, compiles) | RED log: fail count + witness | GREEN log: count | restore |
|----|-------------------------------|-------------------------------|------------------|---------|
| WEB-013 | `ResearchRun::new` forces `adapter_available:false` (`crates/sessions/src/web_013.rs:147`) | `/tmp/opencode/s3-web-013-red.log`: 0 pass / 5 fail; `web013_t01` panics `web_013.rs:14` (plan build `Unavailable`), T02-T05 same gate | `/tmp/opencode/s3-web-013-green.log`: 5 passed | cmp identical, sha pre==post |
| WEB-014 | `upsert_chat` returns `Err(NotFound)` first (`crates/sessions/src/chat_nav_lane.rs:126`) | `/tmp/opencode/s3-web-014-red.log`: 3 pass / 2 fail; `web014_t01` (`chat_nav_lane.rs:45` `NotFound`), `web014_t04` (`:211` `NotFound`); T02/T03/T05 pass (encode/focus paths need no upsert) — suite bites, not full-fail | `/tmp/opencode/s3-web-014-green.log`: 5 passed | cmp identical |
| WEB-015 | `create_project` returns `ProjectId(u64::MAX)` without insert (`crates/sessions/src/project_context.rs:171`) | `/tmp/opencode/s3-web-015-red.log`: 0 pass / 5 fail; `web015_t01` panics `:22` (`add_session` `Unavailable` on ghost id), T02-T05 same | `/tmp/opencode/s3-web-015-green.log`: 5 passed | cmp identical |
| WEB-016 | `VoiceSession::new` forces `adapter:false` (`crates/server/src/voice_capture.rs:135`) | `/tmp/opencode/s3-web-016-red.log`: 1 pass / 4 fail; T01/T03/T04/T05 panic `NoAdapter` at `start()`; T02 passes (it asserts explicit failure incl. `NoAdapter`) — correct: negative-path test stays green under stub | `/tmp/opencode/s3-web-016-green.log`: 5 passed | cmp identical |
| WEB-017 | `open_artifact` returns `Err(DocumentEmpty)` first (`crates/server/src/web_artifact.rs:115`) | `/tmp/opencode/s3-web-017-red.log`: 1 pass / 4 fail; T01/T03/T04/T05 panic `open writing: DocumentEmpty` at `web_artifact.rs:16`; T02 passes (run/apply gate + source scan, no open) — correct negative-path survival | `/tmp/opencode/s3-web-017-green.log`: 5 passed | cmp identical |
| PROV-015 | `profile_of` returns `Err(EmptyProvider)` first (`crates/providers/src/auth_profile.rs:189`) | `/tmp/opencode/s3-prov-015-red.log`: 0 pass / 5 fail; T01/T02/T03/T05 panic `EmptyProvider`, T04 mismatched variant (`left EmptyProvider / right UnknownMethod`) | `/tmp/opencode/s3-prov-015-green.log`: 5 passed | cmp identical |
| PROV-016 | `begin_login` returns `LoggedOut` (`crates/providers/src/codex_oauth.rs:302`) | `/tmp/opencode/s3-prov-016-red.log`: 0 pass / 5 fail; T01 panics at `codex_oauth.rs:23` (not PendingConsent), T02 `:41`, T03 `:70`, T04 `:86`, T05 `:115` | `/tmp/opencode/s3-prov-016-green.log`: 5 passed | cmp identical |

Notes:
- GREEN "5 passed (1 suite, 0.00s)" lines are rtk-filtered summaries; RED logs carry
  full per-test panic output and `test result: FAILED. N passed; M failed` lines.
- Partial-fail REDs (WEB-014 2/5, WEB-016 4/5, WEB-017 4/5) are valid suite-bite
  receipts: stub exercises the happy-path entry point, so tests not touching it
  (encode/focus gates, explicit-failure assertions, run/apply gates) stay green.
- No frozen test, `lib.rs`, `Cargo.toml`, schema, or `ralph.json` edited. Working-tree
  modifications outside the 7 files are pre-existing sibling-lane state, not this lane.
- Backup copies: `/tmp/opencode/bak-<id>.rs` (7 files); runner: `/tmp/opencode/red_run.py`.
