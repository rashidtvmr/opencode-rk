# TOOL-015 worklog

## Claim

Implement an MCP-specific policy gate in the existing MCP client plus a caller-owned elicitation path that can be approved, denied, cancelled, or timed out without an internal unbounded request queue.

## Source evidence

- Candidate base revision: current main after SESS-019/SESS-020 task work.
- `tasks/TOOL-015.md`: REQ-037, five test obligations.
- `docs/upcoming-features/codex-harness.md:22,57-61`: MCP calls carry their own policy gate; elicitation requires timeout and cancel ownership.
- `docs/upcoming-features/SYNTHESIS.md:23,40`: structured permission escalation is preferred over free-text permission asks.
- Current code: `crates/tools/src/mcp.rs` has a basic async MCP client and tool invocation path but no MCP-specific policy decision or elicitation lifecycle.

## Observed scenario

`McpClient::call_tool` currently checks only whether a tool is registered and then returns a simulated successful result. It has no deny/ask decision before execution and no bounded elicitation wait/cancel path.

## Target boundary

- Product implementation: `crates/tools/src/mcp.rs` only.
- Independent RED tests: `crates/tools/tests/mcp_policy_elicitation.rs` only.
- Status/evidence: this worklog and `tasks/TOOL-015.md`; `ralph.json` already records TOOL-015 as `in-progress`.

## Tests

- Independent RED file: `crates/tools/tests/mcp_policy_elicitation.rs`.
- Frozen SHA-256: `f5a79454451ee8850dcd62f22f16d55f1809a8f528ad5681aee7e70f062e83f3`.
- The worker's single authoring run failed to compile on the missing policy/elicitation API; its final tightened API shape was not rerun by the worker, so that remained authoring feedback only.
- The first main-side scaffold run exposed two remaining signature mismatches (`default_allow()` and optional receiver); after aligning only signatures, `cargo test -p opencode-rk-tools --test mcp_policy_elicitation` compiled and failed behaviorally: 2 passed, 3 failed. This is the frozen RED baseline.
- GREEN: `cargo test -p opencode-rk-tools --test mcp_policy_elicitation` -> 5 passed, 0 failed.
- Regression: `cargo test -p opencode-rk-tools` -> 49 library tests plus 5 MCP policy/elicitation integration tests passed; doc tests passed.
- Frozen hash was rechecked unchanged before GREEN.
- Plan validation: `python3 tools/validate_plan.py` -> `validate_plan: OK stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `git diff --check` for the TOOL-015 product/test/task/worklog paths passed.

## Decisions

- Preserve existing callers with a default-allow MCP policy, then enforce explicit per-tool deny/elicitation rules before simulated execution.
- Keep elicitation ownership with the caller rather than storing an internal pending queue. A one-shot response is naturally bounded to one decision per call.
- Use the existing Tokio dependency for timeout/cancellation semantics; add no dependency.
- `call_tool` applies the per-tool policy before execution: allow executes, deny returns a typed policy error, and elicit returns a typed approval-required error.
- `call_tool_with_elicitation` owns no pending collection; it consumes one caller-supplied one-shot receiver and maps negative response, dropped sender, and timeout to distinct typed errors.
- Approved elicitation executes once and is not replayed automatically after an ambiguous outcome.

## Remaining unknowns

- Formal acceptance remains verifier/controller-owned; this worklog records implementation and test evidence only.
