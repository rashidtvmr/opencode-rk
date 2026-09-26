# BRIDGE-GAP-41

Claim: ses_gap41, in-progress (pre-claimed by orchestrator).
Source: crates/opentui-bridge/src/context_kv.rs (KvStore, Args conventions).
Target: crates/opentui-bridge/src/kv_toggles.rs only.
Tests: defaults_all_false, diff_wrap_flips, assistant_meta_flips, animations_flips, double_flip_restores, bits_independent.
Decision: plain struct + Toggle enum + apply flip; std-only, forbid(unsafe_code).
Unknowns: lib.rs wiring is orchestrator/integration job, out of scope.
