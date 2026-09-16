# RED-VALIDITY-AUTO2 (sole-writer wave, rev 248f519)

Scope: subagent owned ONLY `crates/agents/src/delegation_lane.rs` +
`driver_lane.rs` + `crates/agents/tests/delegation_gated.rs` this wave.
GREEN pre-existed; per-ID temp-stub behavior inversion, compiling RED log,
restore byte-identical (sha256 pre==post), GREEN 5/5. Serial,
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1, timeout 120, rtk prefix, free -h
first (3.0-3.1Gi avail, abort-line 1GiB never hit), --test-threads=1,
disposable /tmp/opencode only, no host DB.

Pre hashes (== post, diff -q IDENTICAL):
- delegation_lane.rs 51d8a6c1c7fcfed589c60d1deb7eced8cbf642f7ba83772baeded5bf573e9681
- driver_lane.rs c19ad097a15b82cc5285f641e15db0af698d9dc4c19ef306283234838fa1f6dd
- delegation_gated.rs 0dd9e565e23cb1d3d0e29edcd47917eaff7a656eabe4500fee0707b0b23adec7

## AUTO-004 (owned impl delegation_lane.rs; frozen tests untouched)
- Stub: `detach` owner guard `!=` -> `==` (edit tool, one line).
- RED /tmp/opencode/wA-AUTO004-red.log: compiles, 3 passed / 2 failed.
  FAIL auto_004_t02 (tests/delegation_lane.rs:37 owner detach wrongly
  denied), FAIL auto_004_t03 (:60 detach wrongly denied).
- GREEN /tmp/opencode/wA-AUTO004-green.log: 5 passed / 0 failed.

## AUTO-006 (owned impl driver_lane.rs; frozen tests untouched)
- Stub: `LeaseTable::release` guard `==` -> `!=` (edit tool, one line).
- RED /tmp/opencode/wA-AUTO006-red.log: compiles, 4 passed / 1 failed.
  FAIL auto_006_t02 (tests/driver_lane.rs:70 non-owner release wrongly
  succeeded).
- GREEN /tmp/opencode/wA-AUTO006-green.log: 5 passed / 0 failed.

## AUTO-005 declarative (owned gated tests; impl owned delegation_lane.rs)
- Behavior stub: `broker_allows` Allow-only -> inverted (one line).
- RED /tmp/opencode/wA-AUTO005-declarative-red.log: compiles, 0 passed /
  5 failed (fa3_gated_t01..t05, delegation_gated.rs:18/:34/:44/:55/:77).
- GREEN /tmp/opencode/wA-AUTO005-declarative-green.log: 5/5 pass.
- Live tool PASS/mutated evidence (rev auto005-rev-001, /tmp/opencode/wA005):
  PASS exit 0 all-pass (report-pass.json); mutated 1-byte exit 2
  mutated-frozen-evidence (report-edited.json); worker-only exit 2
  self-report-not-evidence (report-self.json); wrong-rev exit 2
  verifier-revision-mismatch (report-rev.json). Tool sha256
  eea1ac0f21d5d047f9455b6d9b77c5a8f3942ad4ad1ff34efcff9b723e288474.

## Full crate GREEN (restored impls)
- /tmp/opencode/wA-agents-full-green.log: lib 15 + gated 5 + lane 5 +
  driver 5 + turn 5 = 35 passed, 0 failed.
