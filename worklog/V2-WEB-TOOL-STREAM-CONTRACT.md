# V2-WEB-TOOL-STREAM-CONTRACT (B1, independent test owner, frozen RED)

Base: `7fe4656` on branch `v2/web-tool-stream` (isolated worktree
`/Users/mymac/Projects/opencode-rk-v2-web-tool-stream`).
Scope grant: only `web/src/lib/api.tool-stream.test.mjs` + this worklog.
`web/src/lib/api.ts`, `web/src/App.tsx`, all other sources: frozen read-only.

## Failure (B1)
Real browser API `runTurnStream` rejects the daemon's real tool events as
unknown. Server evidence (read-only, this worktree):
- `crates/server/src/lib.rs:1115-1123`: NDJSON
  `{"type":"tool_call","call_id","name","arguments"}` (`write` args redacted
  to `"{}"`); emitted between provider rounds.
- `crates/server/src/lib.rs:1292-1300`: NDJSON
  `{"type":"tool_output","call_id","name","output"}` (truncated bound);
  persisted as `MessageRole::Tool "[name] output"` (`1311-1333`).
- `crates/server/src/lib.rs:1412-1435`: `201`, `application/x-ndjson`,
  `no-store`; `{"type":"error","code","message"}` terminal errors.
- Client `web/src/lib/api.ts:911-987`: `parseTurnStreamEvent` handles only
  `user_message / reasoning_summary_delta / assistant_delta /
  assistant_message / error`; `default:` throws
  `'Server returned an unknown turn stream event'` (`986`).
- Bounds retained by the contract: `MAX_TURN_STREAM_BYTES` 2 MiB,
  `MAX_TURN_STREAM_LINE_BYTES` 256 KiB, reasoning-summary 8 KiB
  (`api.ts:821,883-884`).

Upstream pin `95daf906` (read-only
`/Users/mymac/Projects/opencode-upstream-reference`): upstream has no
`tool_call` NDJSON wire event; tool visibility there is via message parts
(`packages/opencode/src/session/message-v2.ts`, `part.type === "tool"`,
tool result rendering) consumed by the Solid app over the SDK/event stream,
not by the marketing/docs Astro site (`packages/web/`). The native
`tool_call`/`tool_output` NDJSON shape is an opencode-rk extension of the
native daemon; the contract below pins the daemon's actual payload keys
(`type/call_id/name/arguments/output`) and strict string typing, without
copying upstream transport semantics.

## Contract (observable accepted tool stream)
1. A fragmented NDJSON `user_message -> tool_call -> tool_output ->
   assistant_delta* -> assistant_message` sequence resolves with
   `executed:true`, final assistant text equal to the streamed deltas, and
   only after the tool round (completion reached, not truncated at the
   tool events).
2. Two successive identical tool-turn requests parse identically
   (second-turn parser stability; no cross-request state leakage).
3. Malformed tool events (missing/non-string `call_id`/`name`/`output`)
   are rejected with an Error, never silently accepted. NOTE: in the
   current RED these reject via the generic unknown-event path; post-fix
   they must reject via strict tool-field validation. The test regex
   (`/tool|invalid|unknown/i`) accepts either message so the guard holds
   in both states without weakening.
4. A late `{"type":"error",...}` after tool events propagates the server
   message (currently masked: the unknown-event throw fires first).
5. The 256 KiB/line and 2 MiB/stream safety bounds keep rejecting
   oversized tool output lines (passing in RED, retained as guard).

No new handler names are pinned: tests observe callbacks only via the
existing `onUserMessage/onAssistantDelta/onAssistantMessage` surface; the
fix may add `onToolCall/onToolOutput` without breaking this contract.

## RED evidence (clean, compiling)
Command:
`rtk /usr/bin/arch -arm64 node --experimental-strip-types --test web/src/lib/api.tool-stream.test.mjs`
(workdir `/Users/mymac/Projects/opencode-rk-v2-web-tool-stream`)
Result: 5 tests, 2 pass (malformed rejection, byte bound), 3 fail with the
B1 signature `Error: Server returned an unknown turn stream event` at
`api.ts:986` via `parseTurnStreamEvent` <- `consumeLine` <- `runTurnStream`:
- `tool round streams to completion` FAILS (unknown event on tool_call)
- `two successive tool turns` FAILS (same, first request)
- `late server error propagates` FAILS (unknown masks server message)

Harness follows `api.auth.test.mjs`: real `./api.ts` under
`--experimental-strip-types`, loopback `node:http` server, real `fetch`
with relative-URL rewrite, synthetic 64-hex fragment credential, owned
server/global cleanup per test, no user browser state or credentials.

## Status
Component RED only. Existing fake opener probes are not browser proof;
real-browser acceptance (explicit tab, same daemon, tool + reload/resume)
still pending and is owned by the G6 gate, not this lane. NO source
implementation performed; NO assertions weakened. Awaiting source
implementation lane after freeze.
