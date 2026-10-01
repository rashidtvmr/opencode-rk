# V2 native transcript paging

## Candidate implementation

- Package: native transcript paging
- Base: `06eadf18a64ff61f672dcbe489aa0201adcce300`
- Scope: `crates/cli/src/native_transcript.rs` and this worklog only
- Status: candidate; independent verification and integration remain pending

## Source evidence

The pinned upstream checkout at
`/Users/mymac/Projects/opencode-upstream-reference` uses sticky-bottom
scroll behavior in `packages/tui/test/cli/tui/inline-tool-wrap-snapshot.test.tsx`
(the `StickyScrollFixture` and its `stickyScroll`/`stickyStart="bottom"`
assertions). The native Rust `TimelinePage` precedent in
`crates/cli/src/native_timeline.rs`, `TimelinePage::scroll_up` and
`TimelineBuilder::page`, records a bounded positional end index when leaving
the pinned state and renders against that frozen position.

This patch applies that minimal local parity to `TranscriptPage`: pinned pages
follow appended lines, while a scrolled page freezes its positional viewport
until it is scrolled back to zero or reset. The anchor is intentionally
relative to the retained vector; this candidate does not claim newline-ID
stability through later scrollback eviction.

## Change and verification boundary

The product change adds only the private bounded `TranscriptPage::anchor`
state and uses it in `Transcript::page`; no API, retention bound, rendering,
ANSI handling, input-width logic, resource constant, or frozen test body was
changed. The frozen `#[cfg(test)]` suffix hash reported by the controller is
`acc3215f5b6634f64c220e7258b83f8c64cef237fd169b9f934c13b3cdaee5cd`.

Per the implementation ceiling, no build or test command is run by this
worker. The parent must independently run the focused paging gate and the
required native CLI regression gate on the exact candidate SHA, then repeat
them after integration.
