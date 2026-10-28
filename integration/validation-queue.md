# Validation Queue - Recovery Mode Phase 1

## Part 1: Usage Tests (commit 77629e5)
- 21/21 PASS (8 responses_request_retry + 13 responses_usage), exit 0
- Source hashes verified immutable before/after
- Log: integration/baseline_check.log

## Part 2: Context Patch + Repair Validation (commit 44f9d409)
- 14/14 PASS, exit 0
- Hashes verified: native_context.rs (91ce191...), native_context_tests.rs (6516374...)
- Log: integration/context_repaired_check.log

## Part 3: Held-Summary v2 4-File Candidate Diagnostic (base 77629e5)
- Result: EXIT 101 (COMPILE FAILURE in test file)
- Server LIB compiled successfully (prepare_background fix confirmed)
- Test file failed: error[E0599] missing `use futures_util::StreamExt;` in background_compaction_runtime.rs line 98
- All hashes match manifest; no source edits
- Log: integration/held_summary_4file_check.log

## Part 4: OAuth V4 Provider Tests (base 44f9d409)
- Result: EXIT 101 (COMPILE FAILURE)
- Error: error[E0433] cannot find `openai_device_auth` in `crate`
- Root cause: patch references module but does not declare it
- Log: integration/oauth_v4_providers.log

## Part 5: OAuth V5 Provider Tests (base 44f9d409)
- Compile: SUCCESS (fixes E0433 by removing unused save_openai_credential helper)
- oauth_request_runtime: 5 passed, 1 FAILED (InvalidJson assertion at line 199)
- zero_expiry_metadata: 5/5 PASSED
- responses_usage: 13/13 PASSED
- responses_request_retry: 8/8 PASSED
- Total: 31 passed, 1 failed
- Hashes verified immutable before/after
- Logs: integration/oauth_v5_*.log

## Part 6: Held-Summary v3 4-File Candidate Diagnostic (base 77629e5)
- Snapshot: git archive 77629e5 + 4 v3 patches applied
- All 5 hashes verified match manifest:
  - auto_compaction.rs: 6571edc759fb2d7747f7f2302450f71f97c951997afa56164d073aff4fa2171c (MATCH)
  - fallback_compaction.rs: bfb67893f78ae7e4cd3324d3aab1d76b7c31e80ff19ec883eeb0e2159ea61c67 (MATCH)
  - lib.rs: 41ee4ee181bf2030d5e3f0e656ee74f846a41010c3f82cf11045f40495d1e723 (MATCH)
  - background_compaction_runtime.rs: 3f2f8a218c23cd4f32b4e8205f2f3d77f26053fee29e8791276905e980598998 (MATCH)
  - Cargo.lock: 0dcd8464bf873ddb288dd7f30fbcb59785df7a8df18c8889cf19b1fc922f6d9f (MATCH)
- Hashes immutable before/after (verified)

**Command:** CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo +1.98.1 test -p opencode-rk-server --test background_compaction_runtime --locked --offline

**Result: EXIT CODE 101 (COMPILE FAILURE - server lib)**

**Exact error:**
```
error: expected `;`, found `#`
 --> crates/server/src/auto_compaction.rs:559:11
  |
559 |     .await
  |           ^ help: add `;` here
560 |
561 |     #[test]
  |     - unexpected token
```

**Root cause:** v3 `auto_compaction.rs.patch` introduces a syntax error - missing semicolon after `.await` at line 558-559, followed by a blank line before `#[test]` at line 561. The `.await` call needs a `;` to terminate the statement. This is a compile error in the server LIB source (not the test file).

**Test status:** NOT reached - compile failure prevents test execution
- Expected test: `successful_turn_holds_background_result_without_durable_checkpoint` (1 test)

**Log:** integration/held_summary_v3_check.log

**Note:** The v3 candidate added `use futures_util::StreamExt;` to fix the v2 test compile error, but introduced a new syntax error in the lib source. The writer must fix the missing semicolon in auto_compaction.rs.

## Part 7: OAuth V6 Provider Tests (base 44f9d409)

### Patch
- oauth-scoped-candidate-v6.patch (SHA ad70566002787d7cecbe1596a6d19aba1e57666805dcbbeb6187bedb05134a59) - **verified match**
- v6 change from v5: fixture only adds `object` = response.compaction and `created_at` required fields (no production change)

### Hash verification (all 6 match expected before AND after - immutable)
- persisted_auth.rs: 8574740f74a33e1e6f4172cade32cf6cdef2439fd90e71cd971c14c9e257eb83 (MATCH)
- responses.rs: a3206369bc334421ddfa48b6b161d6147427bc5e53878f99a9ba4d0fb9d41b2c (MATCH)
- oauth_request_runtime.rs: 9ef22f8964337094d069cf0d1b58eade006f1931f912312fda0a96da1b44f5ac (MATCH)
- zero_expiry_metadata.rs: 7453fb0733637cb1d7994657c9463554dd3b1b3a3436650ad88a3312ffdc3a4d (MATCH)
- server lib.rs: 8cbe8e583d9a9914b7fe44fd3d79c0d3e31c0e5226d2cce0ef618c5f52439d1b (MATCH)
- Cargo.lock: 0dcd8464bf873ddb288dd7f30fbcb59785df7a8df18c8889cf19b1fc922f6d9f (MATCH)

### Command 1: Provider tests
```
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo +1.98.1 test -p opencode-rk-providers --test oauth_request_runtime --test zero_expiry_metadata --test responses_usage --test responses_request_retry --locked --offline
```

**Result: EXIT_CODE=0**

| Test file | Result |
|-----------|--------|
| oauth_request_runtime | 6 passed, 0 failed |
| responses_request_retry | 8 passed, 0 failed |
| responses_usage | 13 passed, 0 failed |
| zero_expiry_metadata | 5 passed, 0 failed |
| **Total** | **32 passed, 0 failed** |

### Command 2: Server lib provider_failure_tests (gated on providers GREEN)
```
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo +1.98.1 test -p opencode-rk-server --lib provider_failure_tests --locked --offline
```

**Result: EXIT_CODE=0**
- 1 passed, 0 failed, 247 filtered out

### Logs
- integration/oauth_v6_providers.log
- integration/oauth_v6_server_lib.log

---

## Part 8: Held-Summary v5 4-File Candidate Diagnostic (base 77629e5)

### Snapshot
- Location: /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/held-summary-v5-validate
- Method: git archive 77629e5 | tar -x, then git apply 4 v5 patches
- Patches: auto_compaction.rs.patch, fallback_compaction.rs.patch, lib.rs.patch, background_compaction_runtime.rs.patch

### Hash verification (all 5 + Cargo.lock match manifest before AND after - immutable)
- auto_compaction.rs: b89497b5a695a2ecfe62196b89961b4fe8455b78d9520ec9300b5374c3932da7 (MATCH)
- fallback_compaction.rs: bfb67893f78ae7e4cd3324d3aab1d76b7c31e80ff19ec883eeb0e2159ea61c67 (MATCH)
- lib.rs: 41ee4ee181bf2030d5e3f0e656ee74f846a41010c3f82cf11045f40495d1e723 (MATCH)
- background_compaction_runtime.rs: 924b6bfe8c6e4ebca2148a6741338a522415fb4cb304eec099c2a3ccb6100ac8 (MATCH)
- Cargo.lock: 0dcd8464bf873ddb288dd7f30fbcb59785df7a8df18c8889cf19b1fc922f6d9f (MATCH)

### Command
```
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo +1.98.1 test -p opencode-rk-server --test background_compaction_runtime --locked --offline
```

**Result: EXIT_CODE=101 (COMPILE FAILURE)**

**Exact error:**
```
error[E0308]: mismatched types
 --> crates/server/src/auto_compaction.rs:448:29
 |
448 | ) -> Result<(), ApiFailure> {
...
625 | | .await;
 | | - help: remove this semicolon to return this value
626 | | }
 | |_____^ expected `Result<(), ApiFailure>`, found `()`
```

**Root cause:** v5 `auto_compaction.rs.patch` fixes the v3 missing-semicolon issue (E0599 in test), but introduces a new compile error E0308. The function `compact_non_streaming` (line 448) returns `Result<(), ApiFailure>` but line 625 ends with `.await;` (semicolon discards the Result), causing the function to return `()` instead. The README claims to "close the missing semicolon/function-test-module boundary" but the semicolon at line 625 causes a return-type mismatch in the lib, not the test.

**Test status:** NOT reached - lib compile failure prevents test execution
- Expected test: `successful_turn_holds_background_result_without_durable_checkpoint` (1 test)

**Log:** integration/held_summary_v5_check.log

**Note:** The v5 candidate correctly fixes the test-file `StreamExt` import and the lib's `.await;` boundary, but the lib itself fails to compile due to the `.await;` discarding the Result return value. The writer must fix the semicolon in auto_compaction.rs at line 625.

### Hashes after (verified immutable - all match before)
Same as above - no source modifications during compile.

---

## Part 9: OAuth V6 EXACT COMMIT verification (12018b66)

### Commit identity
- Full SHA: 12018b66653c1d6edfa60479ccbc949d6f898886
- Message: "providers: dispatch persisted OAuth credentials"
- Parent: 44f9d40993e8e6cecb2d14223b9ce982390e5ece
- Verified via `git cat-file -t` and `git rev-parse`

### Snapshot
- Method: `git archive 12018b66 | tar -x` (exact commit archive, NO patches applied)
- Location: /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/oauth-v6-commit-validate

### Hash verification (all 5 sources + Cargo.lock match v6 manifest, before AND after)
- persisted_auth.rs: 8574740f74a33e1e6f4172cade32cf6cdef2439fd90e71cd971c14c9e257eb83 (MATCH)
- responses.rs: a3206369bc334421ddfa48b6b161d6147427bc5e53878f99a9ba4d0fb9d41b2c (MATCH)
- oauth_request_runtime.rs: 9ef22f8964337094d069cf0d1b58eade006f1931f912312fda0a96da1b44f5ac (MATCH)
- zero_expiry_metadata.rs: 7453fb0733637cb1d7994657c9463554dd3b1b3a3436650ad88a3312ffdc3a4d (MATCH)
- server lib.rs: 8cbe8e583d9a9914b7fe44fd3d79c0d3e31c0e5226d2cce0ef618c5f52439d1b (MATCH)
- Cargo.lock: 0dcd8464bf873ddb288dd7f30fbcb59785df7a8df18c8889cf19b1fc922f6d9f (MATCH)

### Command 1: Provider tests
```
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo +1.98.1 test -p opencode-rk-providers --test oauth_request_runtime --test zero_expiry_metadata --test responses_usage --test responses_request_retry --locked --offline
```

**Result: EXIT_CODE=0**

| Test file | Passed | Failed | Filtered |
|-----------|--------|--------|----------|
| oauth_request_runtime | 6 | 0 | 0 |
| responses_request_retry | 8 | 0 | 0 |
| responses_usage | 13 | 0 | 0 |
| zero_expiry_metadata | 5 | 0 | 0 |
| **Total** | **32** | **0** | **0** |

### Command 2: Server lib provider_failure_tests (gated on providers GREEN)
```
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo +1.98.1 test -p opencode-rk-server --lib provider_failure_tests --locked --offline
```

**Result: EXIT_CODE=0**
- 1 passed, 0 failed, 247 filtered out

### Hashes after (verified immutable - all match before)
All 6 hashes match the values above. No source modifications during test execution.

### Logs
- integration/oauth_v6_commit_providers.log
- integration/oauth_v6_commit_server_lib.log

### Note
This is NOT a reuse of prior v5 patch results (32+1). It is a fresh validation of the exact committed SHA `12018b66653c1d6edfa60479ccbc949d6f898886` via git archive (no patches applied). The v6-manifest hashes match the exact commit archive, confirming the committed candidate is correct. The prior v5 patch-based validation had a semicolon return type bug in auto_compaction.rs which the committed version does NOT have.

---

## Part 10: Composer Repair Diagnostic (base commit 12018b66)

### Commit identity
- Full SHA: 12018b66653c1d6edfa60479ccbc949d6f898886
- Parent: 44f9d40993e8e6cecb2d14223b9ce982390e5ece
- Patch: line-home-completion-repair.patch (SHA 237e3eac82ef3096a626eb1074e51970e08e495e8f71cdfcd0b6a8b1c80454ba)

### Patch verification
- **git apply --check: EXIT 0** (patch applies cleanly to exact commit archive)
- Only 3 CLI files modified: native_composer.rs, native_completion.rs, tui_entry.rs
- No other source/index/commit/DB changes

### Hash verification (before patch vs after patch, immutable during test)
**Before patch:**
- native_composer.rs: 4236248114b335cba9a715275973058d2bea42112d33d1b4250a4a4b7ab662f6
- native_completion.rs: 145e491d6fc5714c38fc41acfa3b37571adb3688b53c934ae6009c52703ec3f0
- tui_entry.rs: 945309c21ccbab8d3a00926eb821eaddaea424e1b9b2e069cb112d1835bbac74
- Cargo.lock: 0dcd8464bf873ddb288dd7f30fbcb59785df7a8df18c8889cf19b1fc922f6d9f (unchanged - no dep changes)

**After patch (before test) AND after test (immutable):**
- native_composer.rs: f8853d5eda99c0d0e4d691dea37bcbf72f914c18e82f229cc1a7d04d79d75c1e
- native_completion.rs: ab124b394efba749dfd58be5d4c521bd1769ee942fd29c009eada2640f070750
- tui_entry.rs: d15c6813c67cbb2dbcf47ba33d7795137754471cc108090b39af1becce6f6379
- Cargo.lock: 0dcd8464bf873ddb288dd7f30fbcb59785df7a8df18c8889cf19b1fc922f6d9f (UNCHANGED)

### Command
```
cd <snapshot> && CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 DYLD_LIBRARY_PATH=/Users/mymac/.local/lib cargo +1.98.1 test -p opencode-rk-cli --bin oc2 --features native --locked --offline native_composer
```

**Result: EXIT_CODE=0 (compilation succeeded, 2 test assertion failures)**

**Test output:**
```
running 20 tests
test native_composer::line_cursor_tests::composer_home_end_dispatch_is_line_local ... FAILED
test native_composer::line_cursor_tests::ctrl_line_home_end_preserve_other_lines_and_unicode_boundaries ... FAILED
... (18 more tests ... ok)

test result: FAILED. 18 passed; 2 failed; 0 ignored; 0 measured; 754 filtered out; finished in 0.00s
```

**2 test failures (assertion failures, NOT compile errors):**

1. `native_composer::line_cursor_tests::composer_home_end_dispatch_is_line_local` at native_composer.rs:774:
```
assertion `left == right` failed
 left: 0
 right: 5
```

2. `native_composer::line_cursor_tests::ctrl_line_home_end_preserve_other_lines_and_unicode_boundaries` at native_composer.rs:764:
```
assertion `left == right` failed
 left: "\n終"
 right: " suffix\n終"
```

### Analysis
- Compilation SUCCESS (99 warnings, 0 errors)
- 754 tests filtered out (expected - filter `native_composer` matches 20 tests)
- 18 passed, 2 failed
- Failures are in line home/end cursor dispatch tests - the repair patch modified native_composer.rs but the line_cursor_tests assertions are not satisfied
- The `native_completion` and `tui_entry` test filters were NOT run (gated on `native_composer` being fully GREEN - 2 failures prevent proceeding)

### Remaining unexecuted
- `native_completion` filter tests (gated on native_composer GREEN)
- `tui_entry` tests (if any match filter)

### Independent review ongoing
- Test pass does NOT constitute review approval or state integration
- Source presence/compile does NOT establish product acceptance

**Log:** integration/composer_v2_result.log
**SLOT RELEASED.**

---

## Part 11: Composer V2 Diagnostic (base commit 12018b66)

### Commit identity
- Full SHA: 12018b66653c1d6edfa60479ccbc949d6f898886
- Patch: line-home-completion-repair-v2.patch (SHA ebd91efa8282ecb5b5b706287dd847feb9b43eac2cc789c0cd98ce5b93ab1c23)

### Patch verification
- **git apply --check: EXIT 0** (patch applies cleanly to exact commit archive)
- Only 3 CLI files modified: native_composer.rs, native_completion.rs, tui_entry.rs

### Hash verification (base commit -> after patch -> after test, immutable during test)

**Base commit (git archive 12018b66, BEFORE patch):**
- native_composer.rs: 4236248114b335cba9a715275973058d2bea42112d33d1b4250a4a4b7ab662f6
- native_completion.rs: 145e491d6fc5714c38fc41acfa3b37571adb3688b53c934ae6009c52703ec3f0
- tui_entry.rs: 945309c21ccbab8d3a00926eb821eaddaea424e1b9b2e069cb112d1835bbac74
- Cargo.lock: 0dcd8464bf873ddb288dd7f30fbcb59785df7a8df18c8889cf19b1fc922f6d9f

**After patch (verified identical BEFORE and AFTER test run):**
- native_composer.rs: caf1b49a2aae8941581175411264a69dcbeae4fb9419cec1a79a97d794d0326f
- native_completion.rs: 804927519529d26df4bb51895a6baeea65f903070d932522163328d7319ecaea
- tui_entry.rs: d15c6813c67cbb2dbcf47ba33d7795137754471cc108090b39af1becce6f6379
- Cargo.lock: 0dcd8464bf873ddb288dd7f30fbcb59785df7a8df18c8889cf19b1fc922f6d9f (UNCHANGED)

### Command
```
cd <snapshot> && CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 DYLD_LIBRARY_PATH=/Users/mymac/.local/lib cargo +1.98.1 test -p opencode-rk-cli --bin oc2 --features native --locked --offline native_composer; echo "CARGO_EXIT=$?"
```

**Result: CARGO_EXIT=101** (test failure)

| Filter | Passed | Failed | Filtered |
|--------|--------|--------|----------|
| native_composer | 20 | 1 | 754 |

**V2 improvement vs V1:** One test fixed (`ctrl_line_home_end_preserve_other_lines_and_unicode_boundaries` now PASSES)

**1 remaining failure:**
`native_composer::line_cursor_tests::composer_home_end_dispatch_is_line_local` at native_composer.rs:776:
```
assertion `left == right` failed
 left: ""
 right: "\n"
```

### Gate status: FAIL (1 failure remains)
- `native_completion` tests NOT run (gated on native_composer fully GREEN)
- `tui_entry` tests NOT run (gated on prior stages GREEN)

### Summary of All 11 Validation Runs

| # | Task | Exit | Passed | Failed | Detail |
|---|------|------|--------|--------|--------|
| 1 | Usage Tests (77629e5) | 0 | 21 | 0 | PASS |
| 2 | Context Postcommit (44f9d409) | 0 | 14 | 0 | PASS |
| 3 | Held-Summary v2 | 101 | 0 | 0 | FAIL - compile (missing StreamExt) |
| 4 | OAuth V4 | 101 | 0 | 0 | FAIL - compile (missing module) |
| 5 | OAuth V5 | 101 | 31 | 1 | PARTIAL - InvalidJson assertion |
| 6 | Held-Summary v3 | 101 | 0 | 0 | FAIL - compile E0308 (semicolon) |
| 7 | OAuth V6 EXACT COMMIT (12018b66) | 0 | 33 | 0 | PASS |
| 8 | Held-Summary v5 | 101 | 0 | 0 | FAIL - compile E0308 (semicolon) |
| 9 | Held-Summary v7 | 1 | 0 | 0 | FAIL - patches don't apply to base |
| 10 | Composer Repair v1 (12018b66) | 101 | 18 | 2 | 2 assertion failures |
| 11 | Composer Repair v2 (12018b66) | 101 | 20 | 1 | 1 assertion failure (V1:2 fixed) |
| 12 | Held-Summary v8 (12018b66) | 101 | 0 | 0 | FAIL - compile: unclosed delimiter (truncated patch) |

---

## Part 12: Held-Summary V8 Diagnostic (base commit 12018b66)

### Commit identity
- Full SHA: 12018b66653c1d6edfa60479ccbc949d6f898886
- Patch: line-home-completion-repair-v2.patch (SHA ebd91efa8282ecb5b5b706287dd847feb9b43eac2cc789c0cd98ce5b93ab1c23)

### Patch verification
- **git apply --check: EXIT 0** (all 4 v8 patches apply cleanly to base 12018b66)

### Hash verification
**Before patch (base archive of 12018b66):**
- auto_compaction.rs: 724f2cb441e800e0ce03cecb61798adbf106f4b9e2f67df1380277e875d7901b
- fallback_compaction.rs: 113fe45cefbe4b2c76f4ccbb7c3e12ddcb0e66451fbca4e62c203264b851e8f5
- lib.rs: 8cbe8e583d9a9914b7fe44fd3d79c0d3e31c0e5226d2cce0ef618c5f52439d1b
- background_compaction_runtime.rs: N/A (not in base)
- Cargo.lock: 0dcd8464bf873ddb288dd7f30fbcb59785df7a8df18c8889cf19b1fc922f6d9f

**After patch (all match manifest-v8.sha256):**
- auto_compaction.rs: 80690eafd593feea3f2d00d0a666518c1999e5ef1b9d232027dcaeee74f7b1d3
- fallback_compaction.rs: bfb67893f78ae7e4cd3324d3aab1d76b7c31e80ff19ec883eeb0e2159ea61c67
- lib.rs: 15188618064a2f13a37597055adcda16e5b5d0331d505ca04810975e1739e931
- background_compaction_runtime.rs: f764cbbadb995a7940761f732d31684483f01865046de6a643d7d78a98240fd0
- Cargo.lock: 0dcd8464bf873ddb288dd7f30fbcb59785df7a8df18c8889cf19b1fc922f6d9f (unchanged)

**After compile failure (immutable - all match post-patch):**
All 5 hashes identical to post-patch values.

### Command
```
cd <snapshot> && CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo +1.98.1 test -p opencode-rk-server --test background_compaction_runtime --locked --offline; echo "CARGO_EXIT=$?"
```

**Result: CARGO_EXIT=101** (compile failure, NOT test failure)

### Exact compile errors
```
error: this file contains an unclosed delimiter
 --> crates/server/src/auto_compaction.rs:647:31
  |
494 | ) -> Result<(), ApiFailure> {
  |                             - unclosed delimiter
...
643 |     let window = tokio::select! {
  |                                 - unclosed delimiter
...
646 |         result = async {
  |                        - unclosed delimiter
647 |             match previous_win
  |                               ^

error: could not compile `opencode-rk-server` (lib) due to 1 previous error
```

### Root cause
The v8 `auto_compaction.rs.patch` is truncated. The final hunk:
```
@@ -108,29 +644,4 @@
```
replaces 29 lines with only 4 lines ending in:
```
            match previous_win
```
followed by `\ No newline at end of file`. The patch does not close the `match`, `tokio::select!`, `async` block, or the enclosing function. The patch file itself is malformed/truncated.

### Gate status: FAIL (compile error)
- Tests NOT executed (compile failure)
- `auto_compaction` lib tests NOT run (gated on prior stage compile GREEN)
- Independent review pending - compilation failure prevents execution

**Log:** integration/held_summary_v8_check.log
**SLOT RELEASED.**

---

## SLOT STATUS
All slots released. No running cargo processes.
No canonical source/index/refs/commits/DB/daemon writes made. User daemon 73389/4096 and private fixtures preserved. Slot remains RELEASED (light diagnostic only).

## Summary of All 12 Validation Runs

| # | Task | Exit | Passed | Failed | Detail |
|---|------|------|--------|--------|--------|
| 1 | Usage Tests (77629e5) | 0 | 21 | 0 | PASS |
| 2 | Context Postcommit (44f9d409) | 0 | 14 | 0 | PASS |
| 3 | Held-Summary v2 | 101 | 0 | 0 | FAIL - compile (missing StreamExt) |
| 4 | OAuth V4 | 101 | 0 | 0 | FAIL - compile (missing module) |
| 5 | OAuth V5 | 101 | 31 | 1 | PARTIAL - InvalidJson assertion |
| 6 | Held-Summary v3 | 101 | 0 | 0 | FAIL - compile E0308 (semicolon) |
| 7 | OAuth V6 EXACT COMMIT (12018b66) | 0 | 33 | 0 | PASS |
| 8 | Held-Summary v5 | 101 | 0 | 0 | FAIL - compile E0308 (semicolon) |
| 9 | Held-Summary v7 | 1 | 0 | 0 | FAIL - patches don't apply to base |
| 10 | Composer Repair v1 (12018b66) | 101 | 18 | 2 | 2 assertion failures |
| 11 | Composer Repair v2 (12018b66) | 101 | 20 | 1 | 1 assertion failure (V1:2 fixed) |
| 12 | Held-Summary v8 (12018b66) | 101 | 0 | 0 | FAIL - compile: unclosed delimiter (truncated patch)