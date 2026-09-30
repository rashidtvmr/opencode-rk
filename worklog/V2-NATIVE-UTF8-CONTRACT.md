# V2 Native UTF-8 Contract — candidate test source

## Package and disposition

* Package: G5 native PTY input, independent TEST OWNER contract preparation.
* Base: `9c8829019b750df6a4cb4e79d6be82bfa83e4dd8`.
* Branch/worktree: `v2/native-utf8-contract`, `/Users/mymac/Projects/opencode-rk-v2-native-utf8-contract`.
* Status: `CANDIDATE` / source-only; no gate-green or acceptance claim.
* Only new files are `tests/e2e/native_utf8_input.py` and this worklog.

## Authority and defect

Pinned upstream is `95daf90670b7c039c436c85537da5fbfe2205b41`, inspected in
`/Users/mymac/Projects/opencode-upstream-reference`. In
`packages/tui/src/component/prompt/index.tsx`, textarea `onContentChange`
reads `input.plainText` (1369-1383), and `onSubmit` submits after the normal
defer (1391-1395). Ordinary Unicode remains Unicode input.

Current native evidence is `crates/cli/src/tui_entry.rs:660-810`: input is read
one byte at a time and line 809 does `draft.push(char::from(b))`; lines
674-676 use `draft.pop()` on the already-misdecoded bytes. This contract
exposes that defect and does not implement a decoder.

## Frozen observable contract

`native_utf8_input.py` requires an installed binary, native library and build
attestation, fresh disposable HOME/data/project, a real raw PTY, and a local
loopback provider. It sends fixed `native café 日本語 😀`, with each UTF-8
sequence fragmented into individual PTY writes. A temporary final emoji is
sent and removed by one DEL before Enter. The provider records exact UTF-8 JSON
and requires the first input string, `stream: true`, and bounded request data.

After a genuine CLI+owned-daemon restart on the same disposable storage, the
second real request must contain the exact original Unicode user and settled
assistant history plus `native utf8 resumed`. Exactly two requests are allowed;
bounded SSE uses ASCII completion markers. No mojibake normalization, fake
client, manually inserted history, or fake success screen is used.

Named checks are accented 2-byte input, CJK 3-byte input, emoji 4-byte input,
split multibyte writes, one-DEL removal of one completed emoji, and exact
durable/restarted provider input/history. Lifecycle checks include source/full
build hashes, installed copies, 100-byte socket bound, no secret echo, true
stream mode, bounded capture/request/response, owned CLI-then-daemon cleanup,
provider thread join, and terminal restoration. Failure cannot write a success
artifact; parent must inspect and run the installed test to establish RED.

## Ownership and handoff

The fixture owns only disposable files, PTYs, launched processes, provider
thread, and its artifact directory. It imports existing
`native_provider_setup.py` helpers and corrected
`native_stream_tool.py::reap_cli_then_daemon`; neither is edited. No host DB,
credentials, Docker, Node runtime, product code, config, lockfile, or existing
test was changed. Parent owns semantic review, self-check, real installed RED,
test-hash freezing, and independent integration.
