# RED-VALIDITY-INT4

Rev: 248f519d53ed29cbf7fef7ceb2b5010fb1a4224b. Workdir /home/rashid/projects/opencode-rk.

Method: serial temp-stub-restore per owned file. RED logs `/tmp/opencode/zG-<id>-red.log`.
Restore verified byte-identical (`diff` pre-stub vs post/final + sha256 match).
GREEN 5/5 each after restore (this epoch, serial, CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1, timeout 120, single-test invocations, `free -h` before each run).
NEVER touched: frozen tests (`crates/providers/tests/*`), `crates/providers/src/lib.rs`, `ralph.json`,
`refresh_gate.rs`, `claude_oauth.rs` (latter two dirty by another lane, not this lane).

## Dirty-on-arrival

Tree dirty on arrival. 6/8 owned src files dirty (fmt-only hunks vs HEAD): int_registry_lane.rs,
int_methods.rs, int_refresh.rs, location_ctx.rs, mcp_transport.rs, share_descriptor.rs.
int_connect.rs + handler_projection.rs clean. All 8 frozen INT tests dirty (reformat, other lane).
lib.rs + ralph.json clean. refresh_gate.rs + claude_oauth.rs dirty (another lane mid-edit, untouched here).
Pre-stub sha256 `/tmp/opencode/zG-pre-stub-sha256.txt`;
post-restore `/tmp/opencode/zG-post-sha256.txt`; final `/tmp/opencode/zG-final-sha256.txt` —
`diff` ALL_IDENTICAL all three. Prestub backups `/tmp/opencode/zG-bak/*.prestub` (8 files).
Zero TEMP-STUB remaining (`grep -rl TEMP-STUB crates/providers/src/` empty, rc=1).
Caveat: stubs applied to CURRENT (dirty) bytes, restored to CURRENT bytes —
RED witnessed against dirty tree, not pristine HEAD. Current-tree hashes recorded below + restore proof lifts dirty-tree caveat to recorded-current-tree evidence.

Current-tree hashes (= pre-stub = post = final):
- bcac9019356f7304bab80b8abb0bf9799673b2c43dfbee0330d4b39746544228 int_registry_lane.rs
- 5b19f2df62ef9730cfe13e65d9fc1dc5ee26d1e8de8b2d266e67e29df8376629 int_connect.rs
- 582e8d08c5086b6ed0bf7a4f844793951de5efd8a60b05e5225200e3a616a9eb int_methods.rs
- 75aa9c28bce8ddc5ff4322ddf1faa821b170e80236802a9c72dda60fb69564bb int_refresh.rs
- b9549a0cacf79ebd4172570c066dc147c714a9253b33edc1a8baaea0a82c2c5b mcp_transport.rs
- 1a39f608b02d3258b973a2af54b349772f164c9e342210e01d105b89101240ec handler_projection.rs
- d97f2f79cd5852f36f0b65cb9453292f4b82c2ed3a430af1db9dfb2e7a56f25b location_ctx.rs
- 627f783f3ccd0e1172aef78fa3edc3ec796dffcdaf365cdf255c7ffb390ed0b7 share_descriptor.rs

## Rows

| ID | File | RED stub | Result | RED log | GREEN |
|----|------|----------|--------|---------|-------|
| INT-001 | int_registry_lane.rs | `get()`→`None` | 2 pass / 3 fail (t01,t02,t04) | /tmp/opencode/zG-INT001-red.log (2601B) | 5/5 (/tmp/opencode/zG-g1.log) |
| INT-002 | int_connect.rs | oracle→`.and(None)` (all `NotFound`) | 0 pass / 5 fail (t01-t05) | /tmp/opencode/zG-INT002-red.log (3889B) | 5/5 (/tmp/opencode/zG-g2.log) |
| INT-003 | int_methods.rs | unknown-method `.unwrap()` (panic) | 3 pass / 2 fail (t04,t05 panic) | /tmp/opencode/zG-INT003-red.log (2965B) | 5/5 (/tmp/opencode/zG-g3.log) |
| INT-005 | int_refresh.rs | `needs_int_refresh`→`false` | 3 pass / 2 fail (t01,t05) | /tmp/opencode/zG-INT005-red.log (2724B) | 5/5 (/tmp/opencode/zG-g4.log) |
| INT-006 | mcp_transport.rs | auth-handle check dropped | 4 pass / 1 fail (t04) | /tmp/opencode/zG-INT006-red.log (1985B) | 5/5 (/tmp/opencode/zG-g5.log) |
| INT-007 | handler_projection.rs | `CodeRequired`→`AuthFailed` | 4 pass / 1 fail (t02) | /tmp/opencode/zG-INT007-red.log (1901B) | 5/5 (/tmp/opencode/zG-g6.log) |
| INT-009 | location_ctx.rs | ignore explicit dir + `InvalidPin`→`InvalidDefault` | 2 pass / 3 fail (t01,t02,t03) | /tmp/opencode/zG-INT009-red.log (3353B) | 5/5 (/tmp/opencode/zG-g7b.log) |
| INT-010 | share_descriptor.rs | `apply`→`Err(Overflow)` non-hosted | 0 pass / 5 fail (t01-t05) | /tmp/opencode/zG-INT010-red.log (3380B) | 5/5 (/tmp/opencode/zG-g8.log) |

## Notes

- Multi-target GREEN (`--test a --test b ...`) does NOT compile: pre-existing breakage in other-lane files
  (`local_credential_import.rs` E0425 `bytes`, `request_profile.rs` E0425 `profile_for_with_options`);
  log `/tmp/opencode/zG-INT-green1.log`. GREEN therefore run serially per test target (8 logs above), each RC=0.
- One transient: location_ctx GREEN first attempt RC=101 on `request_profile.rs` lib error (log `/tmp/opencode/zG-g7.log`);
  immediate serial retry RC=0 5/5 (log `/tmp/opencode/zG-g7b.log`) with no file touched by this lane — another lane mid-edit.
- Frozen tests, lib.rs, ralph.json, refresh_gate.rs, claude_oauth.rs untouched by this lane (status verified).

## Verdict

8/8 FLIPPABLE: each frozen suite fails with behavior-stub, passes 5/5 after
byte-identical restore to recorded current-tree hashes. Restore proof: pre/post/final sha256
ALL_IDENTICAL + per-file `diff -q` prestub-vs-final identical + zero TEMP-STUB.

## Confirm section (aG-INT VERIFY-ONLY epoch, 2026-09-16)

Rev on arrival: 248f519d53ed29cbf7fef7ceb2b5010fb1a4224b (verified `git rev-parse HEAD`).

Dirty-on-arrival (owned paths, `git status --porcelain`):
- src dirty: int_registry_lane.rs, int_methods.rs, int_refresh.rs, location_ctx.rs, mcp_transport.rs, share_descriptor.rs (6/8). int_connect.rs + handler_projection.rs clean.
- tests dirty: handler_projection.rs, int_connect.rs, int_methods.rs, int_refresh.rs, int_registry_lane.rs, int_mcp_lane.rs, int_handler_lane.rs, int_location_lane.rs, int_share_sync_lane.rs, location_ctx.rs, mcp_transport.rs, share_descriptor.rs (all frozen candidates touched by another lane, fmt-level).
- NEVER touched this epoch: crates/providers/src/lib.rs clean, ralph.json clean (verified empty output on scoped status), frozen tests read-only, zero TEMP-STUB (`grep -rl TEMP-STUB crates/providers/src/` rc=1).

sha256 (arrival = post-run, unchanged; VERIFY-ONLY, no edits):
- bcac9019356f7304bab80b8abb0bf9799673b2c43dfbee0330d4b39746544228 int_registry_lane.rs
- 5b19f2df62ef9730cfe13e65d9fc1dc5ee26d1e8de8b2d266e67e29df8376629 int_connect.rs
- 582e8d08c5086b6ed0bf7a4f844793951de5efd8a60b05e5225200e3a616a9eb int_methods.rs
- 75aa9c28bce8ddc5ff4322ddf1faa821b170e80236802a9c72dda60fb69564bb int_refresh.rs
- b9549a0cacf79ebd4172570c066dc147c714a9253b33edc1a8baaea0a82c2c5b mcp_transport.rs
- 1a39f608b02d3258b973a2af54b349772f164c9e342210e01d105b89101240ec handler_projection.rs
- d97f2f79cd5852f36f0b65cb9453292f4b82c2ed3a430af1db9dfb2e7a56f25b location_ctx.rs
- 627f783f3ccd0e1172aef78fa3edc3ec796dffcdaf365cdf255c7ffb390ed0b7 share_descriptor.rs
Match prior epoch hashes byte-for-byte (see Current-tree hashes above).

Runs (serial, JOBS=1 THREADS=1, timeout 120, single-test invocations, free -h before each):
- INT-001 int_registry_lane: RC=0 5/5
- INT-002 int_connect: RC=0 5/5
- INT-003 int_methods: RC=0 5/5
- INT-005 int_refresh: RC=0 5/5
- INT-006 mcp_transport: RC=0 5/5
- INT-007 handler_projection: RC=0 5/5
- INT-009 location_ctx: RC=0 5/5
- INT-010 share_descriptor: RC=0 5/5
Log: /tmp/opencode/aG-int.log (8x `test result: ok. 5 passed`).
Note: one RC=101 line in log is harness env-quoting artifact (`export J="CARGO_BUILD_JOBS=1 ..."`), not a test run; valid retry RC=0 follows.

Counts: suites 8/8 green, tests 40/40, fails 0. No stubs written (VERIFY-ONLY). No RED re-demonstrated this epoch. Prior epoch RED flippability stands on recorded hashes; this epoch confirms GREEN 5/5 each on identical bytes.

## Confirm-2 section (bG-INT VERIFY-ONLY epoch, 2026-09-16)

Rev on arrival: 248f519d53ed29cbf7fef7ceb2b5010fb1a4224b (verified `git rev-parse HEAD`).

Dirty-on-arrival y/n (owned paths, `git status --porcelain=v1`):
- src/int_registry_lane.rs: y (M). src/int_connect.rs: n (clean). src/int_methods.rs: y (M). src/int_refresh.rs: y (M). src/mcp_transport.rs: y (M). src/handler_projection.rs: n (clean). src/location_ctx.rs: y (M). src/share_descriptor.rs: y (M). 6/8 dirty, matches prior epoch set.
- tests/int_registry_lane.rs: y. tests/int_connect.rs: y. tests/int_methods.rs: y. tests/int_refresh.rs: y. tests/mcp_transport.rs: y. tests/handler_projection.rs: y. tests/location_ctx.rs: y. tests/share_descriptor.rs: y. 8/8 dirty (fmt-level, another lane).
- NEVER: crates/providers/src/lib.rs n (clean), ralph.json n (clean), crates/providers/src/refresh_gate.rs y (M, another lane, untouched here), crates/providers/src/claude_oauth.rs y (M, another lane, untouched here). Zero TEMP-STUB (`grep -rl TEMP-STUB crates/providers/src/` rc=1).

sha256 src (8/8, VERIFY-ONLY no edits, match prior epoch byte-for-byte):
- bcac9019356f7304bab80b8abb0bf9799673b2c43dfbee0330d4b39746544228 int_registry_lane.rs
- 5b19f2df62ef9730cfe13e65d9fc1dc5ee26d1e8de8b2d266e67e29df8376629 int_connect.rs
- 582e8d08c5086b6ed0bf7a4f844793951de5efd8a60b05e5225200e3a616a9eb int_methods.rs
- 75aa9c28bce8ddc5ff4322ddf1faa821b170e80236802a9c72dda60fb69564bb int_refresh.rs
- b9549a0cacf79ebd4172570c066dc147c714a9253b33edc1a8baaea0a82c2c5b mcp_transport.rs
- 1a39f608b02d3258b973a2af54b349772f164c9e342210e01d105b89101240ec handler_projection.rs
- d97f2f79cd5852f36f0b65cb9453292f4b82c2ed3a430af1db9dfb2e7a56f25b location_ctx.rs
- 627f783f3ccd0e1172aef78fa3edc3ec796dffcdaf365cdf255c7ffb390ed0b7 share_descriptor.rs

sha256 tests (8/8, VERIFY-ONLY read-only, recorded):
- 6fd83d47f61fed6db064bc14dd2a1a5b7104598a164b5d62a586913a134ce05a int_registry_lane.rs
- 2494e3c7285dd62788136477d7a1a5d81667a17d7c963cfb4a9f683866f5a68b int_connect.rs
- a08c0bea0b4c6dacd48f6586e7f03e970768e36e46a564a320cdf016c3da3b6c int_methods.rs
- 05083e052271f116627b7f9cbb9a1f400c63100627d75fd9a108e7af2248fc48 int_refresh.rs
- 2b30d993df2c55746d12da437d1ecaaf83f6a8a8b82afa5973c643b3a097af36 mcp_transport.rs
- 260d5b1bfe8b31f9ea4a6579c2d18a8e9efd8f582059d98d54bda6a522cc0425 handler_projection.rs
- 638f265fb6b3515b1aabbcfa313629880b13396661c8560ba5b184662959e1d9 location_ctx.rs
- fcd0c112d9fc4f5e0446dfc696cfee37bf7e737ed35ca9c2886717dcd2ebb7f7 share_descriptor.rs

sha256 NEVER (untouched, recorded):
- 49d44b04ccf1b053d22089b30ef777c0ae2911f03c003fd3575888964b6b33df crates/providers/src/lib.rs
- 4b99ccdc6fbe9df8397d15af7ed8682e43fd6de660cb5e2df71465e5835ae9c6 ralph.json
- 594c5ee4069d72b4b41a748dfa1aa5e6e053a8ba26724c485621b888f37d0941 crates/providers/src/refresh_gate.rs
- 7384cf9e3529fb8bdb4f081c4e0cd1533911c492c05df9d9fd479c64fc6ddd33 crates/providers/src/claude_oauth.rs

git diff --stat for INT paths (14 files, 103+/94-; src int_connect.rs + handler_projection.rs absent = clean):
- crates/providers/src/int_methods.rs | 3 +--
- crates/providers/src/int_refresh.rs | 6 ++---
- crates/providers/src/int_registry_lane.rs | 4 +--
- crates/providers/src/location_ctx.rs | 5 ++--
- crates/providers/src/mcp_transport.rs | 9 ++++---
- crates/providers/src/share_descriptor.rs | 4 +--
- crates/providers/tests/handler_projection.rs | 33 ++++++++++-------------
- crates/providers/tests/int_connect.rs | 2 +-
- crates/providers/tests/int_methods.rs | 30 ++++++++++++++-------
- crates/providers/tests/int_refresh.rs | 19 ++++++++++----
- crates/providers/tests/int_registry_lane.rs | 10 ++-----
- crates/providers/tests/location_ctx.rs | 39 +++++++++++++++++++---------
- crates/providers/tests/mcp_transport.rs | 11 +++-----
- crates/providers/tests/share_descriptor.rs | 22 +++++++---------
Full copy: /tmp/opencode/bG-int-diffstat.txt.

Runs (serial, CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1, timeout 120, single-test invocations, free -h before each):
- INT-001 int_registry_lane: RC=0 5/5
- INT-002 int_connect: RC=0 5/5
- INT-003 int_methods: RC=0 5/5
- INT-005 int_refresh: RC=0 5/5
- INT-006 mcp_transport: RC=0 5/5
- INT-007 handler_projection: RC=0 5/5
- INT-009 location_ctx: RC=0 5/5
- INT-010 share_descriptor: RC=0 5/5
Log: /tmp/opencode/bG-int.log (8x `test result: ok. 5 passed`, 8x RC=0).

Counts: suites 8/8 green, tests 40/40, fails 0. No stubs written (VERIFY-ONLY). No RED re-demonstrated this epoch. Prior epoch RED flippability stands on recorded hashes; this epoch confirms GREEN 5/5 each on identical bytes.
