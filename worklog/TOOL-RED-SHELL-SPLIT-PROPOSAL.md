# TOOL-RED-SHELL split proposal

Status: proposal only. No frozen test, production source, manifest, or Cargo file
was edited. Implementation remains blocked pending controller and test-owner
authorization.

## Claim and evidence

- Task: `TOOL-RED-SHELL-SPLIT-PROPOSAL`
- Session: `ses_f2dc08073ffe3gPMjh3dJ5uqhm`
- Branch: `plan/TOOL-RED-SHELL-SPLIT`
- Candidate revision inspected: `8c514e05f63ad4de5942e43d56e3f180126fee2a`
- Original frozen file: `crates/tools/tests/phase1_shell_broker.rs`
- Original SHA-256: `2853960b7ad711d88384b136db5b37eca1dd827e82c4339990c120267a4e11ed`
- Independent verdict: `worklog/TOOL-RED-SHELL-VERIFY.md:7-12`, `ACCEPT WITH SPLIT`
- Original RED receipt: `worklog/TOOL-RED-SHELL-VERIFY.md:36-57`

The original hash and verifier receipt are superseded-by-authorized-refreeze
history only. They are not permission to mutate the frozen file. The controller
must preserve the original blob, hash, command, and failure output in the split
receipt before authorizing any test-owner edit.

## Contradiction proof

### Shared execution path

`crates/tools/src/executor.rs:108-134` routes both `bash` and `shell` directly
to `execute_shell`. `crates/tools/src/executor.rs:240-248` constructs
`bash -c <command>` and places `tokio::time::timeout` around the output future.
There is no authorization check or child kill on the timeout path.

`crates/tools/src/executor.rs:136-147` does not provide an authorized shell
escape: `execute_authorized` authorizes only `read`; every other tool falls back
to the unbrokered `execute` path.

### t01-t03 authorization contract

- t01, `phase1_shell_broker.rs:47-75`, calls `ToolExecutor::new().execute`
  with `bash` and a marker-producing command. It requires denial before spawn
  and marker absence.
- t02, `:77-101`, calls the same executor path with `shell`; it requires no
  marker and no sentinel in result text.
- t03, `:103-164`, calls `RegistryDispatcher::new().dispatch` with `bash`; it
  requires denial, marker absence, output-store stats `(0, 0)`, and all permits
  returned.

The dispatcher currently documents and constructs `AllowAll` by default at
`crates/tools/src/registry_dispatch.rs:64-78,131-145`. Its policy check is at
`:187-195`, then it invokes the unbrokered executor at `:196-207`. This is the
missing default-deny/broker wiring contract.

### t04 cancellation contract

t04, `phase1_shell_broker.rs:166-195`, again constructs `ToolExecutor::new`
at `:172`, then invokes the same `execute` method at `:176` with `bash` and a
50 ms timeout. It requires an error containing `timed out` at `:184-187`, then
waits 1.5 seconds and requires no late marker at `:189-194`.

The contracts are mutually unsatisfiable on this call surface:

1. A correct t01-t03 broker fix must deny this executor path before spawn when
   no concrete authorization is present.
2. The same t04 call has no authorization distinction. That fix therefore
   returns denial before `execute_shell`; t04 cannot observe a timeout.
3. Making this call spawn so t04 can observe timeout permits the same unbrokered
   mechanism t01 and t02 require to be denied. A timeout value is not an
   authorization capability and must not become one.
4. Keeping the child alive after `timeout` cannot repair the contradiction. It is
   a separate lifetime defect in `execute_shell`: the timeout drops the output
   future, while the spawned process is not owned by a kill-on-drop child guard.

Verifier evidence records this exact result at
`worklog/TOOL-RED-SHELL-VERIFY.md:85-92`: after a broker gate, t04 fails its
`timed out` assertion because the same call is denied before spawn.

## Public API testability audit

### Existing public path that is not a valid t04 RED

`ShellTool` has a public broker builder at
`crates/tools/src/shell_tool.rs:145-149`, public `execute` at `:163-166`, and
broker authorization before spawn at `:189-211`. Direct argv execution enables
an explicit process authorization. The current implementation also already
uses `process_group(0)` and `kill_on_drop(true)` at `:232-237`, with drop/cancel
cleanup at `:340-367`.

This path can express a legitimately authorized direct executable fixture, but it
does not provide a RED for the missing executor cancellation behavior: existing
`ShellTool` cancellation tests are already green, and `bash -c` is intentionally
`RequireHuman` under `PermissionBroker` (`crates/security/src/lib.rs:290-300`).
Moving t04 to `ShellTool` unchanged would either test an already-green path or
need an unauthorized opaque shell string. Neither is an authentic RED.

### Missing public path for the intended contract

No current public API expresses an explicitly authorized shell spawn through the
`ToolExecutor` path:

- `ToolExecutor::execute_authorized` only handles `read` and falls back for
  shell (`executor.rs:136-147`).
- `RegistryDispatcher::DispatchPolicy` is only a boolean preflight hook
  (`registry_dispatch.rs:64-68`). It does not carry a `PermissionBroker`, grant,
  operation digest, or cancellation owner into `ToolExecutor`.
- `RegistryDispatcher::new` uses `AllowAll` (`registry_dispatch.rs:131-135`),
  which cannot be the authorized cancellation fixture.

Conclusion: the intended executor cancellation seam is not publicly testable as
an authorized RED today. Do not author a compile-fail test, use private access,
or treat `AllowAll` as a broker grant. A small API-contract/discovery lane must
decide the public authorized-spawn seam first.

## Controller-authorized split

The minimum transformation is two independent contracts, with one API decision
before the cancellation RED:

### Contract A: shell broker authorization

- Future task ID: `TOOL-RED-SHELL-BROKER-REFREEZE`
- Task type: `RED authoring`
- Exact owned test file: `crates/tools/tests/phase1_shell_broker.rs`
- Contents: retain t01, t02, and t03; remove only t04 under explicit
  test-owner authorization. Preserve all denial, no-spawn, no-secret,
  no-store-write, and permit-reclaim assertions byte-for-byte.
- RED command:
  `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test phase1_shell_broker -- --test-threads=1`
- Expected RED: compiling suite; t01-t03 fail for missing broker/default-deny
  behavior, not for imports or fixture errors.
- Freeze: record the post-split file hash and command manifest only after the
  independent verifier reproduces the three intended failures.

### Contract B: authorized process cancellation

- Future task ID: `TOOL-RED-SHELL-CANCEL-RED`
- Task type: `RED authoring`
- Exact owned test file: `crates/tools/tests/phase1_shell_cancellation.rs`
- Contents: one cancellation RED file, authored only after the API lane below
  supplies a public authorized-spawn entry point. The fixture must execute a
  disposable direct executable with PID and late-marker paths passed as argv,
  never concatenate an untrusted shell string, and use an explicit scoped broker
  authorization. Start through the real public API, cancel or time out the
  owned operation, assert child/process-group death within a bounded deadline,
  then assert no delayed marker after a bounded settle period.
- RED command:
  `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test phase1_shell_cancellation -- --test-threads=1`
- Expected RED: compiling test; authorized child remains alive or writes the
  late marker on the pre-fix implementation. A test that passes current code is
  invalid and must not be frozen.
- Security boundary: no `AllowAll` default, no bypass token, no secret fixture,
  no detached task, no unbounded wait. The explicit grant/policy must bind the
  exact program, argv, workspace, requester, policy version, and expiry as
  applicable to the selected API.

### API contract/discovery prerequisite

- Future task ID: `TOOL-RED-SHELL-CANCEL-API`
- Task type: `research`
- Exact owned artifact: `worklog/TOOL-RED-SHELL-CANCEL-API.md`
- Purpose: choose and document the public authorized process API before any
  cancellation RED authoring. Candidate shape may be an executor method that
  receives a broker plus an explicit grant/scope, or a public direct-argv
  execution owner whose cancellation lifetime is observable. The choice must
  preserve broker-before-spawn, human-grant semantics for opaque `bash -c`,
  process-group cleanup, and bounded ownership.
- Validator: source-level API inventory plus a bounded public-call probe. If no
  acceptable seam exists, record the exact missing signature and stop. Do not
  write a test that fails only because an API is absent; that is a contract
  decision, not product RED evidence.

The controller must register these proposed IDs and dependencies before any
claim. The API lane precedes `TOOL-RED-SHELL-CANCEL-RED`; both RED lanes require
independent verification. Broker implementation may begin only after Contract A
is independently refrozen. Cancellation implementation remains blocked until
Contract B is independently refrozen.

### Future implementation ownership

- `TOOL-SHELL-BROKER-EXECUTOR` (`implementation`), exact owned file
  `crates/tools/src/executor.rs`; dependency: Contract A freeze. Owns t01/t02
  broker-before-spawn behavior only.
- `TOOL-SHELL-BROKER-DISPATCH` (`implementation`), exact owned file
  `crates/tools/src/registry_dispatch.rs`; dependency: Contract A freeze. Owns
  t03 default-dispatch policy only. Integrator wires any shared policy contract;
  no lane edits another lane's file.
- `TOOL-SHELL-CANCEL-EXECUTOR` (`implementation`), exact owned file
  `crates/tools/src/executor.rs`; dependencies: `TOOL-RED-SHELL-CANCEL-API`
  decision and Contract B freeze. This lane is serial after
  `TOOL-SHELL-BROKER-EXECUTOR`; it owns only the selected public executor
  cancellation behavior. If the API lane selects a different implementation
  file, the controller must issue a replacement card before claiming this one.
- `TOOL-RED-SHELL-BROKER-VERIFY` and `TOOL-RED-SHELL-CANCEL-VERIFY`
  (`verification`), independent sessions; no product-file ownership. Each
  verifier checks its own frozen hash, RED failure cause, fixture cleanup, and
  scope before implementation proceeds.

The API decision is therefore a prerequisite, not a compile-fail RED. Existing
`ShellTool` authorization is sufficient to demonstrate that an authorized
direct process can be represented, but its cancellation tests are already GREEN
(`crates/tools/tests/shell_tool_cancel.rs` and
`shell_tool_process_tree.rs`). The new cancellation RED must target the missing
executor owner path, so the selected public seam must be designed and exposed
before the test file is authored.

## Hash, verification, and rollback lifecycle

1. Controller snapshots the original `phase1_shell_broker.rs` blob, command, and
   hash `2853960b7ad711d88384b136db5b37eca1dd827e82c4339990c120267a4e11ed`.
2. Controller authorizes the test owner to refreeze Contract A and create the
   separate Contract B file. No implementation worker edits either test.
3. Independent verifier checks each file's exact hash, clean scope, compiling
   semantic RED, bounded fixture cleanup, no secret output, and no test edits.
4. Controller records both new hashes as superseding the original hash while
   retaining the original verifier receipt as historical evidence. The original
   hash must remain recoverable from the pre-split commit/blob.
5. Only then may implementation lanes run their respective GREEN commands.
   Acceptance still requires independent verification and integrated-revision
   reruns; this proposal makes no acceptance claim.

If Contract A or B fails independent RED verification, stop. Preserve the failing
fixture and exact output, do not weaken or edit tests, and do not implement
against that contract. If the split is abandoned, restore the original test blob
from the recorded hash and retain this proposal plus the verifier evidence as a
blocked contract review. If Contract A verifies but the API or Contract B lane
blocks, Contract A may proceed only under a separate controller decision; the
parent shell journey remains open and cancellation is unresolved.

## Validation performed in this proposal lane

- `rtk shasum -a 256 crates/tools/tests/phase1_shell_broker.rs` -> unchanged
  `2853960b7ad711d88384b136db5b37eca1dd827e82c4339990c120267a4e11ed`.
- No Cargo, test, browser, database, or heavy process run.
- Required final checks after proposal write: `rtk git diff --check` and
  `rtk git diff --name-only origin/verify/TOOL-RED-SHELL...HEAD`.

## Remaining gaps

- Controller has not authorized the split or registered child task IDs.
- No public executor-level authorized cancellation API currently exists.
- No new cancellation RED is authored or frozen.
- Broker and cancellation implementation, integration, and acceptance remain
  intentionally out of scope.
