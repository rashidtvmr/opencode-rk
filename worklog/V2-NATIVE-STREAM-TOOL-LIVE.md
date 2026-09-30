# V2 Native stream/tool candidate

Post-fix source investigation confirmed that `std::io::Stdin::read` could
pre-buffer PTY input after the first byte, while readiness was checked against
the bridge-owned descriptor. The native bridge now owns both readiness polling
and the one-byte kernel read through `Renderer::read_input`, eliminating that
split-buffer lifecycle/input stall without adding a reader thread or runtime.

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
## Controller preverification compiler maintenance

The first actual release build of `3280a24` failed with E0382: `rustix::io::read`
takes the mutable slice by value, so retrying after `EINTR` requires a fresh
reborrow. The controller adds `&mut *buffer` to retain the caller's slice across
retry iterations. This compiler repair changes no frozen tests or input/stream
contract. Release and installed product gates must rerun on the corrected SHA.

Independent source review also found that the partial-row replacement checked
the settled `assistant:` prefix while creating `assistant (streaming):` rows.
The controller now replaces only the active streaming row and removes it when
the real terminal assistant/error event arrives, including after intervening
tool rows. The unused settled HTTP submission function is removed. Frozen
semantic fixtures remain unchanged; runtime preverification is still pending.
