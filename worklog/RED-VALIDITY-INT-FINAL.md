# RED-VALIDITY-INT-FINAL

Rev: 248f519. Workdir /home/rashid/projects/opencode-rk.
Zero product/test edits by this lane. No cargo runs. Read-only hash + status + log grep only.

## Dirty-on-arrival note

Tree dirty on arrival. 6/8 owned INT src files dirty (rustfmt-only hunks):
int_registry_lane.rs, int_methods.rs, int_refresh.rs, location_ctx.rs,
mcp_transport.rs, share_descriptor.rs. int_connect.rs + handler_projection.rs
clean. All 8 frozen INT tests dirty (reformat). lib.rs clean. ralph.json clean.
refresh_gate.rs + claude_oauth.rs now dirty (were clean boundary at uA retry;
another lane mid-edit). No file touched, nothing restored by this lane.

## Restore-status note (read-only)

Current sha256 of all 8 INT src files byte-identical to
/tmp/opencode/intretry/pre-stub-sha256.txt (uA retry pre-stub snapshot =
CURRENT-bytes snapshot, not HEAD). Post-restore IDENTICAL per uA retry log.
/tmp/opencode/intretry/*.bak (8 files) present. Zero TEMP-STUB remaining at
uA retry close (grep count 0, per retry section); not re-grepped here beyond
prior evidence — current diff shows only fmt hunks, no stub markers in stat.
Current HEAD-diff on owned src: 6 files (stat below); int_connect.rs and
handler_projection.rs clean vs HEAD.

## Final 8-row matrix (uA retry epoch = witnessed)

| ID | File | RED stub | RED result (fail IDs) | RED log | GREEN | sha256 (current = pre-stub) | Restore | Frozen-RED witnessed via temp-stub |
|----|------|----------|----------------------|---------|-------|------------------------------|---------|-----------------------------------|
| INT-001 | int_registry_lane.rs | get()->None | 2 pass / 3 fail (T01,T02,T04) | /tmp/opencode/uA-INT001-red.log PRESENT (2.5K) | 5/5 | bcac9019356f… (match) | IDENTICAL to pre-stub | y |
| INT-002 | int_connect.rs | oracle .and(None) | 0 pass / 5 fail (T01-T05) | /tmp/opencode/uA-INT002-red.log PRESENT (3.8K) | 5/5 | 5b19f2df62ef… (match) | IDENTICAL to pre-stub | y |
| INT-003 | int_methods.rs | unknown-method .unwrap() | 3 pass / 2 fail (T04,T05 panic) | /tmp/opencode/uA-INT003-red.log PRESENT (3.1K) | 5/5 | 582e8d08c508… (match) | IDENTICAL to pre-stub | y |
| INT-005 | int_refresh.rs | needs_int_refresh->false | 3 pass / 2 fail (T01,T05) | /tmp/opencode/uA-INT005-red.log PRESENT (2.7K) | 5/5 | 75aa9c28bce8… (match) | IDENTICAL to pre-stub | y |
| INT-006 | mcp_transport.rs | drop auth-handle check | 4 pass / 1 fail (T04) | /tmp/opencode/uA-INT006-red.log PRESENT (1.9K) | 5/5 | b9549a0cacf7… (match) | IDENTICAL to pre-stub | y |
| INT-007 | handler_projection.rs | CodeRequired->AuthFailed | 4 pass / 1 fail (T02) | /tmp/opencode/uA-INT007-red.log PRESENT (1.9K) | 5/5 | 1a39f608b02d… (match) | IDENTICAL to pre-stub | y |
| INT-009 | location_ctx.rs | ignore explicit dir + InvalidDefault on bad pin | 2 pass / 3 fail (T01,T02,T03) | /tmp/opencode/uA-INT009-red.log PRESENT (3.2K) | 5/5 | d97f2f79cd58… (match) | IDENTICAL to pre-stub | y |
| INT-010 | share_descriptor.rs | apply->Err(Overflow) | 0 pass / 5 fail (T01-T05) | /tmp/opencode/uA-INT010-red.log PRESENT (4.6K) | 5/5 | 627f783f3ccd… (match) | IDENTICAL to pre-stub | y |

GREEN logs: /tmp/opencode/uA-INT-green1.log (5.3K; registry, connect, methods,
refresh, mcp — each 5/5 verified by grep) + /tmp/opencode/uA-INT-green2.log
(2.8K; handler, location, share — each 5/5 verified by grep). 8/8 GREEN 5/5.

Full current sha256 (verified equal to pre-stub-sha256.txt):
- bcac9019356f7304bab80b8abb0bf9799673b2c43dfbee0330d4b39746544228 int_registry_lane.rs
- 5b19f2df62ef9730cfe13e65d9fc1dc5ee26d1e8de8b2d266e67e29df8376629 int_connect.rs
- 582e8d08c5086b6ed0bf7a4f844793951de5efd8a60b05e5225200e3a616a9eb int_methods.rs
- 75aa9c28bce8ddc5ff4322ddf1faa821b170e80236802a9c72dda60fb69564bb int_refresh.rs
- b9549a0cacf79ebd4172570c066dc147c714a9253b33edc1a8baaea0a82c2c5b mcp_transport.rs
- 1a39f608b02d3258b973a2af54b349772f164c9e342210e01d105b89101240ec handler_projection.rs
- d97f2f79cd5852f36f0b65cb9453292f4b82c2ed3a430af1db9dfb2e7a56f25b location_ctx.rs
- 627f783f3ccd0e1172aef78fa3edc3ec796dffcdaf365cdf255c7ffb390ed0b7 share_descriptor.rs

## Missing-logs honesty note

Prior-epoch logs (pre-uA): /tmp/opencode/int001_red.log, int002_red.log,
int003_red.log MISSING (ls: no such file). int005_red.log, int006_red.log,
int007_red.log PRESENT. INT-009/INT-010 never had log files (worklog-only per
RED-VALIDITY-INT.md). uA epoch complete: all 8 uA-INT*-red.log + both green
logs present and grepped this lane (RED fail IDs + GREEN 5/5 confirmed above).

## Flippability verdict per TDD (frozen-RED witnessed via temp-stub)

8/8 FLIPPABLE (y): each frozen suite fails with behavior-stub applied and
passes 5/5 after byte-identical restore. Caveat: uA retry stubs applied to
CURRENT (dirty) bytes, restored to CURRENT bytes — RED witnessed against
dirty tree, not pristine HEAD. Frozen tests, lib.rs, ralph.json never touched
by retry lane. INT-009 first-draft stub broke compile (stray brace, discarded,
not counted); final behavior-stub compiles and flips 2/3 fail as logged.

## git diff --stat (INT paths, this lane read-only)

6 owned src files dirty vs HEAD: int_methods.rs (3 +--), int_refresh.rs
(6 ++----), int_registry_lane.rs (4 +---), location_ctx.rs (5 ++---),
mcp_transport.rs (9 +++++----), share_descriptor.rs (4 +---).
int_connect.rs + handler_projection.rs clean. ~26 frozen test files dirty
(fmt). lib.rs clean. ralph.json clean.
