# TOOL-RED-SHELL-BROKER-REFREEZE

## Claim
- Task `TOOL-RED-SHELL-BROKER-REFREEZE`, session `ses_f2d7b714dffeDz0zOMaryswDaa`.
- Branch `controller/TOOL-RED-SHELL-REFREEZE`.
- Owned file: `crates/tools/tests/phase1_shell_broker.rs`.

## Source evidence
- Pre-edit candidate revision `bae37145a06d6811debb8f4d42b41d0e70660dc4`.
- Original frozen SHA-256: `2853960b7ad711d88384b136db5b37eca1dd827e82c4339990c120267a4e11ed`.
- `worklog/TOOL-RED-SHELL-SPLIT-PROPOSAL.md` authorizes Contract A split: retain t01-t03, remove t04; cancellation remains open for authorized API discovery and a separate RED.
- `worklog/TOOL-RED-SHELL-VERIFY.md` independently classified t01-t03 as broker/default-deny failures and t04 as a separate unsatisfiable cancellation contract.

## Boundary and decision
- Contract A only: shell/bash denial before spawn, no marker/secret/store side effects, permit reclamation.
- Removed only t04 timeout/cancellation test. No production code, Cargo manifest, verifier, policy, or cancellation test/API changes.
- t01-t03 assertions and support imports/helpers remain unchanged.

## Validation evidence
- Pre-edit hash verified exact.
- Pre-edit SHA-256: `2853960b7ad711d88384b136db5b37eca1dd827e82c4339990c120267a4e11ed`.
- Post-split SHA-256: `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`.
- Exact RED command: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-tools --test phase1_shell_broker -- --test-threads=2`.
- Result: compile succeeded; `running 3 tests`; `0 passed; 3 failed`; t01/t02 failed at unauthorized-success assertions; t03 failed at unauthorized-dispatch-success assertion. Captured log: `/tmp/TOOL-RED-SHELL-BROKER-REFREEZE.log`.
- `grep -c 'RED-SENTINEL' /tmp/TOOL-RED-SHELL-BROKER-REFREEZE.log`: `0`.
- `git diff --check`: clean. Prefix t01-t03 byte-identical to original (`cmp` exit `0`). Diff is t04 removal only, `0 additions, 31 deletions`.
- Independent verifier must confirm compiling three-test RED, intended failure classification, sentinel absence, semantic diff, and final hash.

## Handoff state
- Ledger moved `in-progress -> completed` only after the compiling RED and artifact checks above. This is a candidate refreeze, not independent acceptance.
- Commit and remote branch evidence must be recorded after landing.

## Open gap
- Cancellation is not silently dropped. It requires authorized-spawn API discovery and a separate RED per the split proposal.
