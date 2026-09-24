# TOOL-SHELL-CANCELLATION-RED

## Claim

- Task: `TOOL-SHELL-CANCELLATION-RED`
- Session: `ses_f2ba9efd6ffej1L41iMyj75iwi`
- Branch: `red/TOOL-SHELL-CANCELLATION`
- Candidate source: `1fcd3838e2a849eadfa4220c3d298750e99c52b3`
- Owned test: `crates/tools/tests/phase1_shell_cancellation.rs`
- Scratchpad: this file

## Authority and source evidence

- Accepted contract: `c630f3f`, `worklog/TOOL-SHELL-CANCEL-CONTRACT.md`.
- Seam implementation: `c15c09f`; accepted cwd repair: `1fcd383`.
- Independent seam verifier: `2699dc7`; acceptance verifier: `1c81b4c`.
- Public entrypoint: `crates/tools/src/executor.rs`,
  `ToolExecutor::execute_authorized_process` and public `Process*` types.
- Frozen broker suite SHA-256 target:
  `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`.
- Existing cwd RED SHA-256 target:
  `f4eea500` (full hash recorded after verification).

## Contract matrix

| Scenario | Expected observation | Result |
|---|---|---|
| cancellation already true | `CancelledBeforeStart`; no audit/readiness/spawn | PASS |
| cancellation after readiness | `Cancelled`; group, child, readers reaped | PASS |
| timeout | `TimedOut`; owned process reaped | PASS |
| readiness receiver dropped | typed `ReadinessReceiverClosed` after cleanup | PASS |
| output cap | bounded retained prefix, drain, one marker | PASS |
| closed false sender | process remains uncancelled and exits normally | PASS |
| terminal precedence | child exit before cancellation wins | PASS |
| owner completion | no owned child remains after result | PASS |
| broker deny and human gate | no readiness, no spawn, no marker | PASS |

All waits in the test are bounded. Fixtures use direct argv and disposable
temporary paths. No user DB, shell command string, `AllowAll`, private symbol,
or secret is used.

## RED disposition

NOT RED. NOT FROZEN. The nine-test candidate suite compiled and ran 9/9 GREEN
against accepted seam revision `1fcd3838e2a849eadfa4220c3d298750e99c52b3`.
No deterministic public-API cancellation behavior failed. A RED failure was not
fabricated.

The owner-abort delayed-side-effect assertion was removed. Accepted contract
`c630f3f` states dropped/aborted ownership is best-effort only and cannot claim
deterministic reap without awaiting completion. It is not valid RED behavior.

Conclusion: BLOCKED. The accepted seam already implements every deterministic
cancellation behavior exposed by this public API. Recommend a separate
independent cancellation acceptance-verification lane, not an implementation
lane.

Candidate suite SHA-256:
`44c228804107b65451c04fbbad9e93d14f5494605c2bcab40ad740c8aafc6827`

## Verification receipts

- Full target, sequential, jobs 2/threads 1:
  `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test phase1_shell_cancellation -- --test-threads=1`
  -> `9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s`.
- Broker hash unchanged:
  `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`.
- Cwd hash unchanged:
  `f4eea500a6edffd6d6a93b4289bf449fabfa73ed3c0e2b1db8ec707c3d65aa7`.
- `git diff --check`: PASS.
- Fixture cleanup: PASS. No owned fixture processes survived; test output only
  contained bounded `kill: <pid>: No such process` cleanup probes for already
  reaped children.
- Candidate retained as non-frozen verification evidence. No source edits.
