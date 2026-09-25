# Worklog COORD-001 (repair of failed handback)

Claim: repair lane where prior worker ses_f27cc03adffe0EXj54gdYYbTWu returned
analysis only and made no on-disk claim/scratchpad/commit. Verify current source,
claim the task, and record an honest BLOCKED status with exact evidence. No product,
doc, test, controller, config or policy edits: no independently authored failing
test and no real host binding exists.

## Prior handback defect (reproduction)

- `tasks/completion/claims.json` had NO `COORD-001` row before this lane claimed it
  (verified `'COORD-001' in claims == False`).
- No `worklog/COORD-001.md` existed; only stale `worklog/COORD-001-adapter.md` and
  `worklog/COORD-001-adapter-protocol.md` from other sessions/HEAD.
- `git log --oneline -1 -- tools/harness_adapter.py` -> aea6210 (ancestor); this
  lane branch had no COORD-001 commit. `git status` clean.
- Stale `worklog/COORD-001-adapter.md` claims `tests/completion` ran 38 tests; that
  directory does not exist on disk and is not tracked at HEAD.

## Source evidence (current revision)

- HEAD of lane/COORD-001-NATIVE-ADAPTER: `2d04c1c` (map macOS phase one vertical
  product journey). `tools/harness_adapter.py` tracked and present at HEAD (320 L).
- `tools/harness_adapter.py:1-21` module docstring: binds a host-supplied
  `NativeHarness` only; explicit "never spawns CLI/OS processes", "no invented
  credentials/budget".
- `tools/harness_adapter.py:158-165` `class NativeHarness(Protocol)` with
  `execute/verify/integrate/verify_integrated`; `:167-181` `UnavailableHarness`
  raises `AdapterUnavailable` for every stage (no fake execution).
- `tools/harness_adapter.py:186-260` `HarnessAdapter` binds the scheduler
  `TrustedAdapter`; `:265-274` `shutdown` cancels/joins owned inner tasks.
- `tools/harness_adapter.py:24-27,64-80` `Capability` one-file grant; protected
  prefixes `tests/`,`state/`,`config/`,`sources/` and exact verifier/controller
  files denied to implementers.
- `tools/completion_scheduler.py:79-94` `TrustedAdapter` Protocol; `:136-155`
  `validate_candidate`/`validate_proof`.
- `docs/ADAPTER_PROTOCOL.md` section 8 (lines ~210-240) B1..B8: each row is a
  missing host surface (native subagent spawn/collect API, one-file OS grant,
  independent test-author+frozen-hash store, verifier identity registry,
  serialized VCS integrator, durable lease store, provider budget source,
  signing/consent authority) marked BLOCKED, owner future COORD lanes, no
  credential/device/budget fabricated.
- Card evidence: `python3 tools/completion_plan.py --card COORD-001` ->
  testObligations `COORD-001-T01..T05`, mandatory true, paths
  `tools/harness_adapter`,`docs/ADAPTER_PROTOCOL.md`.
- Negative search: no test file references `harness_adapter`/`HarnessAdapter`/
  `NativeHarness` (`grep -rn --include='*.py'`), and `tests/completion/` is absent
  on disk and untracked at HEAD (`git ls-tree HEAD tests/` -> only `bootstrap`,
  `fixtures`).

## Observed scenario

The adapter module is scaffolding with an unbound host Protocol. The card's journey
requires "real harness-native worker/test-author/verifier/integrator APIs"; section 8
proves those host APIs are absent (B1, B3, B4, B5, B6). No independently authored,
compiling RED test exists for COORD-001-T01..T05, so AGENTS.md test-freeze policy
cannot be satisfied and no real host binding can be exercised.

## Target boundary

Only `worklog/COORD-001.md` + the `COORD-001` row in `tasks/completion/claims.json`.
No changes to `tools/harness_adapter.py`, `tools/completion_scheduler.py`,
`docs/ADAPTER_PROTOCOL.md`, `tests/`, product code, controller/config/policy.

## Checks run (lightweight, rtk-prefixed, no Cargo)

- `rtk python3 -m py_compile tools/harness_adapter.py tools/completion_claims.py
  tools/completion_scheduler.py` -> PY_COMPILE_OK.
- `python3 -c "import tools.harness_adapter, tools.completion_scheduler"` ->
  IMPORT_OK ('implement','verify','integrate') TrustedAdapter.
- `python3 tools/completion_plan.py --card COORD-001` -> card JSON, mandatory true,
  T01..T05.

## Decisions

- Set ledger status `blocked` (not `completed`): a real host binding and an
  independently authored failing test are both absent; the card cannot be satisfied
  by scaffold-only code, and fabricating GREEN is forbidden.
- Do not edit tests to manufacture RED; do not invent a host backend.
- Scratchpad + ledger only; branch pushed as a permanent reference.

## Remaining unknowns / unblock condition

- Operator supplies the native delegate API + auth + concurrency limit, freeze
  service, verifier identity registry, serialized integrator lane and durable lease
  store (docs/ADAPTER_PROTOCOL.md section 8 B1..B8). Once a host binding exists, an
  independent test author writes compiling RED for COORD-001-T01..T05 (needs
  `tests/completion/`), freezes hashes, and implementation proceeds.