# Worklog COORD-001 adapter (harness-adapter lane)

Claim: real stdlib-only `tools/harness_adapter.py` binding scheduler `TrustedAdapter`
with capabilities, role separation, cancellation/join, bounded redacted logs.

Source evidence (HEAD 5af7884):
- `tools/completion_scheduler.py:79-94` TrustedAdapter Protocol
  (execute/verify/integrate/verify_integrated); `109-156` validate_*;
  `158-260` run_rolling (caps 20/20/3, path locks, cancel/join 253-260).
- `tasks/completion/delivery.json:4` COORD-001 card: paths
  `tools/harness_adapter`, `docs/ADAPTER_PROTOCOL.md` (latter NOT owned).
- `config/completion-controller.json`: caps 20/20/3, no budget increase,
  no CLI substitution.
- `docs/ADAPTER_PROTOCOL.md:18-19` forbids invented success adapters;
  `:47-49` worker labels are not trust levels.
- `sources/completion/audits/AUD-017.json:61-64` gap: adapter/roles missing.

Observed scenario: file `tools/harness_adapter.py` did not exist; owned-file
only task creates it fresh. No other file touched.

Target boundary: ONLY `tools/harness_adapter.py`. No edits to scheduler,
tests, config, docs, controller, or any sibling tool.

Tests: frozen suite stays green, not edited:
- `timeout 60 python3 -m compileall -q tools/harness_adapter.py` OK
- `timeout 60 python3 -m unittest discover -s tests/completion -p 'test_*.py'`
  38 tests OK (test_scheduler 22 + test_plan 16)

Self-checks (throwaway, not committed): role separation rejects
implementer==verifier; protected implementer grants rejected
(`tests/`, `state/`, `config/`, `sources/`, exact verifier/controller paths);
unavailable backend raises AdapterUnavailable naming no-CLI-fallback;
real backend roundtrip completes via run_rolling; shutdown cancels/joins
owned inner task (inflight 1 -> 0, CancelledError propagates);
BoundedLog byte-capped with whole-credential-line masking (`sk-` token form
fixed during self-check).

Decisions:
- `NativeHarness` Protocol: host supplies real API; adapter never invents it.
- `UnavailableHarness` fails closed (RetryableFailure subclass) with explicit
  no-CLI-fallback message. No echo/sleep/PASS path exists.
- `Capability` frozen dataclass, `mint_capability` validates via scheduler
  `normalized_path`; implementer denied protected writes.
- `assert_role_separation` enforces verifier set excludes implementer,
  integrator differs.
- `HarnessAdapter._join` enforces max_inflight backpressure, tracks owned
  tasks, re-raises CancelledError after joining inner; `shutdown` cancels
  all owned and clears.
- No subprocess/pty/os-spawn/credentials/budget invention anywhere.
- `ponytail:` lease fencing/heartbeats, VCS ancestry proof, and provider/RAM
  measurement stay in COORD-006/007 lanes; adapter records bounded tails only.

Remaining unknowns: real host backend binding (host-side, out of lane);
`docs/ADAPTER_PROTOCOL.md` update belongs to a docs-owned lane, not this one.
