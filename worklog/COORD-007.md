# COORD-007 worklog — budget enforcer

Claim: implement `tools/completion_budgets.py` per card + controller config.
Source evidence: HEAD 5af7884; `tasks/completion/delivery.json:10` (5 test
scenarios); `config/completion-controller.json:12-16` (8192/2048/1024/jobs 2,
heavy 1, attempts 3, allowBudgetIncrease false);
`config/resource-targets.json:20-25` (measure whole process tree);
`PLAN.md:227-239` (resource acceptance); `docs/SECURITY.md`, `docs/TDD.md`;
sibling `tools/completion_scheduler.py:158-260` (caps 20/20/3, cancel/join).

Observed scenario: file absent (`tools/` has no completion_budgets.py).
Target boundary: owned file only; no edits to controller config, scheduler,
tests, or frozen artifacts. Config read-only; `allowBudgetIncrease` enforced
false (raise rejected). Stdlib only.

Tests: no frozen suite exists for this file. Verified live:
`timeout 60 python3 -m compileall -q tools/completion_budgets.py` OK;
`import tools.completion_budgets` OK; functional probe OK
(cfg 8192/2048/1024/jobs2/attempts3; avail 2734 MiB, tree RSS 14 MiB;
admit F/T/F incl. None-closed; classify/retry correct incl. max-3 cap;
parse_retry_after 120/bad/None; lock acquire/hold/release;
cross-instance exclusion True/False/True; provider slot+rate+Retry-After gate;
BoundedLog byte/entry caps with drop count; BudgetEnforcer mem-gate+shutdown).

Decisions: /proc walk + `ps` fallback (whole tree, never main-pid only);
admission fails closed on unmeasurable memory; file-lock heavy semaphore
(default `state/heavy-validation.lock`, nonblocking, cross-process);
blocked-substring classifier covers auth/security/signing/budget;
Retry-After delay-seconds only, capped 3600s; BoundedLog tail-truncate caps;
`BudgetEnforcer.shutdown()` releases lock/slots/pending/logs.

Remaining unknowns: none for lane. Freezer/verifier decide RED/GREEN status;
integration wiring (scheduler consuming enforcer) belongs to COORD-008, not
this one-file lane.
