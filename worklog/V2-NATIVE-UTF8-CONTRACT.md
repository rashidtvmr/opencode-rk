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
reads `input.plainText` (1369-1383), and `onSubmit` calls `submit()` after the
normal defer (1391-1395); `submitInner` forwards the current prompt through
the session submission path at lines 947-1000. Ordinary Unicode remains
Unicode input.

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
split multibyte writes, one-DEL removal of one completed single-scalar emoji,
and exact durable/restarted provider input/history. Semantic mismatches are
recorded after retaining the actual JSON request and returning valid bounded
SSE, so wrong native decoding is an observed provider-history mismatch rather
than an HTTP 500 timeout. Lifecycle checks include source/full build hashes,
installed copies, 100-byte socket bound, credential non-echo, true stream mode,
bounded capture/request/response, owned CLI-then-daemon cleanup, both CLI and
validated daemon exit checks, provider thread join, per-PTY raw-mode and
original-attribute restoration, and one final result-or-failure evidence file.
Failure cannot write a success artifact; parent must inspect and run the
installed test to establish RED.

## Ownership and handoff

The fixture owns only disposable files, PTYs, launched processes, provider
thread, and its artifact directory. It imports existing
`native_provider_setup.py` helpers and corrected
`native_stream_tool.py::reap_cli_then_daemon`; neither is edited. No host DB,
credentials, Docker, Node runtime, product code, config, lockfile, or existing
test was changed. Its pure-state self-check exercises valid SSE, exact Unicode
state, retained mismatch, request cap, and auth negative without live PTY or
provider threads. Parent owns semantic review, self-check, real installed RED,
test-hash freezing, and independent integration.

The controller's final source review replaces overwrite-prone aggregate terminal
and CLI flags with per-PTY restoration checks and actual CLI exit-code lists.
Failure on the first semantic mismatch can still truthfully prove its owned
cleanup; success requires both restored PTYs, both zero exits, both raw-mode
observations and exactly two real requests. Lifecycle failure always writes a
failure artifact. The pure-state self-check now actually attempts and rejects
the third request before claiming a request-cap control.

Readiness uses the unchanged G2 helper catalogue probe, which requires both
`gpt-5.6` and `gpt-5.6-mini`; the isolated cache therefore advertises both.
Native prompt readiness checks this contract's selected `gpt-5.6`, rather than
the G2 screen helper's different `gpt-5.6-mini` constant.

## Executable RED and frozen contract

Final test-owner tip: `cc532734033d7d9f26c7a50536b83d1d73138058`, preserving
both earlier fixture attempts. The controller ran this reviewed fixture against
the actual installed native release at exact integrated
**`f6e2d3b05c21f97c7224a333f177c60dfe210c0d`**. It returned 1 after a genuine
completed streamed response and recorded one actual provider request with
`native cafÃ© ...` instead of **`native café 日本語 😀`**. The temporary emoji
was only partially removed. This is semantic RED, with no compiler or readiness
failure: raw input was observed, the CLI exited zero, original termios was
restored, the validated daemon was gone and the provider thread was joined.

Frozen fixture SHA-256:
**`f5a4058cc51d6d84e02523633a2601e1bfa741fe8834848353606939a7f80619`**.
The pure-state self-check also passes. Exact command, whitelisted environment,
source/test/artifact/log hashes, actual provider request and cleanup flags are at
`v2-native-utf8-red-f6e2d3b-o_bqky0w` under
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/`.

Frozen gate command (absolute fixture and attested installed artifacts):

```text
python3 tests/e2e/native_utf8_input.py --binary <install/bin/oc2> --native-library <install/lib/libopentui.dylib-or-so> --build-json <build.json> --artifact-dir <owned-evidence-dir>
```

Only valid UTF-8 scalar assembly and one completed-scalar backspace are leased
for product repair. Malformed input, full escape decoding, grapheme editing,
resize and IME remain separate scope. This contract is **FROZEN RED**, not an
accepted implementation; acceptance requires the installed exact integrated
product to complete both requests, durable restart and owned cleanup.
