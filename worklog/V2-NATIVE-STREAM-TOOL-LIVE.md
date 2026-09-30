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

## Exact integrated acceptance

Package: **NATIVE-STREAM-TOOL-RESTART**. Candidate
`ed727a6577527fc560a6dfc736fbdb0479654f06`, based on
`771607929cc6f2026384a820de68d7f6d8c606d6`, was independently PREVERIFIED using
the current canonical executable contract. Integration base is
`85dfa9df6927e73922b33020d88c32a2204e01cc`; exact integrated release SHA is
**`de7e05fefba5374246078674432dcba07b0b5ff2`**. All seventeen source-candidate
commits remain preserved on the candidate branch; the six net changed paths
were byte-identical when integrated as one coherent product package:

```text
crates/cli/src/native_turn.rs
crates/cli/src/tui_entry.rs
crates/opentui-bridge/Cargo.toml
crates/opentui-bridge/src/safe_renderer.rs
crates/server/src/lib.rs
worklog/V2-NATIVE-STREAM-TOOL-LIVE.md
```

Pinned upstream `95daf90670b7c039c436c85537da5fbfe2205b41` evidence:
`packages/core/src/session/runner/publish-llm-event.ts:246-263,291-389`
publishes incremental text/tool events, and
`packages/tui/src/context/sync.tsx:398-413` accumulates live part deltas.
The approved current contract additionally requires the real broker-authorized
write, retained partial-plus-final text, second turn and identical typed history
after CLI/daemon restart. `Cargo.lock` and protected semantic assertions remain
unchanged. The independent test owner's cleanup-only contract repair is frozen
as `c6d8d561a96136061c4d6e87e183bad144e762df17847fcfca4a347d0d30633e`.

All following gates ran on exact integrated release `de7e05f`, using fresh
disposable HOME/data directories and the real archive installer:

| Gate | Mac ARM64 | Ubuntu ARM64 |
| --- | --- | --- |
| Offline locked native release build / real installer | exit 0 / exit 0 | exit 0 / exit 0 |
| Early delta, real tool, second turn, CLI + daemon restart | 4 provider requests, exit 0 | 4 provider requests, exit 0 |
| In-app masked auth / non-default model / restart | 2 provider requests, exit 0 | 2 provider requests, exit 0 |
| Auth controls / raw PTY restoration / packaging | 8 / 1 / 10 passed | 8 / 1 / 10 passed |
| Daemon flows / bridge + lifecycle / server stream | 4 / 73 + 10 / 2 passed | Mac-focused regressions |
| Adjacent installed native library / ten ABI exports | installed journey passed | normalized loader closure + ten exports passed |

The real Playwright browser used the same Mac installed binary at `de7e05f`:
real write/continuation, second turn, authenticated fresh-document reopen,
daemon restart and resumed third turn all passed. It recorded exactly four
provider requests, seven unique durable messages, one tool message, exact marker
bytes and ordered typed call/output in the restarted request. The owned fixture
exited 0, and its captures were preserved outside the repository. Web regressions
passed **49 Vitest tests + 11 Node assertions**.

Exact commands, return codes, source/test/artifact/log hashes and actual results
are retained under the approved artifact parent
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/`:

- `v2-native-integrated-de7e05f-slroa5md/`: Mac build/install, all focused gates
  and `web-packaging-commands.json`.
- `v2-ubuntu-arm64-release-dqhrz_mn/ubuntu-native-de7e05f/`: Ubuntu build/install,
  source archive attestation, installed journeys, loader/ABI evidence.
- `v2-browser-de7e05f/`: actual browser requests, durable messages, snapshots,
  capture manifest and successful owned fixture exit.
- `native-preverify-ed727a6-bsGo0L/`: independent candidate's fresh canonical
  stream and provider runs.

Independent verifier `ses_f0b98d797ffeWTf7GuNITBHwlW` inspected the exact
integrated source and actual paired-release/Web evidence and confirmed
**ACCEPTED on `de7e05f` for this scope**. Full release remains open: explicit
interruption recovery, second-client ownership/concurrent admission, complete
UTF-8/escape decoding, resize/mouse, interactive permissions, per-session file
rooting, non-OpenAI/OAuth, historical disposition and workspace gates.
