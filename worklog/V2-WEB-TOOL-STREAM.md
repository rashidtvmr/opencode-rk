# B1/G6 browser turn-stream adapter

## Handoff

- **Role/package:** implementation owner, B1/G6 browser turn-stream adapter; candidate only (not accepted).
- **Base:** `89780a3121e8626b2d134adcda93dd523a2e53ff` (`v2/web-tool-stream`).
- **Changed paths:** `web/src/lib/api.ts`, `web/src/App.tsx`, this worklog.
- **Current evidence:** the daemon emits `tool_call` with string `call_id`, `name`, and `arguments`, then `tool_output` with string `call_id`, `name`, and `output`; late `error` events must propagate their message. The frozen contract also retains 256 KiB line and 2 MiB stream limits. The pinned upstream at `95daf906` uses message parts for tools rather than this native NDJSON extension, so the implementation follows the current daemon wire contract.

## Implementation

`TurnStreamHandlers` now exposes typed optional `onToolCall` and `onToolOutput` callbacks. The parser validates all required tool fields before invoking callbacks, while preserving existing user, reasoning, assistant delta, and assistant message callbacks. The app keeps only one bounded transient progress string (`Running <name>` / `Finished <name>`), clears it on completion, cancellation, errors, and session changes, and does not synthesize durable message IDs or tool messages.

## Verification

- `rtk /usr/bin/arch -arm64 node --experimental-strip-types --test web/src/lib/api.tool-stream.test.mjs` — **5 passed, 0 failed**.
- `rtk /usr/bin/arch -arm64 node --experimental-strip-types --test web/src/lib/api.auth.test.mjs` — **1 passed, 0 failed**.
- `pnpm typecheck` (in `web/`) — **passed** (`tsc -b --pretty false`).
- `git diff --check` — passed.

## Remaining

This is a **PREVERIFIED candidate**, not `ACCEPTED`: integration, full G6 queue, and real-browser daemon verification remain with the main/integrator lane.
