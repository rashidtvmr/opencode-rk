# V2 Native UTF-8 Live Candidate

## Package

- Package/gate: G5 Native TUI — native interactive UTF-8 input repair
- Base: `47549f06f9c5c39cd2a54f81ad6ae4866ed89596`
- Frozen contract: `tests/e2e/native_utf8_input.py`, SHA-256
  `f5a4058cc51d6d84e02523633a2601e1bfa741fe8834848353606939a7f80619`
- Status: candidate; independent verification and integration are required.

## Change

`crates/cli/src/tui_entry.rs` now assembles terminal input with a fixed four-byte
UTF-8 carry. Complete valid scalars are appended as `char`s only after bounded
validation. ASCII remains on the same draft/dialog/submission path. Backspace
pops one completed scalar, and submit, dialog switching, clearing, and other
control actions discard incomplete carry. Draft byte limits remain 32 KiB for
ordinary input and 16 KiB for API-key input; the append check is overflow-safe.

No queue, task, thread, runtime, dependency, input descriptor, masking, or
terminal cleanup behavior was changed. Invalid or incomplete sequences are
discarded without unsafe or lossy conversion.

## Upstream evidence

The pinned checkout at `95daf90670b7c039c436c85537da5fbfe2205b41` was inspected.
The native V2 source at
`packages/tui/src/component/prompt/index.tsx` implements the corresponding
complete-text behavior: `onContentChange` at lines 1377-1382 reads
`input.plainText`, stores it, forwards it to autocomplete, synchronizes
extmarks, and updates the cursor version. `submitInner` at lines 947-955 reads
and synchronizes `input.plainText` before downstream reads, explicitly covering
the final composed character. The downstream session submission text part at
lines 1104-1106 passes `inputText` as the text payload. This is classified as
implemented native V2 behavior and the Rust bridge preserves its complete-text
observable semantics.

The shared/legacy `packages/opencode/src/cli/cmd/run/footer.view.tsx` and
`footer.prompt.tsx` path was also inspected as corroborating evidence, but is
not the basis for the native V2 classification.

## Verification boundary

Source-only work was performed. No Cargo build, test, clippy, browser, PTY,
archive, or install command was run by this implementation worker. The parent
independent verifier must run the frozen absolute fixture and the required
native/G2/auth/PTY/CLI regressions on this exact candidate commit.

## Exact integrated acceptance

Original candidate `cfe6cbfdc96dbe6d3cf76b284da7e614e40c5a9a`, based on
`47549f06f9c5c39cd2a54f81ad6ae4866ed89596`, passed its actual installed UTF-8,
stream, provider, auth and PTY checks, then exposed two pre-existing transcript
paging failures in the broader CLI unit gate. Those failures reproduce on the
canonical baseline and were independently repaired/accepted as `7c39fee`; the
frozen tests remain unchanged. The UTF-8 source was prepared byte-identically
against repaired canonical base `3086fc1`, independently PREVERIFIED, and
fast-forward integrated as exact **`a79ae96b93f9a0ab94ceafadcd9425db08f28027`**.
Both original candidate histories and failed receipts remain preserved.

The controller rebuilt, archived and actually installed exact integrated
`a79ae96` on Mac and Ubuntu ARM64, then repeated the frozen gates:

| Gate | Mac ARM64 | Ubuntu ARM64 |
| --- | --- | --- |
| Offline locked native release build / archive installer | exit 0 / exit 0 | exit 0 / exit 0 |
| Fragmented 2/3/4-byte input, completed-scalar backspace, durable restart | 2 exact requests; exit 0 | 2 exact requests; exit 0 |
| Early stream, real write/tool continuation, second turn, restart | 4 requests; exit 0 | 4 requests; exit 0 |
| Provider onboarding / auth controls / PTY restoration | 2 requests / 8 / 1 passed | 2 requests / 8 / 1 passed |
| Daemon flow / CLI unit / bridge + lifecycle / server stream | 4 / 314 / 73 + 10 / 2 passed | Mac-focused regressions |
| Packaging / adjacent native loader / ten ABI exports | actual installed path passed | 10 / normalized loader / ten exports passed |

Both Unicode receipts confirm exact original user content
`native café 日本語 😀`, ordered restarted history, two raw-mode observations,
two original-termios restoration checks, two zero CLI exits, validated daemon
PIDs absent and provider thread joined. Fixture hash remains
`f5a4058cc51d6d84e02523633a2601e1bfa741fe8834848353606939a7f80619`.
All earlier frozen native/credential/PTY contracts remain byte-identical.

Exact commands, whitelisted environments, source/test/artifact/log hashes and
actual results are retained under the approved artifact parent
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/`:

- Candidate: `v2-native-utf8-ready-preverify-a79ae96-ub_e1jtw`.
- Exact integrated Mac: `v2-native-utf8-integrated-a79ae96-a8ems97q`.
- Exact integrated Ubuntu:
  `v2-ubuntu-arm64-release-dqhrz_mn/ubuntu-native-utf8-a79ae96`.

Mac binary/library/archive SHA-256:
`e41e7640a70b316def03c1b143e699e1fefe853abdcfeef90fbc52e60b85731e`,
`798f30dd7f4fbe36d52c8834652ed7bcd7f20dfd2a1203d09cc24880eeb13a91`,
`f4a1c2d687305e26b05802e28f8e122982faafd3249c6a9ef2dacd07c538d384`.
Ubuntu binary/library/archive SHA-256:
`c8ff7a6314bc91629ce281f3c662198a154c150f705f3cd35f69a05f951273e7`,
`e85a45710e9e181b3eb7cca877a1d9022f2210bfa1e06b7c159e734506da3b89`,
`f96f878910e7ba3656e7d4e487ca6f147ca168c96c41ab7f9f20e32a12b3d630`.

Independent verifier `ses_f0b3f6f38ffe7tuTisQyuMpbX9` reviewed actual exact
integrated paired-platform evidence and confirmed **ACCEPTED for native valid
UTF-8 scalar input/restart and required regressions on exact `a79ae96`**.
Full release, a new same-SHA Web journey, interruption, concurrent/second-client
admission, full escape/resize/mouse, grapheme/IME, rooting, OAuth, compaction,
historical disposition and workspace-wide gates remain open.
