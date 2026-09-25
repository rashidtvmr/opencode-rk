# BRIDGE-GAP-03

## Claim
- Task: `BRIDGE-GAP-03`
- Session: `ses_gap03`
- Owned file: `crates/opentui-bridge/src/unicode_width.rs` (new)
- Scratchpad: `worklog/BRIDGE-GAP-03.md`

## Source evidence
- Current revision: `d460eb965b200e1454941db5be200b548746dea9`.
- TS reference: `/home/rashid/projects/opencode`, commit `a0d9b6c7014dab2d6e94819d4ad1d3d2a2e4f737`, `packages/tui/src/prompt/display.ts:1-9` uses `Intl.Segmenter` and `Bun.stringWidth`; line 6-7 make newline one prompt offset and other segments use Bun width. Tests at `packages/tui/test/prompt/display.test.ts:12-18` cover CJK, emoji family, and display offsets.
- Target bridge currently has no `unicode_width.rs`; `lib.rs` is outside this lane and is not edited.
- Existing bridge width fallbacks are documented as char-count in `crates/opentui-bridge/src/safe_renderer.rs:567-579`, `text.rs:30-36`, and `error_component.rs:42-44`.

## Contract
- `char_width`: ASCII 1; C0/C1 0; specified combining ranges 0; specified W/E ranges 2; 1F300-1FAFF 2; 2600-27BF only with VS16 2; ambiguous Latin 1; other BMP/scalar values 1 by default.
- `line_width`: sum display columns, no panic for any `&str`, saturating arithmetic at `usize::MAX`.
- `clip_to_width`: return longest safe prefix whose display width is at most max, plus consumed display width. Do not split a base character from following zero-width combining/VS16/ZWJ content. Never exceed max.
- Document intentional divergence from Bun/Intl tables in module docs.

## Tests
Required in-file tests: ASCII, CJK double width, combining zero, emoji, Tamil base+mark, clip boundary, empty. Add boundary/control and max-width checks.

## RED evidence
- Frozen test file SHA-256 before implementation: `ae79f858d1c2b49a012e1069e2da868cce0f12cb5ea93a929ecf76d74ecb3860`.
- Command: `rtk rustc --edition=2021 --test crates/opentui-bridge/src/unicode_width.rs -o /home/rashid/.cache/bun-tmp/opencode/bridge-gap03-red && rtk proxy /home/rashid/.cache/bun-tmp/opencode/bridge-gap03-red --nocapture`.
- Result: expected RED, 1 passed, 7 failed; failures were the missing width and clipping behavior, not compile errors.

## Decisions
- Standard library only. Scalar scanner with small range predicates; no dependency, IO, unsafe, or global state.
- Treat variation selectors, ZWJ, and other explicit joiners as zero-width continuation where they do not independently render columns.
- Preserve combining marks in clipped output when their base was retained.

## Remaining unknowns
- Exact hidden acceptance may expect family emoji clusters to consume two columns. Scanner will keep ZWJ/VS16 continuations with the preceding scalar and count each emoji scalar as requested; document this simplification rather than claiming full UAX #29 parity.
- Integration wiring is intentionally deferred because `lib.rs` is outside approved scope.

## Verification
- `rtk timeout 120 rustc --edition=2021 --test crates/opentui-bridge/src/unicode_width.rs -o /home/rashid/.cache/bun-tmp/opencode/bridge-gap03 && rtk timeout 120 /home/rashid/.cache/bun-tmp/opencode/bridge-gap03 --test-threads=1`: 8 passed, 0 failed.
- `rtk rustfmt --check --edition 2021 crates/opentui-bridge/src/unicode_width.rs`: clean.
- `char_width` at line 102, `line_width` at line 119, `clip_to_width` at line 142; eight in-file tests cover all three.
- No test behavior edits; no cargo; no lib.rs change.

