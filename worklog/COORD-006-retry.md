# COORD-006-retry worklog

## Claim

Crash-safe lease store with fencing tokens, atomic singleton, crash-safe
acceptance publication, proof-gated recovery. Owned file only:
`tools/completion_leases.py` (created, real content, stdlib only).

## Source evidence

- Base revision `5af7884cf7637c0760d985da7a03f2f99ccd0c78`.
- `tools/ralph_loop.py:354-365` acquire_singleton check-then-write race
  (LOCK.exists then write, two starters can both pass).
- `tools/ralph_loop.py:587-618` record_result marks `accepted` (:602)
  before finalize_worktree (:604): crash yields accepted-but-unintegrated.
- `tools/ralph_loop.py:550-570` heartbeat scoped to worker/verify only;
  serial record_result+heartbeat/release `:738-754` holds lease through
  integrate but heartbeat renewal stops before it.
- `tools/ralph_loop.py:110-283` LeaseTable/DurableLeaseTable: no fencing
  tokens, no O_EXCL singleton, no recovery-proof gate, no atomic
  acceptance publication.
- Config `config/completion-controller.json:17-18` leaseTtlSeconds 180,
  heartbeat 30 (defaults honored, never raised).
- Card COORD-006 via `python3 tools/completion_plan.py --card COORD-006`;
  T01..T05 mapped in self-check asserts.

## Observed scenario

- File did not exist before slice (`ls tools/` showed no
  completion_leases.py; grep for completion_leases/COORD-006 empty).
- Created `tools/completion_leases.py` (24772 bytes, 575 lines).
- No other files edited (AGENTS.md ownership: owned file only).

## Target boundary

- Implement: `tools/completion_leases.py` only.
- Callers (ralph_loop integration) out of scope; module exposes
  LeaseStore/SingletonLock/publish_acceptance for controller wiring.
- No threads/processes/network; fcntl file locks; JSON cap 4MiB.

## Tests

- Self-check under `__main__`: T01 heartbeat through 3 stages same
  fencing; T02 stale fencing heartbeat/release/guarded_write denied;
  fencing monotonic on reclaim; T03 failed integrate -> pending receipt,
  no ledger, no accepted flag; T04 dual singleton refused, stale
  descriptor reclaimable, live lease not reclaimable; T05 recovery
  without proof raises, evidence preserved; bounds (identity/ttl/
  capacity/oversize) fail closed.
- Commands (all exit 0):
  - `timeout 60 python3 -m compileall -q tools/completion_leases.py`
  - `timeout 30 python3 tools/completion_leases.py` -> `leases self-check: OK`
  - `timeout 30 python3 tools/completion_plan.py --check` ->
    `SPEC OK: additions=90, legacy=258, scenarios=450; NOT product acceptance`

## Decisions

- Fencing int per task, +1 on reclaim; every mutation owner+fencing
  checked under exclusive file lock; stale fails without mutation.
- Singleton via O_CREAT|O_EXCL + pid-alive stale reclaim + token-checked
  release; second starter gets SingletonHeldError.
- Acceptance: receipt JSONL first, then ledger tmp+os.replace; failed
  integrate writes pending receipt only, never accepted flag.
- Recovery lists expired leases only; optional owner_stopped proof gate;
  live leases never returned/reclaimed.

## Remaining unknowns

- Controller wiring (ralph_loop.py acquire/heartbeat/release call sites)
  belongs to integrator/another lane; not touched per one-file grant.
- Verifier decides acceptance; evidence above is self-check + spec gate.
