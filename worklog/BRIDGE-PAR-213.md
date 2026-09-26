# BRIDGE-PAR-213 run_footer_full

claim: BRIDGE-PAR-213 ses_par213 ok.
source: footer.ts:167 `export class RunFooter implements FooterApi` (mutable footer control surface); sibling crates/opentui-bridge/src/session_footer_full.rs:9 `SessionFooterFull` (slot cap32, items cap16x128, busy, render cap512).
observed: no run_footer_full.rs; single owned file lane; lib.rs/tui_entry/session_footer untouched.
target: crates/opentui-bridge/src/run_footer_full.rs, FooterFull {mode cap32, busy, items cap16x128} + set_mode->bool + set_busy + push_item->bool + summary cap256. forbid unsafe, std-only, <140 lines, >=5 tests.
tests: 6 unit tests in-file; verify rustfmt --check only (no cargo per scope).
decisions: ponytail: set_mode always true (no failure mode except full, N/A for scalar); items truncate 128 mirroring sibling; summary reports count not contents (spec: N items).
unknowns: none blocking.
