# BRIDGE-GAP-02 — native frame assembler

Claim: BRIDGE-GAP-02, session ses_gap02, owned file crates/opentui-bridge/src/native_frame.rs.
Status: implemented, tests green standalone.

## Source evidence
- crates/cli/src/tui_entry.rs:452-542 `native_page_lines` Chat branch: title/status/divider, body tail via `height-7`, empty hint `last:`/placeholder, pad to `height-3`, divider, `> {draft}`, hint line, truncate to height, char-count clip to width.
- crates/cli/src/tui_entry.rs:662-666 `MAX_NATIVE_TRANSCRIPT=500` drain cap.
- crates/cli/src/tui_entry.rs:546-562 `paint_native` consumes lines only (target caller).
- Style model: crates/opentui-bridge/src/logo_art.rs (`#![forbid(unsafe_code)]`, in-file tests).

## Target boundary
- `pub const MAX_NATIVE_TRANSCRIPT=500`, `OFFLINE_TITLE`, `EMPTY_HINT`, `DRAFT_HINT`.
- `pub fn capped_transcript(&[String]) -> &[String]` (tail slice).
- `pub fn clip_line(&str, usize) -> String` (char-count, unicode-safe).
- `pub fn assemble_frame(transcript, draft, title: Option<&str>, status_bar, cols, rows) -> Vec<String>`.
- Simplifications vs inline: title/status passed in (snapshot/model text stays in cli); `None` title renders offline banner. Empty transcript renders `EMPTY_HINT` (the `last:` live-text variant stays caller-side). Char clipping only, no width tables (GAP-03).
- Fail-closed: cols==0 || rows==0 -> empty vec. Else clamp width>=20, height>=8 (mirrors inline).

## Tests (frozen, in-file #[cfg(test)], RED-then-GREEN)
1. cap_enforced_at_500 — 600 lines -> 500, first line-100.
2. cap_visible_in_frame — oldest visible line is line-100 at 80x520.
3. empty_transcript_shows_hint.
4. zero_size_fail_closed — (0,24),(80,0),(0,0) all empty.
5. draft_echo_included — `> hello` present.
6. offline_banner_without_title — None -> OFFLINE_TITLE; Some passes through.
7. unicode_clip_is_char_safe — `héllo🍕world` clipped to 6 chars intact.

## Verification
- `rustc --edition 2021 --test crates/opentui-bridge/src/native_frame.rs -o /tmp/native_frame_test && /tmp/native_frame_test`: 7 passed, 0 failed.
- `cargo test -p opencode-rk-opentui-bridge --lib native_frame`: 0 matched (expected — lib.rs not wired; scope forbids editing lib.rs; orchestrator must add `pub mod native_frame;` then rerun).
- Pre-existing crate warnings only (4, unrelated).

## Remaining
- Orchestrator: add `pub mod native_frame;` to crates/opentui-bridge/src/lib.rs, rerun crate tests, wire `tui_entry paint_native` caller.
