# HEAD-001 worklog

## Claim
Own `crates/cli/src/run_headless.rs` only. Headless run with Inline/Block
renderers, attachment conversion, bounded preview with caller-dir spill,
typed exits. No edits to lib.rs / Cargo.toml / schemas. Test file additive:
`crates/cli/tests/run_headless.rs` (`#[path]` include, no Cargo.toml change).

## Source evidence
- tasks/HEAD-001.md: contract, 5 frozen tests, ownership lock.
- PLAN.md:36 pin `95daf90670b7c039c436c85537da5fbfe2205b41` vs observed
  `07619a09d6da7945ea3fdbb4f8ae5ce8dc2b6eeb`; never equated.
- worklog/SDK-HEAD-STATUS.md: run_headless.rs MISSING, cli/src held only main.rs.
- Local state at start: cli/src held main.rs + session_export.rs (HEAD-002 lane
  file present on disk, untracked); no run_headless module; tests dir held
  session_export.rs + others, no run_headless test.

## Observed scenario
TDD RED (stub, 0/8) -> freeze hashes -> GREEN (native impl, 8/8) -> rerun +
sibling regression (session_export 8/8) + `cargo check -p opencode-rk-cli --tests`.

## Target boundary
`PREVIEW_MAX_LINES=2000`, `PREVIEW_MAX_BYTES=51200`, `RunPrompt`, `RunOpts`,
`RunSink`, `RunExit{code,message}`, `Attachment::{Text,File}`, `Renderer`,
`ModelPort`, `FilePort`, `ModelError::{Refused,Failed}`, `run`. Single sink
write per call (staged buffer, atomic). One spill file per oversize section
named `{session}-{answer|attachN}.spill` under caller `spill_dir`.

## Tests
HEAD-001-T01..T05 + absolute-path + spill-failure + determinism cases. Command:
`CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-cli --test run_headless -- --test-threads=2`

## RED freeze
- RED run: 0 passed / 8 failed (stub returns code 99).
  Log: /tmp/opencode/head001-red.log
- Frozen test hash (sha256 `crates/cli/tests/run_headless.rs`): `85652c80693ebca9ec7fcf8ee5c8ad54c1be41a6ab5a4527e7a5e00f0026550b`
- Stub hash at RED: `73eacf9913abd0d58caf5c10dc0c20cefdabcd13758c226a4db9bef1a117c540`
- Head commit: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`
- Test file frozen after this point: no edits to tests during GREEN.

## GREEN
- Impl hash (sha256 `crates/cli/src/run_headless.rs`): `77b355582ec81f4b821e433061ba5d1bcb4360294f9f350b5797371bfa7c8f71`
- GREEN run: 8 passed / 0 failed.
  Log: /tmp/opencode/head001-green.log
- Sibling regression `session_export`: 8 passed / 0 failed.
  Log: /tmp/opencode/head001-export.log
- `cargo check -p opencode-rk-cli --tests`: exit 0.
  Log: /tmp/opencode/head001-check.log
- One benign warning on owned test file: `FixtureModel::refusing` never used
  (refusal path implemented, mapped to exit 1, but not covered by frozen suite;
  see gaps). No warnings on owned impl file. Other warnings pre-existing in
  security/sessions/tools crates, unrelated to this lane.
- `cargo check --workspace` skipped: 8GB host budget (6.2Gi total, ~2.6Gi
  avail), scope-narrowed per AGENTS.md. Integrator reruns workspace gate.
- Stub grep: only `forbid(unsafe_code)` hits for "stub"; no `todo!`/
  `unimplemented!`/`unsafe` in owned files.

## Decisions
- `ModelError::Refused` -> exit 1 (usage/model), `Failed` -> exit 2.
- Attachments validated + converted before any model call or sink byte, so
  escape/oversize/missing render zero bytes and (escape) make zero model calls.
- Spill marker format: `\n...truncated (N more bytes spilled to {name})\n`
  appended to the head; head snapped to char boundary under both caps.
- Spill failure (unwritable dir) -> exit 2 with prior sink bytes untouched.
- Exit messages carry variant text only (`ok`, `invalid session`, ...), never
  prompt/answer content. Tests use fixture strings only.
- Prompt streamed as raw `prompt.text.as_bytes()`; no compression framing
  anywhere in the module.

## Remaining unknowns / gaps
- Integrator wires `mod run_headless` into cli lib/bin target.
- `ModelError::Refused` branch has no frozen test (only `Failed` covered);
  suggest verifier add refusal case if desired.
- T04 asserts sink within `PREVIEW_MAX_BYTES + 1024` (marker slack), not
  strictly under cap; spill file holds full bytes so nothing is lost.

## Confirm (2026-09-16, QA rerun)
- Exists: `crates/cli/src/run_headless.rs` (261 lines, 8537 B) yes; `crates/cli/tests/run_headless.rs` (455 lines) yes. Note: task card path `tests/run_headless.rs` wrong; actual `crates/cli/tests/run_headless.rs`.
- Hashes match claim: impl `77b355582ec81f4b821e433061ba5d1bcb4360294f9f350b5797371bfa7c8f71`, tests `85652c80693ebca9ec7fcf8ee5c8ad54c1be41a6ab5a4527e7a5e00f0026550b`.
- Rerun: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-cli --test run_headless -- --test-threads=2` → 8 passed / 0 failed. Log: /tmp/opencode/head001_confirm.log. No impl/test edits.
## Verify (2026-09-16, CLI gate QA): exists y (4/4); run_headless 8/8 + session_export 8/8 = 16/16 (log /tmp/opencode/w6-clinew.log); full -p opencode-rk-cli --tests 23 passed / 0 failed (log /tmp/opencode/w6-clifull.log); hashes impl 77b35558 y / tests 85652c80 y; no edits.
