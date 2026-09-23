# WEB-009 browser verification (verification-only lane)

## Claim and ownership

- Task: `WEB-009`
- Session: `ses_f316984ccffeM8rknVME4AhxbQ`
- Branch: `lane/WEB-009-browser-verify-20260923`
- Base HEAD: `dfb82a9f1575694fbd37a789d84c44d98a597e00`
- Owned file: `worklog/WEB-009-BROWSER-VERIFY.md` plus own ledger row only.
- Non-owned reads: `PLAN.md`, `tasks/WEB-009.md`, `docs/TDD.md`, `docs/SECURITY.md`,
  `docs/CONVERGENCE.md`, `worklog/WEB-009-BROWSER-RED.md`,
  `worklog/WEB-009-BROWSER-IMPL.md`, frozen `web/src/App.browser-red.test.tsx`,
  product `web/src/lib/api.ts`, `web/src/App.tsx`. No product/test edits made.

## Source evidence

- Frozen SHA-256 re-read at verify start:
  `8dbbb07c5d1ff6d1ff10ddcf2fa020aa3fff318a53a6d994d4f6e786fc11901d`
  (`web/src/App.browser-red.test.tsx`).
- RED boundary: `worklog/WEB-009-BROWSER-RED.md:11-32` records old
  `web/src/lib/api.ts:52-55` reasoning-only `AssistantActivity`, `api.ts:859-957`
  limited stream events, `web/src/App.tsx:1347-1351/1379-1395` reasoning-only UI.
- Product delta since RED `7cce88f`: `598e2f3` (`web/src/lib/api.ts`,
  `web/src/App.tsx`) plus worklogs/ledger; `web/src/App.browser-red.test.tsx`
  unchanged.
- Current `web/src/lib/api.ts:52-69`: one `AssistantActivity` with
  `reasoning_summary`, `tool_calls[]` (`call_id/name/state/ok`), `references[]`
  (`label/url`).
- Current `web/src/lib/api.ts:804-895`: fail-closed `normalizeAssistantActivity`
  with legacy defaults (`tool_calls ?? []`, `references ?? []`); bounds
  128 tools / 64 refs / 128-byte tool fields / 1024-byte ref fields / 8 KiB
  reasoning; duplicate `call_id` and duplicate normalized URL rejected;
  non-HTTPS, credential-bearing (`username/password`), control-character, empty,
  over-cap, malformed, and state/ok mismatch (`ok !== (state === completed)`)
  rejected; URL stored normalized as `parsed.href`.
- Current `web/src/lib/api.ts:944-945,1010-1063,1087-1111,1120-1122,1211-1253`:
  bounded NDJSON parsing (`MAX_TURN_STREAM_BYTES=2MiB`,
  `MAX_TURN_STREAM_LINE_BYTES=256KiB`, incremental buffered-line guard);
  production events `tool_call`, `tool_output`, `reference`,
  aggregate `assistant_activity`; final `assistant_message` reconciles streamed
  `assistantText`, `reasoningSummary`, tool/reference sets; aggregate message id
  and tool/reference JSON must agree with accumulated state before projection;
  unknown events fail closed (`unknown turn stream event`); malformed deltas,
  duplicate user/assistant messages, summary/text mismatch, over-limit payloads
  fail closed and never append to answer text.
- Legacy compatibility: `web/src/lib/api.ts:897-914` maps 404 activity to `[]`;
  `api.ts:1150-1158,1180-1185` maps 404 stream/turn to durable POST path.
  Reload adapter `web/src/App.tsx:328-351` merges `/history` plus `/activity`
  into one `assistantActivity` map.
- URL safety: only `https:` with empty `username/password` accepted; links render
  as `target="_blank" rel="noreferrer"` at `web/src/App.tsx:1374-1386`.
- Rendering: `web/src/App.tsx:1347-1353` persisted reasoning collapsed
  (`details` without `open`); `1354-1366` tool activity separate collapsed
  `details`; `1367-1372` final answer expanded outside `details`;
  `1373-1387` References as labeled-link section; `1408-1425` streaming shows
  open reasoning plus streaming answer text only.
- Announcements/cancellation: `web/src/App.tsx:920-924` single polite live
  region bound to `notice`; streamed deltas update `streamingAssistantText` /
  `streamingReasoningSummary`, never the live region; turn abort clears streaming
  state (`App.tsx:366-383,490-500,560-598`).
- Server compatibility spot-check (read-only): production stream emits
  `reasoning_summary_delta`, `reference`, `tool_call`, `tool_output`, terminal
  `assistant_message` with `reasoning_summary` plus `references`
  (`crates/server/src/lib.rs:1318-1348,1463-1471,1661-1673,1522-1531,1764-1772`);
  `/activity` route at `lib.rs:299,622-631`. No `assistant_activity` aggregate
  event is emitted by this server revision; the browser aggregate case is a
  client-side accepted shape exercised only by the frozen fixture, while
  `tool_call`/`tool_output`/`reference` are the production event shapes.
- Server terminal payloads do not include `tool_calls` on `assistant_message`
  (`lib.rs:1523-1529,1765-1771`); browser final reconciliation therefore uses
  accumulated `toolCalls` when `event.tool_calls` is absent
  (`api.ts:1090`). This matches the frozen journey but means production tool
  state reaches the browser through incremental events, not the terminal record.

## Review findings (no patch applied)

1. Frozen browser target GREEN; no code defect found in owned verify scope.
2. Fail-closed bounded parsing confirmed for the frozen shapes and reviewed
   extensions: malformed tool refs, over-cap refs, duplicate IDs/URLs, URL
   credential/protocol violations, control characters, empty fields,
   state/ok mismatch, text/summary mismatch, duplicate messages, unknown
   events, line/total byte caps.
3. One disclosure, not a defect: `api.ts:1015` requires aggregate
   `assistant_activity.reasoning_summary` to equal streamed summary exactly.
   The frozen fixture satisfies this; production does not emit that aggregate.
   No change made (verification-only authority).
4. One compatibility note, not a patched defect: browser `tool_output`
   infers `ok/state` from `output.startsWith('error:')`
   (`api.ts:1046-1047`), matching server persistence convention
   (`lib.rs:1724-1733`). A non-shell/non-error tool failure with a different
   server error encoding would project as `completed`; the frozen contract
   does not cover that server encoding, so no finding is raised beyond this
   note.
5. Accessibility contract from the frozen test confirmed: separate collapsed
   native `details` for Reasoning/Tool, expanded answer, labeled HTTPS link,
   no delta live announcements.
6. `tsc` remains independently blocked by the known unrelated frozen
  `web/src/lib/canvas-model.test.ts` TS6133 unused-import failure; that file
  was not touched (outside verify authority).

## Verification

Serial bounded commands, `web/` Vitest with max/min workers 1:

1. Frozen browser target:
   `timeout 120 pnpm exec vitest run src/App.browser-red.test.tsx
   --reporter=verbose --maxWorkers=1 --minWorkers=1`
   Result: 3 passed, 0 failed.
2. Full web Vitest suite:
   `timeout 180 pnpm exec vitest run --reporter=basic --maxWorkers=1
   --minWorkers=1`
   Result: 17 files, 49 passed, 0 failed.
3. `git diff --check`: passed (only pre-existing ledger modification before
   verify commit; product/test untouched).
4. `tsc`: not re-run as a gate here; recorded as known unrelated blocker from
   `worklog/WEB-009-BROWSER-IMPL.md:29` (`canvas-model.test.ts` TS6133).
   No product inference drawn from it.
5. Frozen SHA-256 after runs: unchanged
   `8dbbb07c5d1ff6d1ff10ddcf2fa020aa3fff318a53a6d994d4f6e786fc11901d`.

## Decision

- Verification-only lane: no implementation change was in scope.
- Code review: no browser code defect requiring `blocked-for-defect`.
- Task status: `blocked` (not `completed`) because production browser evidence
  and branch activity unification remain external/unresolved, per delegation
  orders and `tasks/WEB-009.md:19-30` NOT ACCEPTED boundary. The frozen
  browser journey is GREEN on this branch revision, but that alone is not
  task acceptance.
- Ledger note carries exact test evidence plus the external blockers; zero
  test edits.

## Remaining unknowns / external blockers

- Real production browser evidence (live daemon + provider stream + reload)
  beyond fetch fixtures.
- Branch-session structured activity unification across native producer,
  persistence, and web projection boundaries.
- Independent verifier ruling on aggregate `assistant_activity` shape versus
  production `tool_call`/`tool_output`/`reference` event stream.
