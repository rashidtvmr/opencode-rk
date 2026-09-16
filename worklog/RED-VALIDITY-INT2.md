# RED-VALIDITY-INT2

Rev: 248f519d53ed29cbf7fef7ceb2b5010fb1a4224b. Workdir /home/rashid/projects/opencode-rk.

Method: serial temp-stub-restore per owned file. RED logs `/tmp/opencode/wD-<id>-red.log`.
Restore verified byte-identical (`diff` pre-stub vs post + sha256 match). GREEN 5/5 each after restore.
NEVER touched: frozen tests (`crates/providers/tests/*`), `crates/providers/src/lib.rs`, `ralph.json`,
`refresh_gate.rs`, `claude_oauth.rs` (latter two dirty by another lane, not this lane).
Serial `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `timeout 120`, `rtk free -h` before each lane run.

## Dirty-on-arrival

y. 6/8 owned src files dirty (rustfmt-only hunks vs HEAD): int_registry_lane.rs,
int_methods.rs, int_refresh.rs, location_ctx.rs, mcp_transport.rs, share_descriptor.rs.
int_connect.rs + handler_projection.rs clean. Frozen tests dirty (reformat, other lane).
Pre-stub sha256 recorded `/tmp/opencode/wD-INT2-pre-stub-sha256.txt`; post-restore
`/tmp/opencode/wD-INT2-post-sha256.txt` — `diff` ALL_IDENTICAL. Prestub backups
`/tmp/opencode/wD-INT2/*.prestub` (8 files). Zero TEMP-STUB remaining
(`grep TEMP-STUB crates/providers/src/` empty).

## Rows

| ID | File | RED stub | Result | RED log | GREEN |
|----|------|----------|--------|---------|-------|
| INT-001 | int_registry_lane.rs | `get()`→`None` | 2 pass / 3 fail (T01,T02,T04) | /tmp/opencode/wD-INT001-red.log (2.4K) | 5/5 |
| INT-002 | int_connect.rs | oracle→`.and(None)` (all `NotFound`) | 0 pass / 5 fail (T01-T05) | /tmp/opencode/wD-INT002-red.log (3.8K) | 5/5 |
| INT-003 | int_methods.rs | unknown-method `.unwrap()` (panic) | 3 pass / 2 fail (T04,T05 panic) | /tmp/opencode/wD-INT003-red.log (3.0K) | 5/5 |
| INT-005 | int_refresh.rs | `needs_int_refresh`→`false` | 3 pass / 2 fail (T01,T05) | /tmp/opencode/wD-INT005-red.log (2.7K) | 5/5 |
| INT-006 | mcp_transport.rs | auth-handle check dropped | 4 pass / 1 fail (T04) | /tmp/opencode/wD-INT006-red.log (2.8K) | 5/5 |
| INT-007 | handler_projection.rs | `CodeRequired`→`AuthFailed` | 4 pass / 1 fail (T02) | /tmp/opencode/wD-INT007-red.log (1.9K) | 5/5 |
| INT-009 | location_ctx.rs | ignore explicit dir + `InvalidPin`→`InvalidDefault` | 2 pass / 3 fail (T01,T02,T03) | /tmp/opencode/wD-INT009-red.log (3.3K) | 5/5 |
| INT-010 | share_descriptor.rs | `apply`→`Err(Overflow)` | 0 pass / 5 fail (T01-T05) | /tmp/opencode/wD-INT010-red.log (3.3K) | 5/5 |

INT-009 note: single-stub (ignore explicit dir) bit only T01+T03 (2 pass / 2 fail);
second swap (`InvalidPin`→`InvalidDefault`) needed to flip T02 → final 2 pass / 3 fail.
Caveat: stubs applied to CURRENT (dirty) bytes, restored to CURRENT bytes —
RED witnessed against dirty tree, not pristine HEAD.

## Verdict

8/8 FLIPPABLE: each frozen suite fails with behavior-stub, passes 5/5 after
byte-identical restore. Frozen tests, lib.rs, ralph.json, refresh_gate.rs,
claude_oauth.rs untouched by this lane.
