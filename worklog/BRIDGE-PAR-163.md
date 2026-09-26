# BRIDGE-PAR-163 scratchpad

claim: cc.claim BRIDGE-PAR-163 / ses_par163 / worklog/BRIDGE-PAR-163.md — ok
source: footer.width.ts (footerWidthPolicy breakpoints only, no item sum) + run_width.rs:30-41 policy tiers (read-only)
target: crates/opentui-bridge/src/footer_width_calc.rs — width_for/fits/truncate_to, sep 3 cols, min 1
tests: 6 in-file (empty, single, multi-gap, clamp, drop-tail, min-one) — written, not run (no cargo per scope); rustfmt --check PASS
decision: raw_width helper private; chars().count() for unicode-safety; truncate pops tail only
unknowns: none
