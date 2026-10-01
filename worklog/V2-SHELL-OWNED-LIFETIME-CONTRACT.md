# V2 shell owned-lifetime contract

## Package and scope

- **Package:** G3/tool lifecycle, source-only independent test owner.
- **Base:** `7ffdc6e13385027a1ecabff66d570ad88b963502`.
- **Branch/worktree:** `v2/shell-cancel-contract`, `/Users/mymac/Projects/opencode-rk-v2-shell-cancel-contract`.
- **Writable paths:** only `crates/tools/tests/shell_owned_lifetime.rs` and this file.
- **Status:** candidate RED contract; not product acceptance and not a full native shell-engine claim.

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

1. **normal completion:** a successful direct command completes and does not
   create the fixture marker.
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
