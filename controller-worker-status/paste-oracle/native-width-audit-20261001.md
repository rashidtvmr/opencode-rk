# Native paste width audit — mechanical review

Package: G5 native PTY bracketed-paste contract
Status: CANDIDATE / MECHANICAL REVIEW READY; runtime RED and freeze pending

## Git state

- Actual clean HEAD confirmed before edits: `aeeb9729b9337e9eb070115f4eac39e8ee9b28bf`.
- Requested latest candidate/base evidence: `d0376c80c944c8a802f8ad99dc02845276e2ea97`.
- This worker changed only the granted test and worklog paths, plus this worker-owned receipt.
- No product code, provider helper, fixtures, central ledger/state, worktree/ref, or history was deleted or modified.

## Immutable source evidence

Read-only linked checkout `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/opentui-native-pin-c01292fd`, commit `c01292fd0837bafd07ce458c74416b2b375a41ab`.

`packages/native/src/utf8.zig`:
- `WidthMethod`: `unicode = 1`, `unicode_wide = 3`.
- `eawToWidth`: returns 2 only for `.fullwidth` or `.wide`; ordinary fallback returns 1.
- `GraphemeWidthState.addCodepoint`: only `unicode_wide` promotes an otherwise non-zero grapheme to 2.
Thus this ordinary unicode/xterm fixture requires W/F=2 and A=1; `WFA` was incorrect.

## Change and preservation

`PasteTerminalScreen._width` now uses `unicodedata.east_asian_width(char) in "WF"`. Existing exact provider/history assertions, combining handling, real spaces, erase behavior, 32 KiB limit, provider texts, negative contracts, cleanup, and the frozen native provider setup hash `32edfa5c0ac5eacef9cfa949e02fce0d3440fe1cd8f75580efb584bc0a7780dc` are preserved.

Captured replay artifacts were only inspected as evidence: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-today-control-y52o0gi3/paste-visible-debug-4a28575/0-capture.bin` and `snapshots.json`. No live PTY or replay was run.

## Verification

- `/usr/bin/git diff --check`: PASS.
- No Cargo/build/PTY/runtime/network command run by worker.
- Source hashes after edit:
  - `tests/e2e/native_bracketed_paste.py`: `6b3c91db2240103c2aa3c13cd2b12831b6a3235891a553f5cd1d0f7d5a134526`
  - `worklog/V2-NATIVE-PASTE-CONTRACT.md`: `c53570c7ca2568ff13c9fabad32287539fe5e6a4c4d5235e658a4fd0932e78c6`

The trusted integration writer must commit this normal clean successor and rerun the frozen verifier. Worker status is CANDIDATE / MECHANICAL REVIEW READY, not ACCEPTED.
