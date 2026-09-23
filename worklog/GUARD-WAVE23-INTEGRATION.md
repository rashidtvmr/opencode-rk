# Guard Waves 2–3 integration receipt

## Candidate

- Branch: `lane/GUARD-wave23-integrate-20260923`.
- Base: guard-contract receipt `924301e`.
- Scope: blocked audit evidence only; no product, test, controller, source-gap,
  verifier, or acceptance file changed.

## Integrated evidence

| Task | Audit commit | Integrated scratchpad |
| --- | --- | --- |
| EXT-001 | `ec8df6c` | `worklog/GUARD-EXT-001.md` |
| EXT-002 | `2499012` | `worklog/GUARD-EXT-002.md` |
| EXT-004 | `29d2b8f` | `worklog/GUARD-EXT-004.md` |
| EXT-006 | `2c291e0` | `worklog/GUARD-EXT-006.md` |
| EXT-009 | `26e54d7` | `worklog/GUARD-EXT-009.md` |
| EXT-010 | `bdab3fe` | `worklog/GUARD-EXT-010.md` |
| EXT-011 | `614c220` | `worklog/GUARD-EXT-011.md` |
| EXT-012 | `8c0bcd5` | `worklog/GUARD-EXT-012.md` |
| INT-001 | `2040fcb` | `worklog/GUARD-INT-001.md` |
| INT-003 | `233a99c` | `worklog/GUARD-INT-003.md` |
| INT-005 | `399c654` | `worklog/GUARD-INT-005.md` |
| INT-006 | `3a3751d` | `worklog/GUARD-INT-006.md` |
| INT-007 | `836c987` | `worklog/GUARD-INT-007.md` |
| INT-009 | `eb8bff9` | `worklog/GUARD-INT-009.md` |

Every row is normalized to the canonical task ID and `blocked` coordination
state. Local implementations and focused GREEN receipts are candidate evidence,
not ownership or release acceptance.

## Common result

The audits independently converge on the same discrepancy: Ralph/FEATURES
project accepted stories while the frozen ownership-gap and exhaustion contracts
retain null ownership and stale task/worklog bindings. Several slices also have
duplicate lane/canonical modules, nonexistent `crates/ext` task paths, and no
real application caller. Provider auth, secret, network, reconnect, cancellation,
and resource lifetime remain outside the isolated modules.

`worklog/GUARD-CONTRACT-CONFLICT.md` records why the 51 repository errors cannot
be honestly removed under the current immutable rules: the frozen validator
requires old statuses and absent task/worklog files, while accepted flags and
existing evidence may not be weakened or deleted.

## Verification

- Each source branch was pushed before integration.
- Integrated ledger JSON parsed before every commit.
- `git diff --check` passed before every commit.
- Exact integrated rerun: `python3 tools/convergence_gate.py` remains blocked
  with the same 80 findings; no new audit row created a convergence finding.
- No broad Cargo run was performed for audit-only integration.

## Audit closure

All assigned EXT/INT guard audits in this wave are integrated as blocked evidence.
