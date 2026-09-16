# AUTO-GATED-COVERAGE — fA3 submit_gated/BrokerDecision/BoundedLanePool

## Claim
Zero frozen coverage on gated admission path now covered by new test file.
Frozen files untouched by this lane.

## Source evidence (commit 248f519 + workdir impl)
- `crates/agents/src/delegation_lane.rs:323-398`: `submit_gated` (deny/human ->
  `BrokerDenied`, no state; Allow -> `submit`), `BrokerDecision::{Allow,Deny,
  RequireHuman}`, `broker_allows` (true only Allow), `BoundedLanePool`
  (cap>=1, `try_acquire` broker+cap gate no-mutation on deny, `release`
  saturating via `checked_sub`, `live`).
- Frozen: `crates/agents/tests/delegation_lane.rs` (5x auto_004, no gated refs).
  Workdir shows format-only diff on frozen file from concurrent lane — NOT mine.
  My lane adds one untracked file only.

## Tests (new file only: `crates/agents/tests/delegation_gated.rs`, 5 tests)
1. `fa3_gated_t01_deny_blocks_with_no_state` — Deny -> BrokerDenied, live 0.
2. `fa3_gated_t02_require_human_blocks_with_no_state` — RequireHuman -> BrokerDenied, live 0.
3. `fa3_gated_t03_allow_admits_lane_work` — Allow admits, owner/mode, live 1.
4. `fa3_gated_t04_pool_cap_denies_over_admission_without_mutation` — cap 2,
   3rd acquire false, deny/human false, release+reacquire.
5. `fa3_gated_t05_pool_over_release_never_underflows` — double release at 0 stays 0.

## RED (`/tmp/opencode/gated_red.log`)
Temp probe `zz_gated_red_probe.rs` (2 tests asserting gated contract vs plain
`submit`): 0 passed / 2 failed — plain submit admits deny+human workloads
(live 1 vs 0). Probe deleted after capture.

## GREEN (`/tmp/opencode/gated_green.log`)
`--test delegation_gated`: 5 passed / 0 failed vs gated impl.
Full `-p opencode-rk-agents`: 15 (lib) + 5 (gated) + 5 (lane) + 5 (driver) +
5 (turn) = 35 passed / 0 failed.

## Hashes
- delegation_gated.rs: 0dd9e565e23cb1d3d0e29edcd47917eaff7a656eabe4500fee0707b0b23adec7
- delegation_lane.rs (src): 51d8a6c1c7fcfed589c60d1deb7eced8cbf642f7ba83772baeded5bf573e9681
- delegation_lane.rs (frozen test): 3ca19e36aae23c85a0ee65085d7df62b8d793c88ed2bcc08d02e7e0355b1d971

## Remaining
None. Gated impl pre-existed in workdir; lane added coverage only.

## Re-verify 2026-09-16 (sole-writer wave)
- `submit_gated` present delegation_lane.rs:324. delegation_gated.rs unchanged (sha256 0dd9e565…adec7). No edit.
- Agents: 35 passed / 0 failed (`/tmp/opencode/w1-agents.log`). Frozen tests untouched.
