# TUI-007-nav worklog

Claim: tab navigation types in `crates/cli/src/native_navigation.rs`.
Source evidence: HEAD 5af7884; card TUI-007 via
`python3 tools/completion_plan.py --card TUI-007` (T01 independent tab
state, T02 child provenance, T03 fork/resume real service, T04
concurrent rename/archive converge, T05 bounded lists + close keeps
daemon work); prior art `crates/cli/src/native_app.rs:1-120`
(pure-state module, bounded constants, `forbid(unsafe_code)`).
Pre-existing file was 323 lines, MAX_TABS=12, unversioned rename events,
focus-drifting eviction; `main.rs:19-34` has no `native_navigation` mod
(pre-wired by integrator, not my lane).

Observed scenario: base `rustc --test` run GREEN on 7 tests before edit;
fixed API gaps (no close receipt, no rev convergence, no pagination,
no fork/resume intent) by rewriting owned file only.

Target boundary: pure state, std only, `forbid(unsafe_code)`, no IO/clock/
threads. MAX_TABS=32 bounds tabs/titles/work/tombstones. `close` keeps
daemon work entry, returns `CloseReceipt`. Rename/archive carry daemon
`rev`; LWW on highest rev; archive tombstone drops stale renames.
`fork_request`/`resume_request` build intents caller hands to real
service; daemon `Forked` opens child tab with parent provenance.

Tests (frozen in-file `mod tests`, 8 tests): independent drafts/scroll/
models; close receipt + retained Running work + unknown-close refusal;
concurrent rename LWW + tombstone-zombie drop; fork provenance + request
routing/refusals; archive focus-fallback + work drop; bound=32 +
`tab_page` pagination incl. empty pages; eviction keeps focus + close-before-focus
keeps session; switch refusals.

Decisions: eviction drops oldest (index 0) and shifts `active` so focus
stays on same session; archive reuses `remove_tab`; `tab_page` returns
borrowed slice, empty on OOR/zero per-page.

Remaining unknowns: mod wiring + daemon rev plumbing owned by integrator.
