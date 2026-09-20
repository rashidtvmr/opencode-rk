# LANE-APPSTART-VIEW scratchpad

Claim: LANE-APPSTART-VIEW, session ses_f423ccf75ffe72BSE5xLbs3Rf5, owned file crates/cli/src/app_start.rs.
Ledger: tasks/completion/claims.json (claim ok, in-progress).

Source evidence:
- crates/cli/src/app_start.rs:295-321 plan_default_launch routes mode+role+view; :312-315 Setup on Some(false)|None.
- crates/cli/src/main.rs:227-244 matches only plan.mode; plan.role/plan.view dropped (main lane owns that file).
- Frozen tests in app_start.rs mod tests (T1 fresh-HOME Owner+Main, T2 Attacher, T3 Setup, T4 no role/view off-TTY) already pin plan outputs; lane adds NO test edits.

Observed scenario: main lane needs branchable predicates over plan.role/view without touching plan internals.

Target boundary: add two pure helpers in app_start.rs only:
- is_owner_startup(&DefaultLaunch)->bool (consumes role, gated on NativeTui)
- needs_setup(&DefaultLaunch)->bool (consumes view, gated on NativeTui)
Both fail closed off-TTY even if struct built by hand.

Tests: existing app_start suite (frozen, zero edits); VERIFY cmd with --bin oc2 per lane order.
Decisions: boolean pair over enum — smallest diff main lane can `if` directly; mode gate duplicates struct invariant defensively.
Unknowns: none; main.rs wiring belongs to main lane.
