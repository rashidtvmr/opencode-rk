# TOOL-RED-SHELL-BROKER-REFREEZE-VERIFY scratchpad

## Claim
- Task `TOOL-RED-SHELL-BROKER-REFREEZE-VERIFY`, session `ses_f2d71a9feffeIbSAZUmUQPMLY8`.
- Role: independent controller-refreeze RED verifier. No test/source edit, no GREEN, no merge.
- Branch `verify/TOOL-RED-SHELL-BROKER-REFREEZE`. Owned file this worklog.

## Verdict
`ACCEPT`. Commit `2c5e9cfbdc4989a54e88b0ebb095a078cc4805de` is the user-authorized
minimal broker-only refreeze: t01-t03 byte-identical, removed only t04/exclusive
support, compiling 0/3 fail-for-cause RED, hash-exact, sentinel-clean,
side-effect-clean. Cancellation remains an explicit separate open blocker.

## Candidate under verification
- Commit `2c5e9cfbdc4989a54e88b0ebb095a078cc4805de` (subject `test: refreeze shell
  broker RED without cancellation`), parent `bae37145`.
- Pre-split base `2cd9ca9` (original 4-test RED), original verifier `8c514e0`,
  split proposal `bae37145`.
- Changed files in refreeze: `M crates/tools/tests/phase1_shell_broker.rs`
  (`0 additions, 31 deletions`), `M tasks/completion/claims.json` (author row),
  `A worklog/TOOL-RED-SHELL-BROKER-REFREEZE.md`. No source/Cargo/policy/verifier.

## Hash chain
- Original frozen SHA-256 `2853960b7ad711d88384b136db5b37eca1dd827e82c4339990c120267a4e11ed`
  reproduced from `git show 2cd9ca9:...` -> matches claimed old hash.
- Candidate SHA-256 `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`
  -> matches claimed candidate hash, and unchanged after the test run.

## t01-t03 preservation proof
- `head -161` of old vs new: `cmp` exit 0 -> byte-identical prefix covering all
  three tests and shared helpers.
- Lines 160-164 (t03 tail) `cmp` exit 0 -> identical.
- Diff body adds ZERO lines (`git diff --numstat` = `0 31`); sole hunk
  `@@ -162,34 +162,3 @@` deletes exactly the t04 test and its doc comment.
- Import `std::time::Duration` and helper `result_text_of` remain used by t01-t03
  (`grep -n` confirms use at lines 57/65/87/99/121/122). No exclusive support left
  dangling.

## Command evidence
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-tools --test
  phase1_shell_broker -- --test-threads=2` (log `/tmp/TOOL-RED-SHELL-BROKER-REFREEZE-VERIFY.log`).
- Compile succeeded (`Finished test profile ... in 16.03s`; lib warnings only, no errors).
- `running 3 tests`; `test result: FAILED. 0 passed; 3 failed; 0 ignored`.
- Fail-for-cause classification (exact lines):
  - t01 `phase1_shell_broker.rs:60` `unauthorized shell must not succeed; got output len 0`
    -> shell spawned unbrokered. INTENDED.
  - t02 `phase1_shell_broker.rs:90` same denial cause. INTENDED.
  - t03 `phase1_shell_broker.rs:135` `unauthorized shell dispatch must not report
    success` -> default `RegistryDispatcher::new()` AllowAll. INTENDED.
- `git diff --check`: clean.

## Leak and side-effect evidence
- `grep -c RED-SENTINEL /tmp/...log` = 0. Sentinel never printed on failure.
- No `phase1_shell_broker` / `sleep 1; touch` / marker process alive after run.
- No marker artifacts (`no-spawn`, `no-side-effect`, `dispatch-denied`, `cancelled`)
  under `$TMPDIR`.

## Cancellation boundary
- `t04_timeout_leaves_no_late_marker` absent from candidate. Split proposal
  `bae37145` retains it as Contract B future task `TOOL-RED-SHELL-CANCEL-RED`
  (`crates/tools/tests/phase1_shell_cancellation.rs`).
- Cancellation is NOT silently dropped: refreeze worklog "Open gap" states it
  requires authorized-spawn API discovery and a separate RED.
- No production cancellation code or test added; unresolved, open.

## Resources
- One Cargo process; system memory 62% free pre-run; no unbounded waits.
- Remote: `origin/controller/TOOL-RED-SHELL-REFREEZE` = `2c5e9cf` confirmed.

## Unresolved gaps
- Cancellation contract remains open by design (separate lane, not this refreeze).