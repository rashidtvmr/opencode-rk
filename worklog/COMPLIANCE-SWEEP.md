# COMPLIANCE-SWEEP (AUTO lanes + TDD/SECURITY)

Scope: worklog/AUTO-003.md, AUTO-004.md, AUTO-005.md, AUTO-006.md, AUTO-007.md
Contract: AGENTS.md scratchpad = claim, source evidence, observed scenario,
target boundary, tests, decisions, remaining unknowns.
Plus docs/TDD.md (compiling RED, frozen hash, GREEN-on-frozen, no edited tests)
and docs/SECURITY.md (no unbounded queue/output, no detached task, no secret
logging, disposable fixtures, denied-permission side-effect asserts).

## AUTO-007 disposition

EXISTS — no discovery proposal needed.
- tasks/AUTO-007.md: turn submission state machine, REQ-002/REQ-012, T01-T05.
- ralph.json: id AUTO-007 + obligations T01-T05.
- FEATURES.md: REQ-002 row, REQ-012 row, in-progress row with T01-T05.
- worklog/AUTO-007.md exists with all 7 scratchpad sections.

## Per-worklog verdicts

### AUTO-003 — FAIL (1 missing section)
- Has: claim, source, boundary, tests, decisions, unknowns.
- Missing: observed scenario (no `## Observed scenario` / scenario section).
- TDD: PASS-ish. Behavioral RED 2 passed/3 failed after scaffold, frozen
  SHA-256 recorded, GREEN 5/5, no test edits after hash. Initial
  missing-API compile error correctly classified as authoring feedback.
- SECURITY: PASS. Controller-owned state/, bounded communicate+heartbeat,
  no new deps/threads/network. No secret/DB contact.
- Fixup: append `## Observed scenario` describing pre-change controller
  (no durable owner-checked leases, no heartbeat lifecycle) with 1-2
  file:line citations.

### AUTO-004 — FAIL (1 missing section + incomplete RED)
- Has: claim, source, boundary, tests, decisions, unknowns.
- Missing: observed scenario section.
- TDD: FAIL. RED baseline absent; tests use `#[path="../src/..."]` bypass
  (tests/delegation_lane.rs:1-2), GREEN-only frozen hash. Worklog already
  self-reports this; verifier must treat as incomplete RED per TDD §3.
- SECURITY: PASS with noted gap. Bounds declared (max_live 16, summary cap,
  child_process_count()=0). Tokio/broker gap openly recorded, not smuggled.
- Fixup: add observed-scenario section (no owned delegation records,
  status/cancel undefined); do NOT edit frozen tests to retrofit RED.

### AUTO-005 — FAIL (1 missing section + no RED/GREEN)
- Has: claim, source, boundary, tests, decisions, unknowns.
- Missing: observed scenario section.
- TDD: FAIL (expected for declarative fragment). No frozen hash/manifest,
  no RED run, no GREEN-on-frozen. Worklog self-reports; verifier must
  reject acceptance until controller freezes and reruns. Correctly refused
  to invent product tests per TDD §3.
- SECURITY: PASS. Read-only manifest/receipt reads, writes only `<out>`,
  stdlib only, no state/ writes, no secret contact.
- Fixup: add observed-scenario section (no pipeline-enforcement fragment
  before tool); leave freeze to controller.

### AUTO-006 — FAIL (1 missing section + incomplete RED)
- Has: claim, source, boundary, tests, decisions, unknowns.
- Missing: observed scenario section.
- TDD: FAIL. Same `#[path]` bypass as AUTO-004 (tests/driver_lane.rs:1-2),
  GREEN-only hash. Self-reported; verifier must treat as incomplete RED.
- SECURITY: PASS. One-file lease, gate-never-commits, ATTEMPT_CAP=3,
  no OS process per lane, bounded tails/receipts.
- Fixup: add observed-scenario section (no ready-queue/lease/gate driver
  in agents crate); do NOT edit frozen tests.

### AUTO-007 — PASS (all 7 sections + full RED/GREEN)
- Has all: claim, source evidence, observed scenario, target boundary,
  tests, decisions, remaining unknowns.
- TDD: PASS. Compiling RED 1 passed/4 failed, frozen SHA-256 unchanged
  through GREEN 5/5, plus agents-crate regression (10 lib + 5 integration).
- SECURITY: PASS. One active turn, reject-not-queue, cancel releases
  payload/owner, typed errors leave state unchanged, no detached
  task/queue/retained output.
- Fixup: none.

## Stub scan

Log: /tmp/opencode/stub_scan.log (5 lines, 0 real hits).
- `crates/agents/tests/driver_lane.rs:91` — test fixture string
  `"placeholder"` fed to gate to assert rejection. Intentional, not a stub.
- `crates/agents/src/driver_lane.rs:354` — doc comment mentioning
  placeholder. Not code.
- `crates/storage/src/import_v2.rs:185,403` — comment + assert guarding
  AGAINST zero-byte placeholders. Not a stub.
- `crates/providers/tests/auth_profile.rs:4` — comment stating "no mocked
  success". Not a mock.
- Zero hits for `todo!`, `unimplemented!`, `#[ignore]`, mocked impl.
Verdict: no committed stubs/placeholders/ignores in crates/.
