# Wave 4 - Connect services and complete core web parity

Status: DRAFT. Entry: `P1-G3` independently passed.

## Demonstration that closes this wave
Native and web clients both perform real agents/MCP/tools/workspace/history/Library/artifact workflows; all Wave 3 core integration-pending rows close.

## Work packages
- **P1-W4-01 - Complete durable foreground/background subagent execution**. A parent starts a real child, receives real results, and both clients can inspect/steer/cancel/resume it without privilege escalation or orphan work.
- **P1-W4-02 - Activate account routing, fallback and truthful usage telemetry**. The app actually uses its routing subsystem; changing an account or fallback choice is observable in provider requests and both status panels.
- **P1-W4-03 - Finish real MCP configuration, discovery and lifecycle**. MCP enablement changes actual tool registration and execution, not merely a toggle in a state object.
- **P1-W4-04 - Integrate LSP, Git and constrained local PTY services**. UI commands and model tool calls invoke the same authorized real service, with no hidden auxiliary runtime when off.
- **P1-W4-05 - Wire rules, memory, skills, custom commands and native extensions**. A user can prove that a selected rule, skill, memory file or custom command affected the real executed turn.
- **P1-W4-06 - Complete web workspace, sidebar, history and navigation parity**. Web navigation reflects the same sessions, projects, pin/search/archive and context state as the daemon and native UI.
- **P1-W4-07 - Complete web message actions and structured turn presentation**. Message actions perform the promised operation and structured turns never confuse reasoning summaries/tool output with the final answer.
- **P1-W4-08 - Transmit real attachments and finish the local Library**. A provider receives the intended authorized attachment content; upload success alone does not complete the feature.
- **P1-W4-09 - Complete safe artifact edit, preview, run and apply**. Artifacts are durable useful files, and execution/application is explicitly authorized rather than browser-side arbitrary code.
- **P1-W4-10 - Accept complete core web and cross-client integration**. Both clients can perform the core local product workflows with the same durable outcomes and no blocked required core control.

## Gate criteria
- All required core native/web controls invoke the actual engine/service and persist correct outcomes.
- Close each Wave 3 deferred-in-wave service row with recorded evidence.
- No disabled required core capability is counted as done.

## Capacity and sequencing
Freeze shared contracts first; then dispatch only non-conflicting one-file candidates. Reserve integration and independent review capacity. Use one heavy validation token across the wave, not one build per worker. A wave may contain multiple bounded dispatch rounds; wave count is not a concurrency or time estimate. Keep repair capacity rather than filling every slot with new breadth.
