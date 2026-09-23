# WEB-009 browser boundary RED

## Claim and ownership

- Task: `WEB-009`
- Session: `ses_f3194bfd0ffe4AgI0nSo6QgPWu`
- Branch: `lane/WEB-009-browser-red-20260923`
- Base: `086f1bf`
- Owned test: `web/src/App.browser-red.test.tsx`

## Source evidence

- `web/src/lib/api.ts:52-55` currently models `AssistantActivity` as only
  `message_id` and `reasoning_summary`; tool lifecycle/reference fields are not
  parsed at the public API boundary.
- `web/src/lib/api.ts:859-957` accepts only user, reasoning delta, assistant delta,
  assistant message and error stream events; structured tool/reference stream
  events fail closed as unknown events, and final `assistant_message` ignores
  tool/reference fields.
- `web/src/App.tsx:1347-1351` renders only a collapsed Reasoning summary for
  persisted activity. `web/src/App.tsx:1379-1395` renders only streaming reasoning
  and answer text. No Tool activity or References UI exists.
- `web/src/App.tsx:920-924` has one polite live region for status notices; the new
  test asserts streaming answer deltas do not enter any live region.
- `web/src/App.tsx:328-355` reloads history plus `/activity` into one
  `assistantActivity` map, so the reload fixture uses the existing public HTTP
  shape and asserts durable fidelity.
- `tasks/WEB-009.md:34-47` requires one structured stream/activity record,
  fail-closed malformed data, collapsed accessible Reasoning/Tools, navigable
  References, restrained announcements, and persisted fidelity.
- `worklog/WEB-009-DURABLE-IMPL.md:44` records browser accessibility as an
  unresolved WEB-009 blocker after native durable implementation.

## RED contract

`App.browser-red.test.tsx` uses only `fetch` at the browser public HTTP boundary.
It covers:

1. A stream carrying reasoning, tool lifecycle, final answer and HTTPS reference
   data must render one assistant turn with collapsed Reasoning and Tool activity,
   expanded final answer, meaningful reference link, and no per-delta live
   announcement.
2. Reloaded `/history` plus `/activity` data must reconstruct the same activity.
3. Malformed tool/over-cap reference data must not fabricate final answer or links.

The current browser model/parser drops or rejects structured tool/reference data;
current rendering has no Tool activity or References projection. RED is therefore
expected and implementation-caused. The malformed case currently passes because
the current parser fails closed at the public boundary; it remains in the frozen
browser contract as a negative guard.

## Verification

Final command:

```text
python3 - <<'PY'
import subprocess, sys
cmd = ['pnpm', 'exec', 'vitest', 'run', 'src/App.browser-red.test.tsx', '--reporter=verbose', '--maxWorkers=1', '--minWorkers=1']
try:
    result = subprocess.run(cmd, cwd='web', timeout=120, text=True)
except subprocess.TimeoutExpired:
    print('TIMEOUT')
    raise SystemExit(124)
print(result.stdout)
print(result.stderr, file=sys.stderr)
raise SystemExit(result.returncode)
PY
```

Expected RED result: 2 failed, 1 passed. Failures are missing `Tool activity`
rendering in stream and reload journeys. The malformed-data guard passes because
the current client does not expose fabricated data.

## Residual

The one browser test cannot independently prove provider-side TCP/SSE behavior or
daemon cancellation/resource release. Those remain covered by server lanes. It
also cannot assert a bearer header because the current same-origin web client has
no browser auth-header contract; the fixture intentionally stays at the existing
public HTTP boundary.
