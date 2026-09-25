# BRIDGE-GAP-24 — run_replay anchor-merge

claim: ses_gap24, ledger in-progress (pre-existing fence = own claim).
source: no TS session-replay.ts / replaySession in tree (grep empty); spec self-contained.
target: crates/opentui-bridge/src/run_replay.rs only. No lib.rs/cargo/commit per task.
tests: 7 unit tests in-file (disjoint, anchor dedupe remote+local, non-anchor dup preserved, cap 2000, empties, order stable).
decision: std-only HashSet<&str> borrow, saturating_add/min capacity, `#` prefix = anchor.
