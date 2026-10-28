# UI handoff reconciliation — 2026-10-27

Canonical checkout: `/Users/mymac/Projects/opencode-rk-recovery-20261005`.
Selected reference: `/Users/mymac/Projects/tmp/opencode` at
`450e7b92412c780a7b5a75226bb8d49aaca24e22`.
These are research dispositions, not integrated acceptance. All worker usage
is UNKNOWN; returned code was read-only and unexecuted by its author.

## Reviewed changes and blocking findings

- Sidebar heading candidate `e8f4e942b00e5a01e5897f5870cdfe2ace766d6323525320d81ba6acd472df12`
  independently APPROVED by `ses_e820a99faffeKbBRkFHYT3hxwY`; committed as
  `e1e646b0d59c3805d813fc3e269e89560b02c6f5`. Normal sidebar now uses
  Text + bold; generic diagnostic Heading remains Primary + bold.
- Wrapping v3 remains rejected despite one approval. Reviewer
  `ses_e8200a6a2ffez25RceAZTOzblL` established native OpenTUI separator elision
  in `text-buffer-view.zig:1738-1744,1931-1977`, native fixtures at
  `tests/text-buffer-view_test.zig:571-596` and
  `tests/editor-view_test.zig:675-706`. Three failed candidates triggered
  independent strategy review: replace the rendered-concatenation invariant
  with source intervals and explicit hidden-separator affinity.
- Main captured compiling RED (`hello world`, width 5 produced a whitespace-only
  middle row), replaced the model, and froze v4 manifest
  `d43c4e8b44cec4e0dc122dacd99cdda22e365d37defa024dff9cc393e731e849`.
  Dirty-tree 974 native tests and nonnative CLI check PASS; exact v4 reviews
  pending in the same two evaluator sessions. No v3 approval carries forward.
- Strategy handoff's suggested `hi world` cursor at 3 on previous row is
  contradicted by `findVisualLineIndex:591-593`: equality belongs to following
  source-start row. Main uses `(0,1)` at 3, `(1,1)` at 4.
- v4 subsequently received one APPROVE and one NEEDS_FIX: its ASCII-only
  whitespace predicate contradicts native U+3000 handling. Main confirmed RED,
  implemented the exact `unicodeLayoutWrapBreakKind` separator set, and isolated
  U+200B from the painter's continuation grouping. Normalized byte starts now
  distinguish source boundaries on either side of a zero-cell separator while
  cell starts still measure caret columns. v5 manifest
  `723e9c486e537431be1a73fa75868669c8b59778cd6a1f701d4c0741c0b76498` passes
  975 native tests; two fresh exact-candidate reviews pending. v4 remains rejected.
- Duplicate top composer metadata removal was authorized by independent contract
  evaluator `ses_e820a9a07ffegh7eaoedix9ZPk`. Strengthen existing last-occurrence
  lookup to exact-one occurrence. Preserve composer-owned metadata, real busy
  header, loader classes, and all genuine busy/status assertions. Not implemented.

## Returned implementation proposals — retain as proposals

| Area/session | Useful source findings | Remaining issue or scope |
| --- | --- | --- |
| Theme `ses_e820a9a1fffeo7fF92fTZ3tkF0` | Default dark interactive orange, accent purple, warning `#F5A742`; contextual resolution belongs in theme resolver. | New proposal still places orange first in categorical while claiming blue first. Do not copy its palette wholesale. |
| Tabs `ses_e820a9a10ffet6djXWTRBNjhdW` | Settled horizontal active is `decrease(raised.base)`; vertical wide selected is action.selected. New follows tabs/overflow/list. | Proposed vertical loop is not viewport bounded; requires real scroll geometry. Resolver ignores alpha. |
| Agent `ses_e820a9a07ffdS0HksoAi3cYvA2` | Visible inventory order; dedupe categorical; default step200/new400; configured override; unknown first. Backend currently drops configured color. | Supplied `Copy` struct contains String and cannot compile. Resolve active categorical theme before wiring. |
| Assistant footer `ses_e820a99fbffedejdxzm0aiASOr` | Durable history has message/body/created_at plus reasoning/tool activity; live terminal frame has provider usage and stop_reason. | No persisted agent/model/start/completion/finish/retry/error facts. Honest history footer requires a producer and durable sidecar/API change; do not fabricate facts in client projection. |
| Titles `ses_e820a9957ffey1mj3b1GHg1T9n` | Current native lacks title SSE; authenticated listing and accepted bind snapshots should seed display titles. Future events need shared durable ordering. | Proposed tuple still uses compatibility revision0; must separate snapshot/live provenance honestly before integration. No exact-candidate review. |
| Markdown `ses_e820a9917ffeFICAAB8Ocqn6HV` | Inline parser unused; production wrapping loses style spans; display ranges must be cells. | Caller is placeholder; link target display/parsing semantics and output budget remain unreviewed. Cites mini source despite normal-source requirement. |
| Fences `ses_e820a9917ffd0IJ0O8U66H4G50` | Existing parser can omit recognized opening/closing marker rows while retaining body whitespace and literal user path. | Source/rendered setting not wired; constant true is a proposal, not full mode support. |
| Shell `ses_e820a994bffeZ3VnbrkcuhBd5B` | Normal shell caller limits command to2 rows and output to remaining budget; helper source in collapse-tool-output.ts. | Supplied code assumes undefined grapheme helpers and character units; test transcription inconsistently prefixes each command. Re-read source before port. |
| Inline tools `ses_e820a994cffeImPHegAHi6gGq3` | Tool-specific glyph/title dispatch; backend-only status/error; no fabricated timing/metadata. | Number-or-Bool Rust pattern binds incompatible types; generic running still gets completion check; stated inset differs from prefix. Not ready. |
| Settings registry `ses_e820a9917ffbM14IYoxAXgYrq8` | Only theme/sidebar/tab layout are meaningfully wired; system theme partial. Other rows need load/live consumer, not persistence alone. | Helper normalizes JSONC and does not preserve comments. Must validate values and actual StateLocation API. |
| Settings interaction `ses_e820a9902ffeRIZapAHyTzXohw` | Shared layout/hit map, persist-first proposals, owned rows. | Incomplete runtime wiring; height truncation lacks selected-row viewport; hit width uses scalar count rather than cells. Not accepted. |
| Dialogs `ses_e820a9902ffd0nVWLDyp13wZrb` | Normal dialog source declares inset3 and widths60/88/116, alpha150; new-design opt-in. | Proposed permission marker/style mapping must not be adopted as generic exact dialog parity without caller-specific evidence. |
| Permissions `ses_e820a98b2ffewr1nN8V5DpGbhb` | DialogView owns choice/ACK identity; typed presentation metadata is needed; scoped/persistent Deny retained. | Return is adapter plus prose, not full patch. Must preserve real row-to-choice hit map while introducing horizontal buttons. |
| Completion `ses_e820a98b2ffd3RRoezsYPUjnTe` | Header/footer removal requires parallel hit values and removing first/last row guards. Capacity min10/count/anchor space. | Concrete proposal still needs existing caller acceptance and cursor/suffix tests; no runtime/render verification. |
| Toast `ses_e820a9891ffeKbX3fajNtalPmz` | Transcript notices exist but no toast owner. Reference top1/right2/max60/terminal-minus6, split rails, padding. | Code uses terminal-minus2 instead of stated-minus6 and cell-clamped measurement cannot detect overflow; no actual owner. Do not integrate sketch. |
| Visual `ses_e820a9881ffeW9kumwNEeaec35` | Freeze font/theme/content/state; compare Home/chat/cursor/tabs/sidebar/dialog/permission/tool states at269x59,120x59,43x24. | Return repeats obsolete right-edge New assertion and misses matched no-shadow1904x937 capture. Dynamic content mismatch still prevents pixel acceptance. |

Private candidate/test evidence remains under common Git
`recovery-evidence/full-visual-parity-20261027/`. Preserve all original negative
verdicts; this document does not outvote them or make an acceptance claim.
