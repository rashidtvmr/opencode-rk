# RED-VALIDITY-INT3

Rev: 248f519d53ed29cbf7fef7ceb2b5010fb1a4224b. Workdir /home/rashid/projects/opencode-rk.

Method: serial temp-stub-restore per owned file. RED logs `/tmp/opencode/xE-<id>-red.log`.
Restore verified byte-identical (`diff` pre-stub vs post/final + sha256 match).
GREEN 5/5 each after restore (this epoch, serial, CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1, timeout 120, bare cargo, `free -h` before each run).
NEVER touched: frozen tests (`crates/providers/tests/*`), `crates/providers/src/lib.rs`, `ralph.json`,
`refresh_gate.rs`, `claude_oauth.rs` (latter two dirty by another lane, not this lane).

## Dirty-on-arrival

y. 6/8 owned src files dirty (rustfmt-only hunks vs HEAD): int_registry_lane.rs,
int_methods.rs, int_refresh.rs, location_ctx.rs, mcp_transport.rs, share_descriptor.rs.
int_connect.rs + handler_projection.rs clean. Frozen tests dirty (reformat, other lane).
lib.rs + ralph.json clean. Pre-stub sha256 `/tmp/opencode/xE-pre-stub-sha256.txt`;
post-restore `/tmp/opencode/xE-post-sha256.txt`; final `/tmp/opencode/xE-final-sha256.txt` —
`diff` ALL_IDENTICAL all three. Prestub backups `/tmp/opencode/xE-bak/*.prestub` (8 files).
Zero TEMP-STUB remaining (`grep -rl TEMP-STUB crates/providers/src/` empty, rc=1).
Caveat: stubs applied to CURRENT (dirty) bytes, restored to CURRENT bytes —
RED witnessed against dirty tree, not pristine HEAD.

## Rows

| ID | File | RED stub | Result | RED log | GREEN |
|----|------|----------|--------|---------|-------|
| INT-001 | int_registry_lane.rs | `get()`→`None` | 2 pass / 3 fail (t01,t02,t04) | /tmp/opencode/xE-INT001-red.log (1062B) | 5/5 |
| INT-002 | int_connect.rs | oracle→`.and(None)` (all `NotFound`) | 0 pass / 5 fail (t01-t05) | /tmp/opencode/xE-INT002-red.log (1529B) | 5/5 |
| INT-003 | int_methods.rs | unknown-method `.unwrap()` (panic) | 3 pass / 2 fail (t04,t05 panic src:204) | /tmp/opencode/xE-INT003-red.log (719B) | 5/5 |
| INT-005 | int_refresh.rs | `needs_int_refresh`→`false` | 3 pass / 2 fail (t01,t05) | /tmp/opencode/xE-INT005-red.log (679B) | 5/5 |
| INT-006 | mcp_transport.rs | auth-handle check dropped | 4 pass / 1 fail (t04) | /tmp/opencode/xE-INT006-red.log (591B) | 5/5 |
| INT-007 | handler_projection.rs | `CodeRequired`→`AuthFailed` | 4 pass / 1 fail (t02) | /tmp/opencode/xE-INT007-red.log (498B) | 5/5 |
| INT-009 | location_ctx.rs | ignore explicit dir + `InvalidPin`→`InvalidDefault` | 2 pass / 3 fail (t01,t02,t03) | /tmp/opencode/xE-INT009-red.log (974B) | 5/5 |
| INT-010 | share_descriptor.rs | `apply`→`Err(Overflow)` non-hosted | 0 pass / 5 fail (t01-t05) | /tmp/opencode/xE-INT010-red.log (1541B) | 5/5 |

INT-003 note: first-draft stub (no-op `.map` wrapper) changed nothing — reverted uncounted;
final `.unwrap()` behavior-stub compiles and flips t04+t05 via panic.
INT-010 note: first-draft stub left dead `match op` after early return (would not compile) —
reverted uncounted; final stub gates non-hosted ops to `Overflow`, hosted still `HostedPartition`
(T04 fails because upsert/remove setup hits `Overflow` first). All RED suites compile.

## Verdict

8/8 FLIPPABLE: each frozen suite fails with behavior-stub, passes 5/5 after
byte-identical restore. Frozen tests, lib.rs, ralph.json, refresh_gate.rs,
claude_oauth.rs untouched by this lane.
