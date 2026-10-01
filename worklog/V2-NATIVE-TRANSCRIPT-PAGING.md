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

## Exact integrated acceptance

The controller independently verified candidate
`5c6f9cb05f6997423da62c8edca8df07a4754103` against the frozen test suffix and
the actual pinned OpenCode `95daf90670b7c039c436c85537da5fbfe2205b41` sticky
scroll fixture. All three focused paging tests and **314 native CLI unit tests**
passed. The same commands passed after integration on exact
**`7c39feef3a3282831522ea226708e6aee8ff847b`**:

```text
cargo test --offline --locked -p opencode-rk-cli --features native --bin oc2 page_pins_to_bottom_and_scrolls -- --test-threads=1
  3 passed, 0 failed, exit 0
cargo test --offline --locked -p opencode-rk-cli --features native --bin oc2 -- --test-threads=1
  314 passed, 0 failed, 0 ignored, exit 0
cargo fmt --all -- --check
  exit 0
```

Actual source/command/environment/log hashes and results are retained at
`v2-paging-preverify-5c6f9cb-nbzylt6r` and
`v2-paging-integrated-7c39fee-eianxukd`, under
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/`.
The two original failures also reproduce on canonical baseline `06eadf1`, at
`v2-paging-baseline-06eadf1-lnv8vacs`; they predate the Unicode decoder.

State: **ACCEPTED for the bounded transcript paging helper contract on exact
`7c39fee`**. The existing frozen test body is byte-identical. This clears the
observed native-unit blocker for subsequent Unicode verification; interactive
scroll/mouse wiring and retention-eviction anchoring remain separate scope.
