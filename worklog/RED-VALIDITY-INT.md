# RED-VALIDITY-INT

## Verdict: BLOCKED — tree dirty, RED not run

Owned INT impl files + frozen tests both dirty in working tree. Another lane
mid-edit (rustfmt-only hunks per `git diff`: `int_registry_lane.rs`,
`int_methods.rs`, `int_refresh.rs`, `location_ctx.rs`, `mcp_transport.rs`,
`share_descriptor.rs` src; all 8 frozen tests reformatted; `lib.rs` clean).
Temp-stub-restore now would clobber in-flight edits and log RED/GREEN against
mixed content. No file touched. No test run.

Boundary kept: `refresh_gate.rs`, `claude_oauth.rs` untouched.

Memory at check: ~2.5 GiB avail / 6.2 GiB total. Serial `JOBS=2 THREADS=2`,
`timeout 120` stands for retry.

## Prior evidence (worklogs, not re-witnessed)

| ID | File | RED stub | Result | Log |
|----|------|----------|--------|-----|
| INT-001 | int_registry_lane.rs | `get()`→`None` | 2 pass / 3 fail | /tmp/opencode/int001_red.log |
| INT-002 | int_connect.rs | oracle→`None` | 0 pass / 5 fail | /tmp/opencode/int002_red.log |
| INT-003 | int_methods.rs | unknown-method `.unwrap()` | 3 pass / 2 fail | /tmp/opencode/int003_red.log |
| INT-005 | int_refresh.rs | decision→`false` | 3 pass / 2 fail | /tmp/opencode/int005_red.log |
| INT-006 | mcp_transport.rs | drop auth-handle check | 4 pass / 1 fail | /tmp/opencode/int006_red.log |
| INT-007 | handler_projection.rs | `CodeRequired`→`AuthFailed` | 4 pass / 1 fail | /tmp/opencode/int007_red.log |
| INT-009 | location_ctx.rs | `resolve_*`→typed errors | 0 pass / 5 fail | (worklog INT-009) |
| INT-010 | share_descriptor.rs | `apply`→`Err(Overflow)` | 0 pass / 5 fail | (worklog INT-010) |

All GREEN 5/5 after byte-identical restore per lane worklogs.

## Retry

Once owned paths clean: serial temp-stub-restore per file, RED logs
`/tmp/opencode/rD-<id>-red.log`, restore byte-identical (`diff -q` + sha256),
GREEN 5/5 each. Frozen tests, `lib.rs`, `ralph.json` never touched.

## Retry 2026-09-16 (uA lane, dirty-on-arrival=y)

Dirty on arrival: y. 6/8 owned src files dirty with rustfmt-only hunks
(int_registry_lane, int_methods, int_refresh, location_ctx, mcp_transport,
share_descriptor); int_connect.rs + handler_projection.rs clean. No revert:
temp-stub applied to CURRENT bytes per file, restored to CURRENT bytes.
Pre-stub sha256 saved /tmp/opencode/intretry/pre-stub-sha256.txt;
post-restore sha256 identical (diff -q IDENTICAL all 8). Zero TEMP-STUB
remaining (grep count 0 across crates/providers/src/). Frozen tests, lib.rs,
ralph.json never touched.

Method: serial, CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2, timeout 120, bare
cargo (rtk passthrough). Mem at start ~2.3GiB avail / 6.2GiB.

| ID | File | RED stub | Result | Log |
|----|------|----------|--------|-----|
| INT-001 | int_registry_lane.rs | get()->None | 2 pass / 3 fail (T01,T02,T04) | /tmp/opencode/uA-INT001-red.log |
| INT-002 | int_connect.rs | oracle .and(None) | 0 pass / 5 fail | /tmp/opencode/uA-INT002-red.log |
| INT-003 | int_methods.rs | unknown-method .unwrap() | 3 pass / 2 fail (T04,T05 panic) | /tmp/opencode/uA-INT003-red.log |
| INT-005 | int_refresh.rs | needs_int_refresh->false | 3 pass / 2 fail (T01,T05) | /tmp/opencode/uA-INT005-red.log |
| INT-006 | mcp_transport.rs | drop auth-handle check | 4 pass / 1 fail (T04) | /tmp/opencode/uA-INT006-red.log |
| INT-007 | handler_projection.rs | CodeRequired->AuthFailed | 4 pass / 1 fail (T02) | /tmp/opencode/uA-INT007-red.log |
| INT-009 | location_ctx.rs | ignore explicit dir + InvalidDefault on bad pin | 2 pass / 3 fail (T01,T02,T03) | /tmp/opencode/uA-INT009-red.log |
| INT-010 | share_descriptor.rs | apply->Err(Overflow) | 0 pass / 5 fail | /tmp/opencode/uA-INT010-red.log |

INT-009 notes: single-error-swap stub bit only T03 (other tests unaffected —
expected, suite isolates typed-error paths); strong stub (both resolvers always
err) first draft broke compile (stray brace, discarded, not counted); final
behavior-stub compiles and fails 2 pass / 3 fail.

GREEN after byte-identical restore: 5/5 each target.
/tmp/opencode/uA-INT-green1.log (registry, connect, methods, refresh, mcp),
/tmp/opencode/uA-INT-green2.log (handler, location, share).
