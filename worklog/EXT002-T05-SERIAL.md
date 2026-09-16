# EXT002-T05 serial verification (v3)

Rev `248f519`. Target `crates/tools/tests/ext_builtins_lane.rs` frozen (`:175` untouched). No product/test edits. Log: `/tmp/opencode/v3-t05.log`.

Prior verdict read: `worklog/EXT002-T05-DETERMINISM.md` — T05 racy under parallel (global `/proc/self/task` vs libtest workers); serial 30/30 GREEN there.

Cmd: `CARGO_BUILD_JOBS=1 timeout 120 cargo test -p opencode-rk-tools --test ext_builtins_lane [-- --test-threads=1]`.

| # | mode | ext002_t05 outcome | suite |
|---|---|---|---|
| 1 | serial `--test-threads=1` | ok | 5 passed, 0 failed |
| 2 | serial `--test-threads=1` | ok | 5 passed, 0 failed |
| 3 | serial `--test-threads=1` | ok | 5 passed, 0 failed |
| 4 | parallel default | ok | 5 passed, 0 failed |
| 5 | parallel default | ok | 5 passed, 0 failed |

## Verdict

Serial mandate holds: **y** (serial 3/3 GREEN, parallel 2/2 GREEN this window — flake not reproduced; consistent with DETERMINISM "lucky window" note, not a refutation).
