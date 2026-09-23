# WEB-009 disconnect contract review

## Role
Independent frozen-contract review. No product/test edits, no Cargo.

## Scope
- Read only: `crates/server/tests/runtime_wiring_disconnect_http.rs`,
  `crates/server/src/lib.rs`, `crates/tools/src/shell_tool.rs`,
  `crates/server/src/runtime_wiring.rs`, `crates/server/src/turn_parts.rs`,
  existing WEB-009 worklogs, `tasks/WEB-009.md`, `docs/TDD.md`, `docs/SECURITY.md`.
- No edits to product code, test code, Cargo files, or any source file.

## Verdict: BLOCKED / DISPUTED

The frozen test `runtime_wiring_disconnect_http.rs` (SHA-256
`95fbe83a0361256d9e0c229cfc97f3cc0b222e8c6d89625ac5db582ce18199e8`)
is **nondeterministic at the exact revision 732c046** because of an
irreconcilable ordering race between the startup-readiness handshake
and the client-side disconnect. The test requires fixture PID publication
to complete before the process group can be killed, but the readiness
signal precedes the fixture command's PID publication, and the client
disconnects immediately upon seeing `tool_call` -- which is emitted right
after readiness.

## Exact ordering analysis

### Test-side flow (`runtime_wiring_disconnect_http.rs`)

1. **Line 48-55** `disconnect_fixture_command`: builds a bash command that
   writes `$$` to `parent_pid`, spawns `sleep 5 &` as a child, writes
   `$child` to `descendant_pid`, waits for the child, then writes a
   delayed-sentinel file.

2. **Lines 407-413** `wait_for_pids` task: a `spawn_blocking` task polls
   `parent_pid` and `descendant_pid` files concurrently with the HTTP
   client task. Deadline: 4 seconds (line 39).

3. **Lines 258-303** `stream_until_tool_call`: connects to the server,
   sends the turn request, reads the HTTP response stream byte-by-byte
   until it finds `"type":"tool_call"` in the NDJSON, then **immediately
   calls `stream.shutdown(Shutdown::Both)`** (line 300-301).

4. **Line 421** `wait_for_permits`: asserts both permits are returned
   (turn permit released on stream drop).

5. **Lines 422-438**: joins the provider task, asserts
   `connection_closed`, then calls `wait_for_fixture_exit(parent,
   descendant)` (line 428) which polls for process death, then checks
   sentinel absence and message persistence.

### Server-side flow (`crates/server/src/lib.rs` at 732c046)

`create_turn_stream` (line 1117+) sets up a `stream::unfold` with
`TurnStreamState` (line 984+). The state includes:
- `shell_guards: HashMap<String, ShellExecutionGuard>` (line 1012)
- `_permit: HttpTurnPermit` (line 1012 in TurnStreamState, line 992)

When a `FunctionCall` event arrives (line 1301):
1. **Line 1324**: `shell_command(&arguments)` parses the command.
2. **Line 1326-1329**: `state.broker.authorize(&OperationIntent::Tool{...})`
   -- single broker authorization per call.
3. **Line 1331**: `new_shell_execution(command)` creates a
   `ShellExecutionGuard`. This calls `ShellTool::new("bash", vec!["-c",
   SHELL_STARTUP_WRAPPER, "--", readiness_path, command])` and pins
   `shell.execute_with_startup(config, readiness_path, startup_tx)`
   (line 1080).

4. **Line 1333**: `guard.wait_for_startup().await` -- polls until
   the readiness file is written by the startup wrapper.

5. **Line 1335**: Guard inserted into `shell_guards`.

6. **Lines 1385-1407**: `pending_calls` and `history_items` updated,
   then `tool_call` NDJSON event emitted to the client.

### The startup wrapper (`lib.rs:119-120`)

```
const SHELL_STARTUP_WRAPPER: &str =
    "tmp=\"$1.tmp.$$\"; printf '%s\\n' \"$$\" > \"$tmp\"; mv -f \"$tmp\" \"$1\"; exec bash -c \"$2\"";
```

This wrapper:
1. Writes `$$` to a temp file
2. Atomically renames to `readiness_path` (the `ShellArtifact::readiness_path`)
3. `exec bash -c "$2"` -- replaces the process with the fixture command

### The `execute_with_startup` readiness monitor (`shell_tool.rs:278-310`)

```rust
let monitor = async {
    let Some(child_id) = child_id else { ... };
    loop {
        if startup_ready(&readiness_path, child_id) {
            let _ = startup.try_send(child_id);
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
};
tokio::pin!(execution);
tokio::pin!(monitor);
tokio::select! {
    result = &mut execution => { ... }
    _ = &mut monitor => execution.await,
}
```

`startup_ready` (`shell_tool.rs:370-392`) checks:
- File exists at `readiness_path`
- File size <= 20 bytes
- Content is numeric and equals `child_id`

### `ShellExecutionGuard` drop chain (`lib.rs:169-217`)

```rust
struct ShellExecutionGuard {
    artifact: ShellArtifact,
    startup: mpsc::Receiver<u32>,
    execution: Option<Pin<Box<dyn Future<Output = Result<ShellResult, ShellError>> + Send>>>,
    result: Option<Result<ShellResult, ShellError>>,
}
```

Drop is implicit (no explicit `Drop` impl for `ShellExecutionGuard`).
When the guard is dropped:
- `execution: Option<Pin<Box<...>>>` is dropped -> the pinned
  `shell.execute_with_startup(...)` future is dropped
- `ShellTool` is owned by that future -> `ShellTool` is dropped
- `ShellTool::Drop` (`shell_tool.rs:358-367`) calls `self.cancel()`
- `cancel()` (`shell_tool.rs:341-350`) calls `kill_process_group(group,
  Signal::KILL)` on Unix
- `ShellArtifact::Drop` (`lib.rs:162-167`) removes the readiness directory

## The race

The critical race is between two concurrent actions:

**A)** The startup wrapper's readiness file becoming visible (which
unblocks `wait_for_startup` and leads to `tool_call` emission).

**B)** The fixture command writing `parent_pid` and `descendant_pid`
files (which the test polls for in `wait_for_pids`).

The ordering within the spawned bash process is:
```
printf '%s\n' "$$" > "$tmp"         # write readiness temp file
mv -f "$tmp" "$1"                    # atomic rename to readiness_path
exec bash -c "$2"                    # replace process with fixture command
  -> printf '%s\n' "$$" > parent_pid  # fixture writes parent PID
  -> sleep 5 &; child=$!
  -> printf '%s\n' "$child" > descendant_pid  # fixture writes child PID
```

The readiness file is written BEFORE `exec`, and the fixture PIDs are
written AFTER `exec`. The `wait_for_startup` polling loop (5ms sleep)
detects the readiness file and returns. But there is no guarantee that
between the readiness file appearing and the `tool_call` being emitted
and the client disconnecting, the `exec` has completed and the fixture
command has written its PID files.

In fact, the test's `stream_until_tool_call` reads the `tool_call` event
and **immediately shuts down the TCP socket** (line 300). This triggers
the response body drop, which drops `TurnStreamState`, which drops
`shell_guards`, which drops `ShellTool`, which calls `kill_process_group`.

The PID polling task (`wait_for_pids` at lines 407-413) runs concurrently
but is **not synchronized** with the disconnect. It simply polls the
fixture PID files. If the process group is killed before the fixture
command writes its PIDs, `wait_for_pids` will timeout with:

```
crates/server/tests/runtime_wiring_disconnect_http.rs:317: tool fixture PID publication deadline exceeded
```

## Why no implementation fix can resolve this

### Why arbitrary delay is not acceptable
Adding a sleep between `wait_for_startup` and `tool_call` emission would
attempt to give the fixture command time to write PIDs. But:
- `AGENTS.md` forbids arbitrary sleeps as a synchronization mechanism.
- `docs/TDD.md` requires deterministic fixtures, no wall-clock dependence.
- The delay would be arbitrary -- too short and the race persists;
  too long and it slows real turns.

### Why command-specific PID parsing is not acceptable
The server cannot parse the fixture command to learn where PIDs will be
written, because:
- `AGENTS.md`: "No changes to the user's existing OpenCode database..."
  and the server must work with arbitrary user commands.
- The `SHELL_STARTUP_WRAPPER` is a fixed protocol; the fixture command is
  opaque to the server.
- Command-specific parsing would be a test-only hack, not a real
  contract.

### Why the existing GREEN one-pass is not reliable
The GREEN worklog (line 44) claims 1/1 passed. But:
- The implementation at 732c046 uses `tokio::select!` between startup
  completion and the monitor (shell_tool.rs:296-308). When the monitor
  fires first, it sends the PID and then awaits `execution`. When
  execution finishes before readiness (fast commands), it tries to send
  the PID as a fallback (line 299-303).
- The readiness file is written by the wrapper before `exec`. After
  `exec`, the fixture command runs. There is no synchronization between
  the readiness file appearing and the fixture command's PID publication.
- A one-time pass under a specific scheduler/timing does not prove
  determinism. The controller's exact-revision rerun failed, confirming
  nondeterminism.

## Root cause summary

The frozen test has an **inherent ordering race**: the readiness signal
that unblocks `wait_for_startup` (and subsequently `tool_call` emission
and client disconnect) occurs strictly before the fixture command writes
its PIDs. The test polls for those PIDs concurrently but does not wait
for them before allowing disconnect. No implementation change within the
`crates/server/src/lib.rs` boundary (or even in `shell_tool.rs`) can
guarantee that the fixture command's PID writes complete before
`kill_process_group` fires on disconnect, because:

1. The readiness file and the fixture PID files are independent -- the
   wrapper writes the readiness file, then `exec`s into a command that
   writes different files.
2. `wait_for_startup` returns on seeing the readiness file, not the
   fixture PID files.
3. The client disconnects immediately upon seeing `tool_call`.
4. The disconnect drop chain kills the process group synchronously.

## Proposed minimally corrected replacement contract

The test cannot be changed by this review. But the following contract
preserves all assertions while resolving the race:

**Replace the concurrent PID polling + immediate disconnect with a
sequential "wait for fixture readiness before disconnect" step:**

1. After `tool_call` is observed in the HTTP response, the client does
   NOT immediately shut down.
2. Instead, the client polls for the fixture PID files (with the existing
   4-second deadline) to confirm the fixture command has started.
3. Only after PIDs are confirmed published does the client disconnect.

This preserves every assertion:
- Provider connection closure (line 423)
- Shared turn permit return (line 421)
- Parent/descendant death after disconnect (line 428)
- Sentinel absence (line 429)
- No durable tool/assistant success (lines 431-438)

The only change is the **ordering**: wait for PID publication, THEN
disconnect. This makes the test deterministic because the disconnect
cannot precede the fixture's PID publication.

The current test instead has the disconnect fire immediately after
`tool_call`, racing against PID publication. This is why the controller
rerun fails with "PID publication deadline exceeded".

## Source evidence summary

| File:Line | What |
|-----------|------|
| `tests/runtime_wiring_disconnect_http.rs:48-55` | Fixture command writes PIDs then sentinel |
| `tests/runtime_wiring_disconnect_http.rs:258-303` | Client reads `tool_call`, immediately `shutdown(Both)` |
| `tests/runtime_wiring_disconnect_http.rs:311-320` | `wait_for_pids` polls fixture PID files, 4s deadline |
| `tests/runtime_wiring_disconnect_http.rs:342-349` | `wait_for_fixture_exit` asserts parent/descendant dead |
| `tests/runtime_wiring_disconnect_http.rs:407-414` | PID polling runs concurrently with client, not synchronized |
| `crates/server/src/lib.rs:119-120` | `SHELL_STARTUP_WRAPPER` writes readiness before `exec` |
| `crates/server/src/lib.rs:1062-1087` | `new_shell_execution` creates pinned `execute_with_startup` future |
| `crates/server/src/lib.rs:1331-1338` | `wait_for_startup` awaited before guard inserted; `tool_call` emitted after |
| `crates/server/src/lib.rs:176-217` | `ShellExecutionGuard` drop kills process group via `ShellTool::Drop` |
| `crates/server/src/lib.rs:162-167` | `ShellArtifact::Drop` removes readiness dir |
| `crates/tools/src/shell_tool.rs:169-176` | `execute_with_startup` signature |
| `crates/tools/src/shell_tool.rs:278-310` | `tokio::select!` between execution and monitor |
| `crates/tools/src/shell_tool.rs:358-367` | `ShellTool::Drop` calls `cancel()` -> `kill_process_group` |
| `crates/tools/src/shell_tool.rs:370-392` | `startup_ready` checks readiness file only, not fixture PIDs |

## Remaining unknowns

- No way to verify determinism without running the test multiple times
  under varying scheduler conditions. The controller rerun already
  demonstrated failure at the exact revision.
- Whether the GREEN one-pass was a timing lucky case or reflects a
  different scheduler configuration.

## Review status

BLOCKED / DISPUTED. The frozen test has an irreconcilable race between
startup readiness and fixture PID publication, exposed by the immediate
client disconnect on `tool_call`. No product-layer fix can eliminate the
race without either arbitrary delay (forbidden) or command-specific PID
parsing (not a real contract). The proposed correction -- waiting for
fixture PID publication before disconnecting -- preserves all assertions
but requires a test edit, which this review does not authorize.
