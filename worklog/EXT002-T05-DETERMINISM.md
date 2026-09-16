# EXT002-T05 thread-count determinism — investigation

Task: EXT002-T05-DETERMINISM. Read-only unless safe impl-only fix exists.
Frozen: `crates/tools/tests/ext_builtins_lane.rs:175` (no test edits).
`ralph.json` untouched. Commit `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`.
Full raw log: `/tmp/opencode/rI-t05.log` (+ `probe_*`, `binstress_*`, `t05*_*.log`, `final_*.log`, `mechx_*.log` excerpts).

## 1. What the impl + twin do (source evidence)

- `crates/tools/src/ext_builtins_lane.rs` and `crates/tools/src/plugin_builtins.rs`
  differ only in module-doc name + `#![forbid(unsafe_code)]` (diff-exit 1, those 2 hunks).
- `grep thread|spawn|tokio|Command` on the lane hits only doc comments and
  `std::thread::park_timeout` inside `wait_ready` (lane src 191); T05 path never
  calls `wait_ready`. Zero `spawn` in lane src, lane test, twin.
- The lib wires `pub mod plugin_builtins` (`crates/tools/src/lib.rs:35`); the test
  bypasses the lib via `#[path = "../src/ext_builtins_lane.rs"]`.
- T05 (`tests/ext_builtins_lane.rs:145-185`) does not touch any impl thread API:
  `tempdir()` + empty `PluginRegistry::new()` + `format!("{reg:?}")` + `read_dir`,
  and asserts `/proc/self/task` count `t0 == t1`.

Verdict on "what spawns threads in the T05 path": **nothing in the T05 path**.
The impl spawns zero threads. Observed extra threads come from outside the lane.

## 2. Measurements

Budget respected: one validation command at a time, `CARGO_BUILD_JOBS=2`,
`RUST_TEST_THREADS=2`, `timeout 120`, `free -h` before/after (2.1 Gi avail start,
2.4 Gi end). Note: bare `cargo` fails to build (`opencode-rk-tools` lib has 9
pre-existing E0425 errors: `bytes`/`query`/`index` in unrelated files); all results
below use the `cargo test -p opencode-rk-tools --test <target>` path that was GREEN
at HEAD, then the prebuilt binary directly to isolate harness effects.

| scope | cmd | result |
|---|---|---|
| serial cargo 5x | `--test ext_builtins_lane -- --test-threads=1` | 5/5 GREEN |
| parallel cargo 5x | default threads | 5/5 GREEN (lucky window) |
| T05 alone, binary 10x | `ext_builtins_lane-18e9e48045019a97 T05 --exact` | 10/10 GREEN |
| full binary `--test-threads=1` 20x | same binary | 20/20 GREEN |
| full binary default threads 40x | no rebuild | **33 pass / 7 fail** |
| full binary `--test-threads=2` 20x | no rebuild | **11 pass / 9 fail** |
| full binary `--test-threads=4` 20x | no rebuild | 19 pass / 1 fail |
| T05+1 sibling 20x | 2-test filter | 20/20 GREEN |
| final parallel cargo 5x (post-cleanup) | JOBS=2 THREADS=2 | 5/5 GREEN (lucky window again) |

Failure-pair spectrum over 17 sampled failures
(`left:`=t0, `right:`=t1): `{3,2}x11 {3,1}x1 {4,2}x1 {4,3}x2 {5,4}x1 {1,2}x1`.
Example: `binstress_25.log`: `left: 5 / right: 4`. Both endpoints vary.

Isolated probe (temporary file, deleted after): T05-equivalent body in its own
binary measured `t0=2 t1=2 min=2 max=2` over 2000 samples — impl body itself
adds zero threads.

## 3. Root cause (proved, not inferred)

Rust's libtest runs each `#[test]` on its own worker thread, and `/proc/self/task`
counts **all** threads in the process, including sibling-test workers and transient
harness threads. So T05's `t0 == t1` compares a racy global:

- The **mechanism demo** (temporary `tests/zzz_t05_mech.rs`, deleted after; excerpts
  in `rI-t05.log`): 4x `sleep-200ms` tests + comm-dumper + T05-clone sampler.
  10/10 runs showed `MECH unique comms: {"mech_dump_comms","mech_sampler_cl",
  "mech_spin_a",...}` — worker threads are **named after the test fn** and visible
  in `/proc/self/task`. Sampler failed 4/10 with `left:6 right:7`: t0 sampled while
  one sibling worker had already exited, t1 after another was still alive.
- Same signature in the real binary: `left` varies (1..5) because t0 races sibling
  completion; `right` varies (1..4) because t1 races them again. Worse at
  `--test-threads=2` (9/20) than `=4` (1/20): no monotone load law — racy sampler,
  not a load-proportional leak.
- Not cross-binary pollution: failures reproduce invoking **one prebuilt binary**
  in a loop, one process at a time. Other test binaries in the same `cargo run`
  are separate processes and cannot appear in `/proc/self/task`.

Genuine nondeterminism? The **impl is deterministic** (zero threads, pure data).
The **measurement is genuinely nondeterministic**: global thread count sampled
twice across a window where harness-owned sibling threads start/exit. Flake rate
is timing-dependent (fast siblings usually finish before t0 → usually 2 vs 2;
any stagger → 3 vs 2, 4 vs 3, 5 vs 4, even 1 vs 2).

## 4. Why NO-FIX (impl-only fix impossible)

Every deterministic fix requires editing the frozen test or the harness contract:

- Thread-scoped counting (filter `/proc/self/task/<tid>/comm` to self, or count
  before spawning any sibling) = test edit. Frozen.
- Join-before-assert (run T05 serially: `--test-threads=1`) = invocation change,
  not an impl fix; default `cargo test` stays flaky. Proven: 20/20 serial GREEN
  vs 33/40 parallel.
- Impl change (e.g. registry exposes a thread counter) cannot fix a test that
  measures the process-global `/proc/self/task` — the sensor, not the impl, is racy.
  Any `src/` edit would be cosmetic and would not move the failure rate.

No `src/` or test file was modified by this investigation (probe/demo files
created under `crates/tools/tests/` were deleted; `git status` confirms no
leftovers). The only observed worktree diff in `crates/tools/tests/
ext_builtins_lane.rs` is a pre-existing formatting-only hunk from another lane
(8+/2-, `assert!`/`assert_eq!` rewraps, zero semantic change).

## 5. Recommendation to owner

- T05 as frozen is **flaky-by-construction under parallel libtest**. Options
  (all need test/owner authority, none taken here):
  a) scope the assertion to current-thread tasks or assert `t1 <= t0`-style
     quiescence — still racy, weakest;
  b) pin the binary with `--test-threads=1` in the lane gate for this target;
  c) rewrite T05 to assert on impl-owned state (registry Debug/alloc footprint)
     instead of the process-global thread table — correct fix, needs frozen-test
     waiver.
- Until then: run this target serial (`--test-threads=1`, 30/30 GREEN here) and
  treat parallel failures with `left/right in 1..5` at line 175 as harness noise,
  not regressions.

## Verdict

**NO-FIX. Evidence: `/tmp/opencode/rI-t05.log`.**
Impl spawns zero threads; T05 measures the libtest harness's own transient worker
threads via `/proc/self/task`; prebuilt-binary loops reproduce 3-vs-2 plus
4-vs-2/5-vs-4/1-vs-2 spectra; serial runs are 30/30 GREEN. Deterministic repair
needs frozen-test edits — out of authority.
