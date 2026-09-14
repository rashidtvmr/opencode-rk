# Codex Harness: claims worth borrowing

Pins: codex `35d9e4bc4d7a44dc84d86a54f4e815ea5cad6986` (verified `rtk git rev-parse HEAD`, 2026-09-14).
Target: opencode-rk `a754746dbaeb136286ef1862017ae49e68e89289`.
Codex workspace version `0.0.0` (codex-rs/Cargo.toml:153). Scope: harness patterns only, not model routing or TUI skin.

## Claim ledger

| # | Claim | Evidence (codex path:line) | Target REQ / story | Decision |
|---|-------|---------------------------|-------------------|----------|
| 1 | Agent loop is typed Op enum over Thread, not free-form messages | protocol/src/protocol.rs:592, core/src/codex_thread.rs:82,320,332 | REQ-002 AUTO-002 | Adopt shape (submission state machine, below #3) |
| 2 | Turn has explicit TurnInputMode (text vs structured review) | core/src/turn_input.rs:133, protocol.rs:4056 ReviewDecision | REQ-013 AGENT-007 | Adopt for plan mode (#8) |
| 3 | Granular per-tool approval config persists per session | protocol.rs:1010 GranularApprovalConfig, approvals.rs:108,139,206 | REQ-021 SEC-001/003, REQ-030 | Adopt (#1) |
| 4 | Execpolicy is prefix-rule allowlist with README contract | execpolicy/src/policy.rs:28,225, decision.rs:9, README.md:5-37 | REQ-025 SEC-002/012 | Adopt (#2) |
| 5 | Sandboxing is OS-backend (seatbelt/landlock/windows), prompt is not sandbox | core/src/sandboxing/seatbelt.rs:863, landlock.rs:23, windows.rs:36 | REQ-027 SEC-004/005 | Cite as prior art, keep our broker |
| 6 | RolloutRecorder writes append-only JSONL; fork boundary is typed | rollout/src/recorder.rs:86, thread-store/src/types.rs:185,196,205,217 | REQ-006/008 SESS-011 | Adopt dual form (#5) + split fork (#4) |
| 7 | Queue + compression split from thread store | queue_store.rs:56, compression.rs:29,196 | REQ-006 SESS-001 | Adopt boundary, defer auto-compact tuning |
| 8 | Models manager separates registry metadata from runtime choice | models-manager/src/manager.rs:56,218, model_info.rs:143 | REQ-011 CAT-001/002 | Align naming only, our API already planned |
| 9 | SubAgent vs Internal session source typed at protocol level | protocol.rs:2825 SubAgentSource, :2879 SessionSource, codex_delegate.rs:50,188 | REQ-009/010 AGENT-005 | Adopt label for audit |
| 10 | InterAgentCommunication op + request_permissions tool | protocol.rs:804, permissions.rs:71,226, safety.rs:17,29 | REQ-010/021 AGENT-008 SEC-001 | Adopt request_permissions (#7) |
| 11 | Skills have loader/invocation/model split | skills/loading.rs:16,33, invocation.rs:14, model.rs:8, lib.rs:77 | REQ-017 EXT-001/002 | Adopt gate (#6) |
| 12 | MCP calls carry own policy gate + elicitation path | mcp_tool_call.rs:14,58, mcp_policy.rs:6,52 | REQ-020/021 SEC-010 | Adopt with #6 |
| 13 | Hooks typed (types.rs:92); TUI/CLI/SDK are thin over Thread | hooks/types.rs:92, cli/src/main.rs:156,221, sdk/typescript/src/thread.ts:41 | REQ-015/020 BASE-004 | Affirms our thin-client plan, no new dep |

## Ranked Top 8

### 1. Granular approvals + per-session cache
What: per-tool allow/ask/deny persisted for session lifetime (claim 3).
Lean-value: kills repeat prompts, the top agent-UX tax; tiny HashMap, no service.
Cost/risk: low. Risk is stale allow after fork; clear cache on fork boundary.
Maps-to: REQ-021, REQ-030; SEC-001, SEC-003, SEC-017.

### 2. Execpolicy prefix rules
What: declarative path/command prefix allowlist evaluated before tools (claim 4).
Lean-value: O(prefix len) check replaces regex soup; auditable file.
Cost/risk: low. Risk is over-permissive prefix; longest-match + tests.
Maps-to: REQ-025, REQ-030; SEC-002, SEC-012.

### 3. Turn submission state machine
What: submit → running → interrupted/complete as typed Op transitions (claims 1-2).
Lean-value: cancels cleanly, no orphan tasks or unbounded queues.
Cost/risk: low-medium. Touches controller states; lease one slice.
Maps-to: REQ-002, REQ-012; AUTO-002, AGENT-003/004.

### 4. Fork / revert / rollback split
What: three distinct ops: branch copy, pointer rewind, truncating rollback (claim 6).
Lean-value: revert is free (pointer move); only fork copies.
Cost/risk: low. Risk is conflating them in UI; keep separate API names.
Maps-to: REQ-008; SESS-011, UI-004.

### 5. Dual rollout: JSONL log + state.db snapshot
What: append-only JSONL is truth; sqlite holds queryable snapshot (claim 6).
Lean-value: crash recovery by replay; bounded DB writes.
Cost/risk: medium. Two writers must agree; recorder owns order.
Maps-to: REQ-006, REQ-033; SESS-001, DB-001/003.

### 6. Skill-gated approvals + MCP elicitation
What: skill invocation passes policy gate; MCP tools can ask back via elicitation (claims 11-12).
Lean-value: reuses one approval path for skills, hooks, MCP.
Cost/risk: medium. Elicitation needs timeout + cancel owner.
Maps-to: REQ-017, REQ-020, REQ-021; EXT-001, SEC-010/011.

### 7. request_permissions tool
What: agent explicitly asks for a named capability mid-turn (claim 10).
Lean-value: replaces out-of-band prompt injection with audited grant.
Cost/risk: low. Risk is nag-spam; rate-limit + session cache (#1).
Maps-to: REQ-021, REQ-026; SEC-001, SEC-013.

### 8. Plan mode + structured review child
What: turn mode where child returns ReviewDecision accept/revise, not text (claim 2).
Lean-value: review becomes machine-checkable; feeds steer loop.
Cost/risk: medium. New turn variant; keep to one story.
Maps-to: REQ-013, REQ-009; AGENT-007/008, AGENT-002.

## Remaining unknowns
- Compression trigger thresholds (compression.rs:196) not benchmarked; need token counts under our budgets.
- models.dev refresh cadence vs models-manager polling; our versioned API may diverge.
- Windows sandbox parity untested on this Linux host; seatbelt/landlock only cited.
- Elicitation UX in our TUI unspecified; needs UI-008 design before #6 lands.
- SDK thread.ts:41 surface vs our singleton multi-client (REQ-015) mapping unchecked.
