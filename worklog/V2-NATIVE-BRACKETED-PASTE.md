# G5 native bracketed paste — candidate repair

**CANDIDATE DONE; exact main-v2 acceptance PENDING.**

## Package and ownership

- Package/gate: `G5-NATIVE-BRACKETED-PASTE`.
- Base SHA: `7628b61d10625ea35aa09392394c7b689b20d55d`.
- Candidate branch: `v2/native-paste-repair-y52o0gi3`.
- Worktree: `/Users/mymac/Projects/opencode-rk-v2-paste-repair-y52o0gi3`.
- Candidate SHA: the commit containing this receipt; the final leaf handoff and
  the worker's external `worker-status/native-paste/candidate.json` report its
  full SHA after commit creation.
- Declared active harness model: **GPT-6.1 Sol**, `openai/gpt-6.1-sol`; no model
  override or child agent. Session: `ses_f0938d3c9ffewCKC1SQwVJlVVY`.
- Parent owns independent heavy validation and canonical integration.

Exact changed paths:

```text
crates/cli/src/native_input_decoder.rs
crates/cli/src/tui_entry.rs
crates/opentui-bridge/src/safe_renderer.rs
worklog/V2-NATIVE-BRACKETED-PASTE.md
```

## Frozen runtime RED

The supplied freeze is
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-today-control-y52o0gi3/native-paste-freeze.json`
(SHA-256 `be0eb4c66815c5b23fbbf8b7ae517381cae0701e8f6fd26954aed9d37fb32da9`).

The independent installed release at the exact base submitted
`paste contract alpha café` without Enter, rather than the complete normalized
draft `paste contract alpha café\n日本語 line 🦀\n:quit /connect stays inert`.
The actual native setup emitted `ESC[?2004h`; normal exit did not emit
`ESC[?2004l`. Raw mode, controlling TTY, exact termios restoration, normal
exit 0, daemon absence, provider join and no secret echo passed in that RED.

Protected contract hashes, rechecked without executing either fixture:

| Path | SHA-256 |
| --- | --- |
| `tests/e2e/native_bracketed_paste.py` | `3bc110bb17135d683afba833fda4defacd8811111ad1ba85ce70e0392338532e` |
| `tests/e2e/native_provider_setup.py` | `32edfa5c0ac5eacef9cfa949e02fce0d3440fe1cd8f75580efb584bc0a7780dc` |

The frozen parent receipt is `paste-integrated-red/receipt.json` beneath the
same external control root. It records exit **1**, a 4.26-second runtime,
binary SHA-256 `c67dc35274e172d80d018a23958bad2ed175aec02bd080528a053fe6766b42f6`,
and linked native library SHA-256
`798f30dd7f4fbe36d52c8834652ed7bcd7f20dfd2a1203d09cc24880eeb13a91`.
No protected test, helper, freeze, budget, Cargo manifest or lockfile was edited.

## Pinned evidence inspected

1. **Native V2 product behavior:** the full read-only OpenCode checkout at
   `/Users/mymac/Projects/opencode-upstream-reference` is exactly
   `95daf90670b7c039c436c85537da5fbfe2205b41` (`git rev-parse HEAD`, exit 0).
   The full `packages/tui/src/component/prompt/index.tsx` was read:
   - `Prompt` / textarea `onPaste`, lines 1396–1420, normalizes CRLF first and
     remaining CR second, prevents default paste and inserts through
     `pasteInputText`, without submitting;
   - `pasteInputText`, lines 1183–1221, inserts normalized text and requests
     redraw;
   - `submit` / `submitInner`, lines 930–1146, separates explicit submission
     from insertion and uses complete prompt text for the provider request.
2. **Input protocol shape:** exact OpenTUI 0.4.5 object
   `0c8c4f7cff2927e3df63a9757a45eff9a343611c` in
   `/Users/mymac/Projects/opentui`, inspected with `git show`, rather than HEAD:
   `packages/core/src/lib/stdin-parser.ts`, `StdinEvent`,
   `BRACKETED_PASTE_START` / `BRACKETED_PASTE_END`, `StdinParser.push`,
   `scanPending` and `consumePasteBytes` keep paste separate from key events
   and recognize closing markers across chunk boundaries.
   `packages/core/src/lib/paste.ts`, `decodePasteBytes`, decodes a complete body.
3. **Actual linked FFI/cleanup authority:** checkout
   `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/opentui-native-pin-c01292fd`
   is exactly `c01292fd0837bafd07ce458c74416b2b375a41ab` (exit 0).
   - `packages/native/src/lib.zig`: `destroyRenderer(handle, flush_input)` at
     1227–1233 has two arguments; `restoreTerminalModes` at 2129–2132,
     `setupTerminal` at 2174–2177 and `suspendRenderer` at 2179–2182 have their
     existing linked signatures.
   - `renderer.zig`: `setupTerminalWithoutDetection` at 519–537 enables detected
     features; `suspendRenderer` at 539–542 calls `performShutdownSequence`
     (561–615); `destroy` at 464–498 preserves cleanup-before-backend-destruction.
   - `terminal.zig`: `enableDetectedFeatures` at 453–495 enables bracketed paste;
     `resetState` at 285–338 disables it at 303–305;
     `restoreTerminalModes` at 1089–1145 is a focus-in **re-enable**, not exit
     restoration. This explains the incorrect cleanup API choice.
4. **Read-only local bounds precedent:** `native_composer.rs`, `MAX_PASTE_BYTES`,
   `MAX_DRAFT_BYTES`, `Composer::apply_paste`, and `terminal_host.rs`,
   `MAX_PASTE_BYTES`, `admit_paste`, require atomic 32 KiB paste/draft bounds.
   These historical helpers remain evidence; the repair uses the live raw-input
   loop. The frozen current requirement authorizes the bounded/deadline policy,
   rather than claiming it is OpenTUI's unbounded collector behavior.

## Implemented behavior

- The decoder emits `InputEvent::Paste(String)` only after the exact closing
  marker, valid complete UTF-8 and CRLF/CR-to-LF normalization. Body bytes,
  including CR, LF, Ctrl-C/Ctrl-D, Escape, Kitty-looking sequences, quit and slash
  text, bypass keyboard dispatch.
- One body retains at most **32 KiB**, plus at most five closing-marker prefix
  bytes. The existing event ring remains capped at eight events; every paste
  event is byte-bounded. Invalid UTF-8 rejects the complete frame. Overflow
  releases the body and consumes everything through the exact closing marker,
  including controls, without a partial insertion or escape recovery.
- An incomplete body has **150 ms idle grace** and a **five-second absolute
  frame deadline**. Expiration releases its body and keeps consuming late frame
  bytes. A fresh standalone Escape after abandonment retains the existing
  explicit draft/dialog-clear action after its own quiet 150 ms grace. Discard
  states have no continuously expired timer, avoiding no-input busy loops.
- The live composer appends a completed paste atomically only if the combined
  draft fits 32 KiB. The existing API-key dialog's 16 KiB bound and masking are
  retained. Paste itself has no submit, dialog action, provider request, queue or
  persistence operation. Explicit submission preserves the complete normalized
  draft, including surrounding whitespace; command/dialog matching still trims.
- Multiline drafts occupy separate bounded native rows. `native_paint` clips
  row text to the renderer/1024-character bound and renders control scalars as
  spaces without changing the stored/provider text. Raw input controls never
  cross the drawing boundary. Model selection, auth flow, raw reads, four-byte
  ordinary UTF-8 carry and `TurnWorker` dispatch are retained. Worker polling
  stays at 20 ms and idle/resize waits are capped at 100 ms.
- Owned terminal restoration and Drop use the already-declared
  `suspendRenderer` shutdown export instead of the focus-in re-enable export.
  Cleanup remains scoped to the live renderer's terminal flag and native output
  backend, followed by exact captured input-attribute restoration and the
  existing two-argument, exactly-once destroy. Memory/headless rendering gains
  no direct terminal escape output or new FFI declaration.
- Native non-Unix `input_ready` / `read_input` return a typed terminal failure;
  platform guards prevent calls into Unix-only raw descriptor APIs. This is a
  compile-boundary repair, not a claim of non-Unix interactive input support.

## Source-only verification

These final source checks ran in the repair worktree:

```text
rustfmt --edition 2021 --check --config skip_children=true crates/cli/src/native_input_decoder.rs crates/cli/src/tui_entry.rs crates/opentui-bridge/src/safe_renderer.rs
  exit 0
git diff --check
  exit 0
shasum -a 256 tests/e2e/native_bracketed_paste.py tests/e2e/native_provider_setup.py /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-today-control-y52o0gi3/native-paste-freeze.json
  exit 0; all three hashes match the values above
```

No compilation, Cargo, native execution, PTY, test, container or network command
was run by this leaf. Formatting parses syntax; it does not establish GREEN.

## Parent-owned verification and remaining status

The frozen installed-gate argv, with `RELEASE_ROOT` set to the parent's fresh
attested native release of the exact candidate and later exact integrated SHA,
is:

```sh
/usr/bin/arch -arm64 /usr/bin/python3 /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3/tests/e2e/native_bracketed_paste.py --binary "$RELEASE_ROOT/install/bin/oc2" --native-library "$RELEASE_ROOT/install/lib/libopentui.dylib" --build-json "$RELEASE_ROOT/build.json" --artifact-dir "$EVIDENCE_ROOT/fixture"
```

The RED receipt's environment was `PATH=/usr/bin:/bin`,
`TMPDIR=/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/pp`,
`PYTHONDONTWRITEBYTECODE=1`; its build JSON, binary and library were independently
attested before the fixture ran. The independent parent must preserve its
trusted runner limits, recheck frozen hashes, build/install, run the identical
frozen positive/negative gate and its independent negative probe, then run the
required native regressions, including:

```text
cargo test --offline --locked -p opencode-rk-cli --features native --bin oc2 -- --test-threads=1
cargo test --offline --locked -p opencode-rk-opentui-bridge --features native --lib --test native_terminal_lifecycle -- --test-threads=1
```

The bridge lifecycle command requires the parent's attested library directory in
`TUI015_NATIVE_LIB_DIR` and the platform loader environment. These commands are
handoff requirements, not leaf-run results.

The last observed product failure remains the frozen base's early truncated
submission and missing paste-mode reset. Candidate runtime behavior and
platform compilation are **unverified** until the parent supplies receipts.
Status is **CANDIDATE only**, neither PREVERIFIED nor ACCEPTED; this package
does not certify full G5/G8 or broader TUI parity.

**CANDIDATE DONE; exact main-v2 acceptance PENDING.**
