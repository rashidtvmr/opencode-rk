# V2 shell owned-lifetime contract

## Package and scope

- **Package:** G3/tool lifecycle, source-only independent test owner.
- **Base:** `7ffdc6e13385027a1ecabff66d570ad88b963502`.
- **Branch/worktree:** `v2/shell-cancel-contract`, `/Users/mymac/Projects/opencode-rk-v2-shell-cancel-contract`.
- **Writable paths:** only `crates/tools/tests/shell_owned_lifetime.rs` and this file.
- **Status:** frozen compiling RED on current canonical `59dc523`.

The tests freeze the narrow ownership guarantee that a live direct child belongs
to the async execution owner. They do not cover timeout policy, shell parsing,
output streaming, PTY behavior, process trees, or the separate CLI executor
path. They use `/bin/sh` with a fixed authored script and argv, with no dynamic
`-c` command string, inherited environment, network, or user database.

## Current source evidence and failure

On the base revision, `crates/tools/src/shell_tool.rs`:

- `ShellTool::execute` spawns at lines 186--208 but does not set
  `kill_on_drop(true)`.
- `ShellTool::cancel` at lines 255--260 calls `child.kill()` without polling
  the returned future. This is the observed Clippy `let_underscore_future`
  failure and does not actually signal the process.
- `Drop` takes no async ownership action after `cancel` (lines 268--273).
- The existing `cancel_on_drop` unit test (lines 341--352) drops a
  `JoinHandle`, which detaches the task; it does not abort/await the task and
  does not assert that an actual child PID is dead.

The retained parent evidence is the prior fresh-workspace Clippy output
`T/opencode/v2-workspace-gates-f6e2d3b-20jfnwb5/clippy.log` and its
`commands.json`; the parent should freeze that manifest together with this test
hash before implementation.

## Frozen behavioral cases

1. **normal completion:** a successful direct command announces its PID through
   the marker, exits successfully, and leaves no process row for that PID.
2. **broker denial:** denial occurs before spawn and leaves no marker or child.
3. **unpolled future:** dropping an unpolled `execute` future has no side
   effects.
4. **aborted owner:** a live, marker-verified direct child is observed before
   `JoinHandle::abort`; abort plus await must reclaim it within two seconds.
5. **timeout owner:** timeout of the owner must not detach a live child; the
   child must be reclaimed within two seconds.

The marker is bounded to 64 bytes and accepted only when `ps` reports the PID
as a live direct child of this test process. Failure cleanup signals only that
owned PID, waits at most two seconds, and never converts forced cleanup into a
passing assertion. No test is ignored or relies on an unbounded sleep/output
fixture.

## Upstream authority classification

Pinned upstream is OpenCode commit
`95daf90670b7c039c436c85537da5fbfe2205b41` (per
`sources/upstream.lock.json`). Relevant evidence:

- `packages/core/src/tool/bash.ts:158-170` constructs the native V2 process
  with `detached` on POSIX, `forceKillAfter: 3 seconds`, timeout, and bounded
  output through `AppProcess.run`.
- `packages/opencode/src/tool/shell.ts:542-555` races exit/abort/timeout and
  explicitly calls `handle.kill({ forceKillAfter: "3 seconds" })` for abort or
  timeout.
- `packages/core/src/cross-spawn-spawner.ts:338-340,395-397,432-434` contains
  the shared process kill/escalation lifecycle.
- `packages/core/src/session/execution.ts:16-18` defines interruption as work
  owned by this process; `packages/core/src/session/run-coordinator.ts:94-103` interrupts
  the owning fiber.

These are upstream lifecycle authorities, not a demand to reproduce the
TypeScript architecture. The Rust contract intentionally tests only the
shared compatibility/library lifetime invariant currently exposed by public
`ShellTool::execute`; it does not claim parity with the native V2 `bash.ts`
engine or CLI executor.

## Resource and security bounds

- Disposable `TempDir` only; no existing database or host fixture mutation.
- Fixed `/bin/sh` and `/bin/sleep` authored script; direct argv only.
- `env_clear` remains product behavior; the test supplies no ambient secrets.
- Readiness, cancellation, and cleanup deadlines are each <=2 seconds; output
  is finite and bounded.
- On an unexpected platform `ps` ownership cannot be proven, the contract must
  remain RED rather than using unsafe or fake process proof. No implementation
  is authorized by this file to expand into process-tree killing or timeout
  redesign.

## Verification record

The earlier focused command run on candidate `1cf7377` is retained only as
historical, non-acceptance evidence. It used the pre-review permissive probe,
was not run under the parent's final locked/isolated command manifest, and
must not be used to freeze this stricter contract or claim acceptance.

The revised source-only candidate intentionally has not run Cargo or any
runtime/process fixture after this review. The parent/controller must run the
final compiling RED with an offline locked Cargo command, isolated HOME, and
one approved heavy slot, then freeze the exact test hash and command manifest.

Historical command run on candidate `1cf7377`:

```text
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-tools --test shell_owned_lifetime -- --nocapture
```

The test target compiled and ran five tests. Three passed (normal completion,
broker denial, and unpolled future); two correctly failed RED:
`aborting_owned_task_reclaims_live_direct_child` and
`dropping_timeout_owner_reclaims_live_direct_child`, each reporting its
marker-verified PID remained alive after the two-second deadline. The existing
library warnings include the known unpolled `child.kill()` issue. No PTY,
browser, Docker, Node, or unrelated subprocess fixture gate was run.

That historical result is candidate evidence only and is superseded by this
stricter source revision. The parent/controller must independently classify any
forced cleanup as failure evidence rather than GREEN.

The controller's final source review makes the timeout handshake explicitly
owner-confirmed: the test first proves the live direct-child PID, then signals
the task to start its timeout. The timeout consumes the owned execution future,
and the result must report cancellation. PID marker reads retain at most 65
bytes and only `NotFound` establishes absence; permission/read failures cannot
be mistaken for a no-spawn or cleanup success. Earlier fixture revisions remain
preserved. This final source still requires an isolated locked/offline compiled
RED before the hash is frozen.

## Current-base preparation

The controller selectively prepared the net fixture from preserved
`efa0b13667f1ec6ec3fd3c47bce4c141ec0db6c6` on canonical base
`d87ec3bfb40a0ecb954f4a9518e72c9b31e9027b`. Only the two declared test/worklog
paths are added. The earlier controlled compiling RED on `efa0b13` reports
**3 passed, 2 failed, 0 ignored**, exit 101, with the aborted and timeout
owners leaving live owned PIDs 24681 and 24863. The fixture's subsequent
forced cleanup is recorded as failure evidence. Its receipt is retained at
`v2-shell-lifetime-red-efa0b13-06obckmp` under the approved artifact parent.

The prepared fixture has a manual `Debug` implementation that reads the probe
message, eliminating the diagnostic from derived-only tuple-field use while
preserving all test cases and assertions. The controller re-inspected the full
pinned native `bash.ts`, scoped spawner acquire/release and compatibility
shell abort/timeout path at the exact reference checkout. Library source
`shell_tool.rs` is unchanged from the earlier RED; its lifetime repair has not
yet been leased. The final hash will be frozen after the current-canonical
fixture compiles and produces the same two semantic failures.

## Frozen exact canonical RED

On exact integrated test revision
**`59dc52365a288d67b904e01252f7c2031e0f6336`**, the controller ran:

```text
/usr/bin/arch -arm64 cargo test --offline --locked -p opencode-rk-tools --test shell_owned_lifetime -- --test-threads=1 --nocapture
3 passed; 2 failed; 0 ignored; exit 101
```

The fixture compiles. `aborting_owned_task_reclaims_live_direct_child` reports
owned PID 27762 alive after abort/await and the two-second deadline;
`dropping_timeout_owner_reclaims_live_direct_child` reports owned PID 27944
alive after consuming/dropping the execution future. Normal completion,
broker denial with no marker, and unpolled-future no-spawn controls pass.
The two forced `-KILL` cleanups are failure evidence, not acceptance. After
the fixture process exits, controller `ps` probes show both PIDs absent
(exit 1, empty stdout/stderr); those observations are separately retained.

Frozen SHA-256 values:

- Test: `8b39c6eafcbb065e42752c8c1d19bac53be6928612fc59b1da654465353097fd`.
- Command spec: `e163ef1c17aebe7cd91fc169c58873ad84f44e5ee45a6f2856872df8274b26fe`.
- Serial isolated runner: `ea83e4503525a18d913da80f329e4f5d9de9f2079e9d95f0f9d835a9595c59a6`.
- RED log: `68ac42a3234e05b357ed939e399045f14d3dfc072889f1f1f7bf8364d5ba0d5f`.

Receipts and `post-red-owned-pid-probes.json` are retained at
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-shell-lifetime-canonical-red-59dc523-aq7hnycg`.
The manifest uses disposable HOME/XDG paths, literal Cargo/Rustup homes,
whitelisted environment, two build jobs, one test thread, a 90-second deadline
and 32 MiB log bound. Product `shell_tool.rs` and existing in-module tests
remain unchanged. Only the direct-child lifetime implementation is unlocked;
native daemon interruption and process-tree semantics require their own gates.
