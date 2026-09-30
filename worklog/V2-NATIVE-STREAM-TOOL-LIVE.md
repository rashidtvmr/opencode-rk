# V2 Native stream/tool candidate

Base: `771607929cc6f2026384a820de68d7f6d8c606d6`

This candidate owns the native daemon NDJSON client and preserves accumulated
assistant/reasoning text across server tool rounds. Network work is owned by one
Tokio task, with bounded frames, retained text, total bytes, queue count, no
proxy, no redirects, and abort-on-drop. The UI polls stdin so provider deltas
remain visible while input stays responsive.

Heavy validation was later granted. The frozen native stream fixture reached
all observable stream/tool/second-turn assertions (including early
`STREAM_PARTIAL_7d91`, typed write output, final continuation text, and second
turn marker). The Python harness then failed during its own daemon cleanup with
`PermissionError: [Errno 1] Operation not permitted` from `os.killpg`; this is
recorded as a harness cleanup failure, not converted to GREEN. The captured
screen and sanitized fixture evidence are retained under the approved temp
artifact directory. No tests, fixtures, helper assertions, lockfiles,
credentials, or databases were modified.
