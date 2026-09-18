# COORD-003 worklog

Claim: one-file child ownership table at tools/completion_ownership.py.
Source evidence (HEAD 5af7884):
- tools/completion_scheduler.py:97-102 normalized_path (reject absolute,
  traversal, glob, trailing slash); 105-106 overlaps() subtree lock
  (`a==b` or prefix `b+"/"` either direction); 109-133 validate_tasks;
  136-140 validate_candidate one-file grant check; 224-226 held-lock skip.
- Card: tasks/completion/delivery.json:6 (COORD-003, 5 test scenarios).
- Shared-only-integrator rule: docs/ADAPTER_PROTOCOL.md:57-59 (schema,
  migration numbering, manifests, Cargo lockfiles, central route registries).
Observed scenario: file did not exist; only scheduler + test_scheduler.py
(20 tests) + completion_plan.py existed.
Target boundary: OWNED FILE ONLY tools/completion_ownership.py. No other edits.
Tests: self-check asserts in `if __name__` (stdlib only, no framework):
path validation, overlaps incl. fork-namespace non-overlap, concurrent
same/subtree admit denied, unrelated proceeds, idempotent re-admit, grant
move denied, holder_for, check_output exact-grant reject, shared worker
deny + integrator admit, snapshot/restore, release+re-admit.
Decisions: copied scheduler path semantics verbatim for parity; shared set
covers Cargo.toml/lock, lib.rs, routes.rs, manifests, package.json,
migrations/schema(s) dirs; bounded 10000 grants; ponytail marks
parent-completion/discovery-inheritance deferred to plan/integration modules.
Remaining unknowns: none in-slice; crash-safe persistence is COORD-006's lane.
Evidence: `timeout 60 python3 -m compileall -q tools/completion_ownership.py
&& timeout 60 python3 tools/completion_ownership.py` -> self-check OK;
`python3 -m unittest tests.completion.test_scheduler` 20 tests OK;
`python3 tools/completion_plan.py --check` SPEC OK 90/258/450.
