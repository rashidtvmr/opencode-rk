# APP-012-LIVE-WRITE-CONTRACT-REVIEW

## Task identity
- **Task ID:** APP-012-LIVE-WRITE-CONTRACT-REVIEW
- **Task type:** discovery / integration-proposal (REVIEW only, no product/test/controller edits)
- **Role:** worker (contract reviewer)
- **Route:** @vyce-deepseek-v41 (vyce/deepseek-v4.1)
- **Status:** blocked (integration proposal authored; implementation requires crossing owned slices)
- **Frozen RED commit:** 925bb52f7685e2900244fe4755892dee7c831bf9
- **Frozen test file:** crates/server/tests/live_tool_broker.rs
- **Frozen test SHA256 (from git tree 925bb52...):** `9a9e8424f3ff7ca9e1fbc6c4951bf7fa4044ea9a1ffdc8e8b56e378e5abd0eb6` (confirmed via `git show 925bb52...:crates/server/tests/live_tool_broker.rs | sha256sum`)

## Route/allowlist validation
- Assigned route `@vyce-deepseek-v41` matches delegation prompt. No user allowlist provided; task marked canonical N/A for allowlist. Verified: route matches, no restriction.

## Convergence gate baseline
- command: `python3 tools/convergence_gate.py`
- result: exit 0 but reports `total=83` with backlog exhaustion (80 convergence findings). Gate is known-blocked at the repository level (backlog exhaustion, ralph/FEATURES vs backlog status divergence). This is a pre-existing repository-wide blocker, not introduced by this review lane.

## Source evidence (current tree @ HEAD 7938791)

### Gap 1 — Server live tool dispatch: file-backed args never bound to FileAction
- `crates/server/src/lib.rs:1537-1553` — the non-shell tool dispatch branch:
  - Line 1539: `broker.authorize(&OperationIntent::Tool { name: call.name.clone(), description: ... })`
  - This is the **vacuous generic** `OperationIntent::Tool` intent. Baseline `Decision::Allow` at `crates/security/src/lib.rs:253` (`OperationIntent::Tool { .. } => Decision::Allow`). The `name` is only used for the permit check (`enabled_tools`), not passed into the broker as a file intent.
  - The test imports `file_write_intent` from `tool_authorize` (exists at `crates/security/src/tool_authorize.rs:315`). The server does NOT call it.
- `crates/server/src/lib.rs:1544-1552` — parses `call.arguments` as generic `Value` and passes to `executor.execute(ToolCall::new(...))`. The arguments are NOT parsed into `FileOperation::Write { path, content, .. }` or bound to `FileAction::Write`.

### Gap 2 — ToolExecutor has no file tool dispatch
- `crates/tools/src/executor.rs:106-118` — dispatch:
  ```
  let result = if call.name == "bash" || call.name == "shell" {
      self.execute_shell(&call, effective_timeout).await
  } else if call.name == "echo" {
      self.execute_echo(&call, effective_timeout).await
  } else {
      // "Unknown tool: {}"  <-- this is what the RED test hits
  };
  ```
  No branch for `write`, `read`, `file`, or `edit`. So `write` returns `Unknown tool: write` (the observed RED failure).

### Gap 3 — FileTool::execute_authorized exists but is never called from the server dispatch
- `crates/tools/src/file_ops.rs:158-164` (`FileTool::execute_authorized`) and `crates/tools/src/file_ops.rs:178-200` (`execute_authorized` free function) already implement the correct pattern:
  ```
  if let FileOperation::Write { ref path, .. } = op {
      let intent = OperationIntent::File { action: FileAction::Write, path: path.clone() };
      match broker.authorize(&intent) {
          Decision::Allow => (),
          Decision::Deny { reason } => return Ok(FileResult::failure(...)),
          Decision::RequireHuman { .. } => return Ok(FileResult::failure(...)),
      }
  }
  execute(op)
  ```
  This is the **real** authorization path. But it lives in `crates/tools/src/file_ops.rs` and is never wired to the server's live dispatch at `crates/server/src/lib.rs:1537-1575`.

### Gap 4 — No path/cwd resolution from broker project root to the tool path
- `crates/tools/src/file_ops.rs:251-275` (`write_file`) does `fs::write(path, content)` directly with whatever path is passed — it does NOT validate the path against a project root or resolve it relative to one. When the server passes the raw `secret_path` from tool args, `FileTool::execute_authorized` will authorize correctly (since the broker sees the real path and denies `.env`), but `execute` will still attempt `fs::write` if Allow.
- The test's sentinel is `<tempdir>/.env`. The broker's `authorize_file` at `crates/security/src/lib.rs:256-289` checks `authorize_file(FileAction::Read/Write, path)` which denies `.env` regardless of project root (it's a mandatory secret baseline).

## Observable contract (the RED test pins this)

### T1 (broker unit): deny beats wildcard
- `PermissionBroker` with `PermissionSet::star()` (`*`) still denies `FileAction::Write` to `**/.env` and `rm -rf *` (process).
- Explicit deny-first rule + star also works.
- `file_write_intent(&PathBuf)` from `tool_authorize` binds the real FileAction::Write intent.
- `run_if_allowed` must run zero effects on Deny.
- Status: PASSES (broker already enforces this at `lib.rs:256-289`).

### T2 (stale grant): fake grant cannot release denial
- A `Grant` with stale policy_version (generation bump) cannot release `Decision::Deny`.
- `ToolAuthorizer::authorize_with_grant` → `super::app_policy::decide` must reject stale grants fail-closed.
- Status: PASSES (stale grant logic at `app_policy.rs` already handles this).

### T3 (LIVE RED — the failing case):
- Provider requests `write` tool with `{"path": "<sentinel>/.env", "content": "fixture-content"}`.
- Server advertises `write` tool (registry has it at `registry.rs:108-114`).
- Provider emits `FunctionCall` with `name: "write"` and JSON args.
- Server dispatch (non-shell branch, `lib.rs:1522`) must:
  1. Parse `call.arguments` into `FileOperation::Write { path, content, append }`
  2. Authorize via `OperationIntent::File { action: FileAction::Write, path }` through `state.broker`
  3. On Allow: call `FileTool::execute_authorized` (or equivalent) to perform the write
  4. On Deny: return the denial text (contains "denied")
  5. Persist the denial as a tool transcript record
  6. Feed the denial back to the provider in round 2
- Currently: the dispatch uses `OperationIntent::Tool { name: "write", ... }` (baseline Allow) → executor returns `Unknown tool: write` → `raw_output = "error: Unknown tool: write"` → `is_denial_text("error: Unknown tool: write")` is FALSE → assertion fails.
- Expected (after fix): `raw_output` contains "denied" → passes `is_denial_text`.

## Integration proposal

The fix spans **three owned slices** and cannot be completed within this single review lane's file budget:

### Proposal A — `crates/tools/src/executor.rs` (tools crate, owned file)
Add a `write` (and optionally `read`, `edit`) dispatch branch to `ToolExecutor::execute`:
```rust
// At executor.rs:106-119, extend the dispatch:
let result = if call.name == "bash" || call.name == "shell" {
    self.execute_shell(&call, effective_timeout).await
} else if call.name == "echo" {
    self.execute_echo(&call, effective_timeout).await
} else if call.name == "write" {
    self.execute_write(&call, effective_timeout).await
} else {
    // ... existing "Unknown tool" arm
};
```
Add `execute_write` that parses `call.input` JSON (`path`, `content`, `append`) into a `FileOperation::Write` and calls `FileTool::execute_authorized`. **BUT**: `ToolExecutor::execute` currently has no `PermissionBroker` reference — it must be added as a parameter or the executor must be constructed with one. This changes the `ToolExecutor` signature.

**Constraint:** The frozen test at `live_tool_broker.rs` does NOT directly call `ToolExecutor` — it goes through the HTTP `/turns/stream` endpoint. So the broker must be threaded through the server dispatch, not just the executor.

### Proposal B — `crates/server/src/lib.rs` (server crate, owned file)
At `lib.rs:1522-1553` — replace the vacuous `OperationIntent::Tool` authorization for non-shell tools with file-intent binding:

```rust
// Current (line 1539):
match state.broker.authorize(&OperationIntent::Tool {
    name: call.name.clone(),
    description: "turn tool call".to_owned(),
}) {

// Proposed: for file-backed tools, bind the real FileAction intent:
let intent = if call.name == "write" {
    // Parse path from args, authorize as File::Write
    let path = parse_path_from_args(&call)?;
    OperationIntent::File { action: FileAction::Write, path }
} else {
    OperationIntent::Tool { name: call.name.clone(), description: "turn tool call".to_owned() }
};
match state.broker.authorize(&intent) {
    Decision::Allow => {
        // Dispatch to FileTool::execute_authorized(op, &state.broker)
        let result = executor.execute_file(call, &state.broker);
        ...
    }
    Decision::Deny { reason } => format!("error: tool '{}' denied: {}", call.name, reason),
    ...
}
```
This requires:
- Importing `FileTool`, `FileOperation`, `FileAction` into `lib.rs` (currently only `ToolExecutor` is imported at line 99).
- Parsing JSON args for `path`/`content` from `call.arguments` (a Serde Value or string).
- Adding a `write`/`read` dispatch to `ToolExecutor` or inlining `FileTool::execute_authorized` call.
- The broker is already constructed at `lib.rs:1241` (`PermissionBroker::new(SecurityPolicy::lean_default(...))`) and stored in `TurnStreamState.broker` — it is available at the dispatch site.

### Proposal C — `crates/tools/src/file_ops.rs` (tools crate, already exists)
The `FileTool` and `execute_authorized` function already exist and implement the correct zero-side-effect-on-deny pattern. No changes needed to the authorization logic itself. The wiring gap is that it's never called from `B` and `A`.

### Proposal D — `crates/tools/src/executor.rs` signature change
`ToolExecutor::execute` must gain a `&PermissionBroker` parameter (or the executor must be constructed with one), so that `execute_write` can call `FileTool::execute_authorized`. This touches the `ToolExecutor` public API — affecting any existing caller.

**Existing callers of `ToolExecutor::execute`:**
- `crates/server/src/lib.rs:1547-1553` (the dispatch site being fixed)

## Crossing owned slices: blocker classification

This integration proposal **cannot** be completed within a single lane's single owned file:
- **Lane 1 (tools crate)** must modify `executor.rs` to add file-tool dispatch and thread a `PermissionBroker` through `ToolExecutor::execute`.
- **Lane 2 (server crate)** must modify `lib.rs` to (a) import `FileTool`/`FileOperation`/`FileAction`, (b) parse file-tool arguments, (c) bind the real `FileAction::Write` intent at the dispatch site `lib.rs:1539`, (d) pass `&state.broker` to the executor call.
- The frozen test `live_tool_broker.rs` spans both crates (imports from `opencode_rk_security`, `opencode_rk_server`, and exercises the live HTTP path).

**This is a multi-slice integration task.** A single worker cannot own both `executor.rs` and `server/src/lib.rs` simultaneously (one owned file per lane per AGENTS.md section 7). The smallest safe decomposition:

1. **Lane TOOLS-EXEC:** Modify `crates/tools/src/executor.rs` — add `PermissionBroker` to `ToolExecutor`, add `execute_write` and `read`/`edit` branches, dispatch to `FileTool::execute_authorized`. Frozen test for this lane: a unit test in `executor.rs` that asserts `write` to `.env` returns denied text and zero side effects (using a real broker with `*` permission).
2. **Lane SERVER-DISPATCH:** Modify `crates/server/src/lib.rs` — import file tool types, bind `FileAction::Write` intent at dispatch (replace vacuous `OperationIntent::Tool`), pass `&state.broker` to executor. RED proven by the existing `live_tool_broker.rs` T3 assertion.

## RED proof plan (for the implementer lanes)

### Lane TOOLS-EXEC RED:
- Unit test in `executor.rs`: `ToolExecutor::with_broker(&star_broker).execute(ToolCall::new("write", "write", json!({"path": "/work/project/.env", "content": "x"})))` must return `ToolResult { success: false, output: "", error: Some("...denied...") }` and no `.env` file must be created.
- `ToolResult.success` must be `false` (denied), OR if the convention is `success: true` with denial text in `output`, the test must assert the denial text. (Convention TBD — see current server code at `lib.rs:1554-1560`: `if result.success { result.output } else { result.error }`.)
- Currently: returns `Unknown tool: write` with `success: false`, `error: Some("Unknown tool: write")`. The test must assert `is_denial_text(error)` fails today and passes after fix.

### Lane SERVER-DISPATCH RED (frozen `live_tool_broker.rs` T3, commit 925bb52):
- Already compiles (imports exist in tree). Fails on `is_denial_text("error: Unknown tool: write")` → false.
- After fix: dispatches `write` through `FileAction::Write` intent → broker denies `.env` → `output` contains "denied" → passes.
- Zero side-effect assertion: `sentinel.exists()` must be false after denial (the broker denies before any FS write at `file_ops.rs:182-199`).

## Resource bounds
- Test bounds already specified in frozen test: request <= 256KiB, response <= 512KiB, oneshot <= 64KiB, 20s socket timeout, `RUST_TEST_THREADS=1` for the env-mutating T3 test.
- No 8 GiB impact: no new Cargo deps needed (all required types already exist: `FileOperation`, `FileTool`, `FileAction`, `OperationIntent`, `PermissionBroker`, `ToolAuthorizer`).

## Security posture
- The broker already enforces mandatory `.env` denial (`lib.rs:256-289`) — the fix must preserve this. No new secret access. The test uses a disposable tempdir sentinel and existence-check only.
- No regex-as-sandbox: the broker uses real path-component matching (`lib.rs:256-289`), not regex.
- No shell-string concatenation for file paths: arguments are parsed as structured JSON and bound to `FileAction::Write`.
- The fix must NOT weaken the mandatory secret baseline at `lib.rs:253` (Tool intent baseline Allow) or the file authorization at `lib.rs:256-289`.

## Smallest safe next owned task
**Lane TOOLS-EXEC**: `crates/tools/src/executor.rs` — thread `PermissionBroker` through `ToolExecutor`, add `write`/`read`/`edit` dispatch via `FileTool::execute_authorized`. RED: unit test asserting denied-write returns denial text and zero side effects. This is the foundational change; SERVER-DISPATCH follows.

## Remaining limitations
- The frozen test T3 requires both lanes to be integrated and re-run on the integrated tree. Neither lane alone makes T3 pass.
- The test also asserts the denial is fed back to the provider in round 2 (`bodies[1]["input"]` contains `function_call_output` with denial text) — this requires the server dispatch to populate `CallOutputItem` correctly, which happens at `lib.rs:1582-1587`.
- The test asserts the persistence of a `tool` role transcript record with the denial text (`fetch_messages` → `messages[].role == "tool"` → `body.text` contains denial). This path is at `lib.rs:1616-1619` (`sessions.append_text`). Already wired; the denial text must be the output value.
- No Cargo.toml changes needed (no new deps); this is a pure dispatch/wiring fix.
