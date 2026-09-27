# Wave 3 - Complete native pages and navigation

Status: DRAFT. Entry: `P1-G2` independently passed.

## Demonstration that closes this wave
Every approved native page/dialog/command is reachable through actual input; implemented actions are real, and Wave 4 service-dependent rows are explicitly pending rather than falsely accepted.

## Work packages
- **P1-W3-01 - Complete session/workspace navigation and per-tab state**. Users navigate their actual projects and history without losing draft state or terminating background work.
- **P1-W3-02 - Complete fork, rewind, redo, compaction and retry semantics**. Timeline actions change exactly the intended durable branch/boundary and never replay ambiguous effects.
- **P1-W3-03 - Render and navigate a production transcript, code and diff view**. Streaming remains readable and scroll/selection stay stable during long coding sessions.
- **P1-W3-04 - Replace static command labels with the real command registry**. Every listed action either executes its actual behavior or clearly explains why it is unavailable; a static menu is not accepted.
- **P1-W3-05 - Finish provider, model, account, variant and effort pages**. Selected settings affect actual subsequent provider requests and survive restart without leaking credentials.
- **P1-W3-06 - Complete agent selection and child-session presentation**. Agent and subagent screens reflect real daemon work and preserve parent/child provenance.
- **P1-W3-07 - Finish themes, keyboard help, settings, status and memory panels**. Operational panels contain truthful live information, and every visible setting affects the application.
- **P1-W3-08 - Expose actual file, diff and local terminal workflows**. The UI inspects and operates on the selected workspace only; a terminal panel is not unrestricted host shell authority.
- **P1-W3-09 - Complete safe export, local sharing and bounded import journeys**. Users export, safely share within the approved local boundary, and import history without secrets or data loss.
- **P1-W3-10 - Verify every approved native page and interaction from a real terminal**. A source-tracked interaction matrix proves reachable working native pages, not a count of enum variants or menu strings.

## Gate criteria
- Navigation and standalone native interactions are verified; missing Wave 4 backend functions remain open.
- The remaining service-dependent matrix is finite and has exact Wave 4 owners; no full-parity claim is made.
- All prior golden journeys still pass.

## Capacity and sequencing
Freeze shared contracts first; then dispatch only non-conflicting one-file candidates. Reserve integration and independent review capacity. Use one heavy validation token across the wave, not one build per worker. A wave may contain multiple bounded dispatch rounds; wave count is not a concurrency or time estimate. Keep repair capacity rather than filling every slot with new breadth.
