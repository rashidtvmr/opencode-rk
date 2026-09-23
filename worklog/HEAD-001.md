# HEAD-001 scratchpad — headless run + renderers + spill

## Claim
- Ledger `claim(HEAD-001, ses_f384fee05ffee9ZbohjUUo6uif, worklog/HEAD-001.md)` OK, no collision.
- Owned file: `crates/cli/src/run_headless.rs` only. Frozen tests untouched.

## Source evidence (exact)
- `tasks/HEAD-001.md:1-91` — full contract (run/renderers/attachments/spill/exit codes).
- `crates/cli/tests/run_headless.rs:1-455` — frozen 8 tests (T01-T05 + absolute-path, spill-failure, determinism).
- `crates/cli/src/run_headless.rs:1-261` (pre-existing rev `1be93d3`) — already implements full contract: PREVIEW_MAX_LINES=2000 (line 9), PREVIEW_MAX_BYTES=51200 (line 11), Renderer Inline/Block, Attachment Text/File, RunPrompt/RunOpts/RunExit/ModelError/ModelPort/FilePort/RunSink, `run()` validating session/prompt/attachments pre-model-call, atomic staged render, preview_or_spill with caller-dir spill + `…truncated (N more bytes spilled)` marker.
- `crates/cli/src/main.rs:20-54` — mod list does NOT include `run_headless`; seam for integrator: add `mod run_headless;` (worker forbidden from editing main.rs).
- `crates/cli/Cargo.toml:26,39,42` — opentui-bridge optional; dev-deps force `native` feature => `cargo test -p opencode-rk-cli --test run_headless` fails at link gate on aarch64-apple-darwin (only `native/lib/x86_64-unknown-linux-gnu` vendored; build.rs panics). Pre-existing repo condition, out of lane authority.
- PLAN.md:36 pin `95daf90...`; TDD `docs/TDD.md:2-5`, SEC `docs/SECURITY.md:1-6` read.

## Observed scenario
- `cargo test -p opencode-rk-cli --test run_headless` BLOCKED by opentui-bridge native link gate (dev-dep forces `native`, no aarch64 artifact). Workaround: `rustc --edition=2021 --test crates/cli/tests/run_headless.rs -o /tmp/rh_testN` — compiles (1 dead_code warning for unused `refusing` fixture) and runs 8/8 GREEN.
- My delta: +20/-2 lines — `valid_name` also rejects drive-letter (`X:`), `spill_name()` sanitizes session (`/` etc → `_`) so spill filename can't traverse. Verified: session `../../../evil-session-xyz` + 60KiB answer → exit 0, 1 spill file `.._.._.._evil-session-xyz-answer.spill` inside spill dir, zero escape.

## Target boundary (owned)
- `run(session,prompt,out,opts,model,files)->RunExit`; Inline `hello\n`, Block ```` ```\nhello\n```\n ````; attachments validated pre-model-call (`model.calls==0` on reject); oversize head+marker bounded, full bytes to exactly one caller-dir spill file; exits 0/1/2 variant-text-only; sync, no threads/statics/network/TTY/persistence beyond caller spill dir.
- Lifetime/cleanup: caller owns session/sink/spill_dir/ports; `run` spawns nothing, holds no FD after return; failed runs emit zero sink bytes; spill failure → code 2, zero partial bytes.

## Tests
- Frozen: `crates/cli/tests/run_headless.rs` sha256 `85652c80…f0026550b` (unchanged, zero edits).
- Impl: `crates/cli/src/run_headless.rs` sha256 `04d43496…27945e02` after delta.
- GREEN cmds: `rustc --edition=2021 --test crates/cli/tests/run_headless.rs -o /tmp/rh_test2 && /tmp/rh_test2 --test-threads=1` → 8 passed, 0 failed.
- Mandated cmd `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 rtk cargo test -p opencode-rk-cli --test run_headless -- --test-threads=1` NOT runnable: no `timeout`/`rtk`-wrapped cargo path here and cargo blocked by opentui native gate (evidence above). Direct-rustc run is the equivalent harness execution of the identical frozen test file.
- RED gap (honest): implementation already GREEN at claim time (landed in `1be93d3`); no compiling-RED observed by this lane; frozen tests were authored/landed by prior wave. No RED fabricated.

## Decisions
- Minimal hardening delta only (drive-letter + spill sanitize); no contract change, no test change.
- No `todo!/unimplemented!/stub/mock` in owned file (grep clean).

## Remaining unknowns / gaps
- RED receipt missing for this lane (historical, prior wave).
- `mod run_headless;` wiring in main.rs left to integrator (out of authority).
- cargo-path GREEN blocked by pre-existing opentui-bridge native dev-dep gate on darwin (needs lane with Cargo.toml authority).
- lane_gate.py covers storage lanes only; no HEAD lane entry — no task-scoped gate to run.
