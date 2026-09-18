# TUI-layout (layout types slice of TUI-003)

## Claim
`crates/cli/src/native_layout.rs` owns pure layout geometry: Rect/Size,
compact flag (<80x24), sidebar width, zero-sidebar on tiny, has_overlap.

## Source evidence
- Commit 5af7884.
- `crates/cli/src/native_app.rs:120-244`: Rect/overlaps/ShellLayout/compute/has_overlap semantics (sibling slice owns app state; layout extracted here per TUI-003 path split).
- Card TUI-003 via `python3 tools/completion_plan.py --card TUI-003` (tests T01..T05; this file covers T03 geometry: tiny/resized no-overlap).
- Boundary `docs/architecture/COMPLETION_NATIVE_TUI.md:24-32` (domain crates keep `forbid(unsafe_code)`, bounded/coalesced).

## Observed scenario
- File absent before task. Created `crates/cli/src/native_layout.rs` only. No other edits.

## Target boundary
- Owned file only: `crates/cli/src/native_layout.rs`. `#![forbid(unsafe_code)]`, std only, no IO/clock/threads.
- Exports: `Size`, `Rect` (+area/overlaps), `ShellLayout` (+compute/has_overlap), `sidebar_width`, consts MIN_FULL_WIDTH/MIN_FULL_HEIGHT/SIDEBAR_WIDTH/SIDEBAR_MIN_WIDTH.

## Tests
- RED rev hash 32170e57da3ba6f742aef942538c2f9309b98f4c8dd94f2e3dd3e0086b48755e: 1 passed, 2 failed (tiny compact, overlap detect) — failing for missing behavior.
- Frozen tests (unchanged into GREEN): tiny->compact+zero-sidebar, normal no overlap, overlap detected.
- GREEN cmd: `rustc --edition 2021 --test crates/cli/src/native_layout.rs -o /tmp/opencode/nl && /tmp/opencode/nl` → 3 passed, 0 failed.
- GREEN hash c27947b0cb0e670e5788cf8988afd6249152a12061a4de77842aba68b8c8432e, 200 lines.
- Note: `sidebar_width` is `pub fn` not `pub const fn` (rustc const-trait gate on `u16::min`); compute stays non-const matching sibling.

## Decisions
- Compact = width<80 OR height<24. Compact/hidden-sidebar → zero-area sidebar, transcript full width.
- Full: sidebar `min(sidebar_width(width), width)`, transcript remainder; composer 5h/status 1h (compact 3h).
- overlaps(): zero-area never overlaps; edge-touch OK.
- has_overlap(): pairwise over 4 regions skipping zero-area.

## Remaining unknowns
- None for geometry. Wiring into renderer/focus-visibility belongs to native_app/renderer slices, not this file.
