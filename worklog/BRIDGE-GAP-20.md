# BRIDGE-GAP-20 — MenuNav (ses_gap20)

Claim: pre-held by ses_gap20 in-progress (re-claim fenced, same session).
Source: input.rs FocusRing (push/next/prev/current, MAX_FOCUS=64, fail-closed Empty/Full/Invalid); widget_paint.rs menu_calls (rows, selected kept by caller).
Boundary: ONE new file crates/opentui-bridge/src/menu_nav.rs, std-only, 114 lines. No lib.rs/cargo/commit touched.
Tests: 6 unit tests in-file (empty_fail_closed, down_wraps, up_wraps, select_tracks_focus, count_capped_at_64, single_item_stays).
Decisions: ids 1..=count pushed into FocusRing; up/down delegate to prev/next (.ok() => None empty); select/selected delegate to current(); new() caps at MAX_FOCUS.
Unknowns: lib.rs `pub mod menu_nav;` wiring left to orchestrator (out of scope); crate tests not run (would need wiring).
Verify: rustfmt --check crates/opentui-bridge/src/menu_nav.rs => FMT_OK.
