# BRIDGE-GAP-21

- Claim: `BRIDGE-GAP-21`, session `ses_gap21`.
- Evidence: `/home/rashid/projects/opencode/packages/opencode/src/cli/cmd/run/types.ts:169-176` defines footer surface variants; `:299-320` defines append-only stream commit; task requires native compact equivalents.
- Boundary: one std-only file, `crates/opentui-bridge/src/run_types.rs`; no `lib.rs`, cargo, or commit.
- Contract: `FooterPhase` has Prompt, Permission, Question, Subagent, Done. `FooterView` carries phase plus busy. `StreamCommit` carries id/text; validation rejects either field above 1024 bytes, accepts empty and boundary-sized values.
- Tests: five phase cases, empty acceptance, oversize ID, oversize text, and exact-boundary acceptance.
- Decisions: use byte limits (`String::len`) and a small public validation error enum; derive ordinary value traits.
- Remaining: integrator must wire module in `lib.rs`; lane verification is rustfmt plus standalone test compilation.
