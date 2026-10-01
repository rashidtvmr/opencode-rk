# V2 Native Bracketed-Paste Contract (RED fixture, test-owner only)

**Base:** `75ad7b5b681c9bcdb4d37f6f84f3f410e87dabda`
**Lane paths (only):** `tests/e2e/native_bracketed_paste.py`, this doc.
**Pin:** upstream `95daf90670b7c039c436c85537da5fbfe2205b41`.
**Status:** SOURCE CANDIDATE, unexecuted by author (parent runs RED + freezes). AST + diff-check only.

## Gap (audit REPORT.md verbatim disposition)

`TerminalInputDecoder` CSI `200~` consumed but paste-body CR/LF may submit;
`native_loop` never enables bracketed-paste, emits no `Paste` event, has no
`Composer::apply_paste` caller. Relevant current symbols:

- `crates/cli/src/tui_entry.rs` `native_loop` 562-571, 740-909 (no 2004h, no paste state)
- `crates/cli/src/native_input_decoder.rs` 75-259 (framing only, no paste event)
- `crates/cli/src/native_composer.rs` 438-568 (`apply_paste` max 32 KiB, CRLF normalize, inert draft; unwired)
- `crates/cli/src/terminal_host.rs` 95-105, 304-317 (bounded `Paste`/`admit_paste`, unwired)

## Upstream oracle (verified read-only; grandchild Luna ses_f095ec43affeKlPckURvMdVRBi completed)

- `packages/tui/src/component/prompt/index.tsx:1396-1420` `onPaste`:
  `decodePasteBytes(event.bytes).replace(/\r\n/g,"\n").replace(/\r/g,"\n")`;
  empty paste dispatches `prompt.paste`; non-empty `event.preventDefault()` then
  `pasteInputText`; no submit in handler.
- `index.tsx:1183-1221` `pasteInputText`: trim, filepath/URL attach routing,
  `lineCount >= 3 || length > 150` paste-summary insert, else
  `input.insertText(normalizedText)` + dirty/render; non-submitting.
- `app.tsx:191-203`: `useKittyKeyboard: {}`, `useMouse` flag-gated.
- OpenTUI solid `packages/solid/src/elements/hooks.ts:91-100` + react
  `packages/react/src/hooks/use-paste.ts:7-24`: paste is a distinct `paste`
  event on the key handler, not ordinary key bytes.

## Minimum parity slice for product lane (not this lane)

Bounded text paste + CRLF/CR->LF normalize + inert insertion + explicit-Enter
submit only. Out of scope: summary/files/images, mouse, permission UI, old
transport reintroduction. Composer budget 32 KiB (`MAX_PASTE_BYTES` /
`MAX_DRAFT_BYTES`); no auto tool/provider from contents; negotiation must
observe real `ESC[?2004h`/`ESC[?2004l` on the wire, not parser-only.

## Fixture contract (frozen intent)

`tests/e2e/native_bracketed_paste.py --binary ... --native-library ...
--build-json ... --artifact-dir ...` (`--self-check` for pure logic).

- Real TTY + attested native release (`checked_artifact`, schema, 64-hex
  descriptor token) + fresh HOME/XDG/data/project + fixture provider/model.
- Own bounded provider `State` (2-request cap, exact history, auth, SSE);
  frozen `native_escape_input` FIRST constants untouched.
- Fragmented `ESC[200~` + UTF-8/CRLF/CR body + fragmented `ESC[201~`:
  zero provider requests, zero durable user messages, draft visibly holds
  normalized text; quit-like/slash text proves inertness.
- Explicit Enter -> exactly one request with upstream-normalized multiline
  prompt; second plain turn; durable history exactly
  user/assistant/user/assistant.
- Negatives: unterminated paste leaves sentinel draft + history untouched,
  no request; oversized framed paste (>32 KiB) rejected without draft
  mutation, request, or durable change.
- Caps: 128 KiB req / 256 KiB resp / 256 KiB PTY / 512 KiB evidence.
- Own controlling PTY `TIOCSCTTY` via fresh child exec; no thread preexec.
- Normal exit via Ctrl-C, termios restore, owned daemon `stop_owned_daemon` +
  PID-absent probe, provider join; forced kill recorded as failure. Forensic
  root preserved and printed on both outcomes. Fake key never echoed.

## Expected RED (current product)

No `ESC[?2004h` in PTY bytes -> fixture raises before Enter. Even past that,
bare-CR bytes hit the submit path early. No product behavior claimed.
