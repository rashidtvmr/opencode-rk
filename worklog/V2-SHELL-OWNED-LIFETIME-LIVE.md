# G3-SHELL-DIRECT-CHILD-OWNERSHIP

## Scope and status

- **Package/gate:** G3 tool lifecycle, direct-child ownership repair.
- **Base:** `89aeaa8` (full-resolve base supplied by the controller).
- **Worktree:** `/Users/mymac/Projects/opencode-rk-v2-shell-owned-lifetime-live`.
- **Writable product path:** `crates/tools/src/shell_tool.rs` only, before the
  existing frozen `#[cfg(test)] mod tests` suffix.
- **Additional path:** this worklog.
- **Status:** SOURCE-ONLY CANDIDATE; not PREVERIFIED and not ACCEPTED.

## Current frozen evidence

The protected external fixture is
`crates/tools/tests/shell_owned_lifetime.rs`, hash
`8b39c6eafcbb065e42752c8c1d19bac53be6928612fc59b1da654465353097fd`.
The canonical compiling RED is `59dc523`; its normal completion, broker
denial, and unpolled-future controls pass, while aborting or timing out the
owner leaves a live marker-verified direct child until controller forced
cleanup. The fixture was not modified.

No Cargo, test, build, or process-fixture command was run by this worker.

## Implementation

The source-only repair makes the async execution owner responsible for the
direct child without changing command authorization, environment clearing,
argv construction, output capture, or result/error behavior:

1. `Command::kill_on_drop(true)` is enabled before spawn, so dropping an
   owner future after a child is installed synchronously initiates child
   cleanup through Tokio's owned child handle.
2. `ShellTool::cancel` uses synchronous `Child::start_kill()` rather than
   creating and dropping the unpolled `Child::kill()` future.
3. The normal wait path remains responsible for reaping and then clearing
   `self.child`.
4. The allowlist lookup uses `contains`, and the test-only timeout helper and
   import are cfg-gated to resolve the known mechanical diagnostics without
   changing the frozen test body.

## Upstream classification and evidence

Pinned OpenCode is `95daf90670b7c039c436c85537da5fbfe2205b41` from
`sources/upstream.lock.json`. The full pinned checkout was inspected for the
relevant lifecycle authorities:

- `packages/core/src/tool/bash.ts:158-170` creates a child with explicit
  detached/forced-kill lifecycle options and bounded `AppProcess.run`.
- `packages/opencode/src/tool/shell.ts:533-555` races exit, abort, and timeout,
  then awaits `handle.kill` on abort/timeout.
- `packages/core/src/cross-spawn-spawner.ts:373-403,427-436` uses scoped
  acquire/release ownership, waits for termination, and escalates to SIGKILL.

These are the upstream lifecycle authorities. This package is deliberately a
narrow compatibility-library direct-child repair; daemon interruption,
process-tree ownership, timeout-policy redesign, and output-bound changes are
outside scope.

## Hashes and handoff

Computed without running Cargo or fixtures:

- Frozen test: `8b39c6eafcbb065e42752c8c1d19bac53be6928612fc59b1da654465353097fd`
- Cargo.lock (unchanged): `63ef5299dd93286950af00388796375b06aefc5a4a3eedfa38361954fefb6f03`
- Existing test suffix before source edit: `33d5f035f94806f77b9d49e85bab7165a87dac563bcb607b95fc7708db2b5ee8`
- Existing test suffix after source edit: `33d5f035f94806f77b9d49e85bab7165a87dac563bcb607b95fc7708db2b5ee8`

The protected test body and suffix text were not edited; the first provisional
hash extraction accidentally included the newly cfg-gated import and was
discarded. Using the exact existing `#[cfg(test)] mod tests` suffix, the hash
is unchanged. The external fixture and Cargo.lock remain unchanged at the
hashes above.

This candidate requires the parent/controller to run the locked isolated RED /
GREEN gate independently, verify live owned PIDs disappear without relying on
forced cleanup, and accept only on the exact integrated SHA.

The controller prepared this source from preserved `5c6fd50` against current
canonical `89aeaa8`, alongside the reviewed mechanical tools diagnostic package.
Both `Duration` and `timeout` imports are test-only because the existing private
timeout helper is used only by frozen in-module tests. This final import cfg is
mechanical compiler maintenance; the test suffix and external fixture remain
unchanged. Independent combined verification is still required.
