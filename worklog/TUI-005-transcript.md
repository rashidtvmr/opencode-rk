# TUI-005 transcript — worklog

## Claim
Owned file `crates/cli/src/native_transcript.rs` implements transcript
types: `TokenDelta`, `ToolState`, `StreamAccumulator`, ANSI/OSC strip,
bounded virtualized window, scrollback-stable marker. No other files touched.

## Source evidence
- Repo HEAD `5af7884` (`git rev-parse HEAD`).
- No `TUI-005` task card found in `tasks/` or `docs/` (grep `TUI-005`
  returned no card); brief taken from lane prompt.
- Existing TUI pattern: `crates/cli/src/tui_entry.rs:1`
  (`#![forbid(unsafe_code)]`, stdio-bound bounded frames, no TTY/render dep).
- Security policy: `docs/SECURITY.md:24-32` (no secret logging, no broad
  fs trust, prompt not a boundary) and `:37-47` (regex not a sandbox).
- No existing `native_transcript`/`TokenDelta`/`ToolState` in `crates/`
  (grep empty before creation) — new module, no upstream behavior to retain.

## Observed scenario
- `rustc 1.96.1`. Created module, compiled with
  `rustc --edition 2021 --test crates/cli/src/native_transcript.rs`.
- First RED-equivalent: C1 CSI test used `\x9b` byte escapes → rustc
  "out of range hex escape" compile error. Fixed test literal to
  `\u{9b}` (test-only change to make the suite compile; assertions frozen
  after that).
- First GREEN run: 6/7 pass, `hostile_escapes_stripped` failed on C1 CSI
  (`\u{9b}` arrives UTF-8 as `C2 9B`, raw-byte check `b == 0x9b` missed it,
  sequence leaked through). Fixed implementation to recognize both raw
  `0x9B` and UTF-8 `C2 9B` for C1 CSI/OSC. Reran: 7/7 pass.

## Target boundary
- `TokenDelta { stream_id, seq, text }`, per-chunk cap `MAX_DELTA_BYTES`.
- `ToolState::{Approval, Running, Failed, Completed}` + `is_terminal` +
  `can_transition` (Approval→Running, Running→Failed/Completed; terminals
  sink).
- `StreamAccumulator`: per-stream out-of-order converge, dup/stale/wrong-stream
  ignored, pending cap `MAX_PENDING_DELTAS`, text cap `MAX_STREAM_BYTES`
  with `dropped_bytes` accounting.
- `strip_ansi`: CSI/OSC/DCS/SOS/PM/APC/charset/Lone-ESC/C1/C0 stripped;
  unterminated OSC swallowed to end (fail closed, hostile hyperlink text
  kept, URL dropped); `\n`/`\t` kept; output cap `MAX_SANITIZED_BYTES`.
- `Transcript`: scrollback cap `MAX_SCROLLBACK` with `dropped` count,
  `set_marker(id)` + `marker_stable()` (id-keyed, survives truncation until
  evicted).
- `window_slice`/`Transcript::window`: offset/limit, hard cap `MAX_WINDOW`.
- `#![forbid(unsafe_code)]`, std only, no deps.

## Tests (frozen in-file `#[cfg(test)]`, 7 tests)
1. `partials_converge_in_order` — ordered deltas assemble.
2. `partials_converge_out_of_order_with_duplicate` — OOO + dup/stale/
   wrong-stream converge.
3. `hostile_hyperlink_neutralized_but_text_kept` — OSC-8 URL gone, text kept.
4. `hostile_escapes_stripped` — CSI/erase/cursor/lone-ESC/BEL/unterminated
   OSC/C1 CSI stripped, `\n`/`\t` kept.
5. `window_bounded_and_clamped` — huge limit capped, OOB/zero empty, tail ok.
6. `transcript_marker_survives_bounded_scrollback` — marker stable under cap,
   flips after eviction, unknown id rejected.
7. `tool_state_terminals_and_transitions` — terminals + transition table.

## Decisions
- Byte-walk sanitizer (no regex dep, std only); note per SECURITY.md a
  regex/prompt is not a sandbox — this is output neutralization only, not
  an isolation boundary.
- Truncation is char-boundary safe everywhere.
- `Transcript::push` re-sanitizes (defense in depth for hostile tool output).

## Remaining unknowns
- No TUI-005 card in repo — contract sourced from lane prompt only.
- Module not yet wired into `main.rs`/`tui_entry.rs` (other lanes' files;
  integration proposal needed).
- No cargo build per lane instruction (rustc --test only).
