# V2 Native stream/tool candidate

Base: `771607929cc6f2026384a820de68d7f6d8c606d6`

This candidate owns the native daemon NDJSON client and preserves accumulated
assistant/reasoning text across server tool rounds. Network work is owned by one
Tokio task, with bounded frames, retained text, total bytes, queue count, no
proxy, no redirects, and abort-on-drop. The UI polls stdin so provider deltas
remain visible while input stays responsive.

Validation intentionally source-only pending the parent heavy-slot grant. No
tests, fixtures, helper assertions, lockfiles, credentials, or databases were
modified.
