# RED-VALIDITY-EXT2 — EXT-009/010/011/012 retroactive RED receipts

Base revision: dirty workdir on top of `248f519`-line history; impl files
predate lane (prior worklogs `worklog/EXT-009.md` etc recorded GREEN-only).
Method per task: temp behavior-stub impl (signatures intact, no test/lib.rs/
ralph.json touch), run frozen suite expecting compile+fail, restore
byte-identical (`cmp` + sha256 vs pre-run backup), re-run GREEN 5/5.
Bounds: serial, one heavy cmd at a time, `CARGO_BUILD_JOBS=2`,
`--test-threads=1`, `timeout 120` per run. `free -h` at start: 6.2G total,
~2.8G avail.

Claim: all 4 suites bite on behavior-stubbed impls (RED proven below) and
return to 5/5 GREEN after byte-identical restore. No product delta remains
from this lane (3 files show pre-existing fmt-only diffs vs git, identical
to pre-run state; plugin_discover.rs clean).

## Impl hashes (pre == post, restore verified via cmp against /tmp/opencode/bak-rC-*.rs)

- `crates/tools/src/plugin_namespace.rs` `5eaa922a867dacf82eda4b7d5599d8e34c7acd29e5d967a6bf150b1587cb9258`
- `crates/tools/src/plugin_discover.rs` `d7d7c5c0cf60590b232fc46376ddf3ee46bc212af83704cf68cca416f42825fd`
- `crates/tools/src/plugin_transform.rs` `0885c15781b78bf83c8e52b846ec88415728303e02baf7ee39ee684c14d5c3df`
- `crates/tools/src/plugin_ui_boundary.rs` `90883e97473bb51bdb8dc71966b5cb991fef77f80119d33362cd6604d60631b9`

## Frozen test hashes (untouched by this lane)

- `crates/tools/tests/plugin_namespace.rs` `1768220718be90f39986309464e9389d4db6cb5a91b50e3fbcff3b254bd75627`
- `crates/tools/tests/plugin_discover.rs` `06c872b301e3b75f7e0f4d65f85815d44301678c7dc511121b0a8dce5dd51c66`
- `crates/tools/tests/plugin_transform.rs` `55fc1d44c99381f14da2998b7add36aa6eb3d02dc5fea9e7dfda0989d9dbf86e`
- `crates/tools/tests/plugin_ui_boundary.rs` `db18b1e37c5f9fe13d58bbd6d22b83e2a404aa0f472cd286f6ee01a40e57d7c1`

## Per-task receipts (stub -> RED fails -> restore -> GREEN)

| id | stub (behavior-only, compiles) | RED log: pass/fail + witness | GREEN log | restore |
|----|-------------------------------|------------------------------|-----------|---------|
| EXT-009 | `Namespace::register` returns `Err(Overflow)` first (`plugin_namespace.rs:75`) | `/tmp/opencode/rC-EXT-009-red.log`: 0 pass / 5 fail; T01 panics `:49` (`Err(Overflow)` vs `Ok(true)`), T02 `:70`, T03 `:97`, T04 `:141` (`unwrap` on `Overflow`), T05 `:178` same | `/tmp/opencode/rC-EXT-009-green.log`: `cargo test: 5 passed (1 suite, 0.01s)` | cmp identical, sha pre==post |
| EXT-010 | `DiscoveryBoundary::declare` returns `Err(Overflow)` first (`plugin_discover.rs:120`) | `/tmp/opencode/rC-EXT-010-red.log`: 1 pass / 4 fail; T01 panics `:119` (`Err(Overflow)` vs `Ok(1)`), T02 `:154`, T03 `:176`, T05 `:341`; T04 passes (no-loading proof touches no declare-result; correct negative-path survival) | `/tmp/opencode/rC-EXT-010-green.log`: 5 passed | cmp identical |
| EXT-011 | `TransformLog::add` returns `Err(Overflow)` first (`plugin_transform.rs:92`) | `/tmp/opencode/rC-EXT-011-red.log`: 0 pass / 5 fail; T01 panics `:48` (`Err(Overflow)` vs `Ok(1)`), T02 `:67` (`unwrap`), T03 `:103`, T04 `:219`, T05 `:262` | `/tmp/opencode/rC-EXT-011-green.log`: 5 passed | cmp identical |
| EXT-012 | `UiBoundary::declare` returns `Err(Overflow)` first (`plugin_ui_boundary.rs:138`) | `/tmp/opencode/rC-EXT-012-red.log`: 0 pass / 5 fail; T01 panics `:25` (`.expect("decl 1")` on `Overflow`), T02 `:57`, T03 `:83`, T04 `:163`, T05 `:224` | `/tmp/opencode/rC-EXT-012-green.log`: 5 passed | cmp identical |

Notes:
- Partial-fail RED (EXT-010 4/5, T04 green) is a valid suite-bite receipt:
  stub exercises the happy-path entry point `declare`, so the test not
  touching its `Ok` path (registry/loader non-mutation proof) stays green.
- No frozen test, `lib.rs`, `Cargo.toml`, schema, or `ralph.json` edited.
  `ext_*_lane` twins untouched. Workdir diffs outside the 4 owned files
  are pre-existing sibling-lane state, not this lane.
- rtk wrapper quirk: `rtk cargo test` output goes through rtk tee/filter
  (`~/.local/share/rtk/tee/*_cargo_test.log`); raw `cargo test` (bare,
  no rtk) was used to capture full per-test panic text into rC logs.
  One run picked up a wrong-test-target line from rtk tee (stale
  `share_policy` entry); re-ran with output redirect to file and verified
  `running 5 tests` + `ext0*` lines in each rC log.
- Backup copies: `/tmp/opencode/bak-rC-EXT009.rs`, `bak-rC-EXT010.rs`,
  `bak-rC-EXT011.rs`, `bak-rC-EXT012.rs`.
