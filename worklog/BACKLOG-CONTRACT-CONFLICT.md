# Backlog guard contract conflict: independent reproduction

Review-only evidence. No tests, validator, accepted flags or scope changed.

## Exact revision

Integrated candidate: `d382fdfd8966052c81ef7ad10a56d87314ab8a73`.

## Conflict 1: no source-ledger-only repair is possible

- `ralph.json` contains 258 stories, all with `status: accepted`.
- `tools/validate_backlog_exhaustion.py:173-198` defines five fixed category
  sets with a union of 98 IDs. Every one of these IDs is currently accepted.
- `validate_ledger:2492-2510` requires the exhaustion ledger to contain exactly
  non-accepted story IDs and forbids accepted IDs.
- `validate_ledger:2512-2522` simultaneously requires its category ID sets to
  equal those fixed sets.

Retaining the IDs violates the accepted-ID rule; removing them violates fixed
category equality. Refreshing the source JSON alone cannot resolve this.
Legacy accepted flags require trusted controller review.

## Conflict 2: frozen bootstrap baseline differs from current scope

`tests/bootstrap/test_backlog_exhaustion.py:34-48` requires a valid ledger,
219 stories, 133 accepted, 86 non-accepted, and four category counts totaling
86. The current validator derives story count from the 258-story mandatory
plan and requires five categories totaling 98.

Restoring the 219-story count would delete declared scope. Editing frozen
assertions is prohibited. This is a disputed frozen contract, not an
implementation edit.

## Independent command and result

```text
python3 -m unittest tests.bootstrap.test_backlog_exhaustion.BacklogExhaustionTests.test_current_ledger_is_valid_and_exact -v
```

One test executed, one failure at line 36; 51 validator errors. Test duration:
0.504 seconds. Supervisor exit: 1, no timeout.

Receipt directory:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/backlog-frozen-contract-review-20260930`.
Receipt SHA-256:
`a08e15cdde78ed0308023463b4f75b7f19cafe2bd2b10f155145842ea5a66b13`.

## File hashes

- Frozen test: `63a06be248a794f12850c057a4eef91801a5e1d22b076c0f0ebc2a485e2564d5`.
- Validator: `dc54eed6631c7556ef24a10ef87f5a15e934e842aa7e6479009397e8e982fd58`.
- Legacy plan: `8d1e89f91b4241a78a6d2f4188e5e73ae989889e28d8ba036c8448c84d4e3ecb`.

## Required authority decision

The independent test-author/controller and repository owner must review the
baseline transition while preserving all 258 stories, their obligations and
the original frozen test as historical evidence. Workers cannot silently
migrate the freeze, narrow scope, disable the guard or fabricate acceptance.

Run canonical repository validation and required bootstrap acceptance on the
exact reviewed revision before leasing DISC-120 or DISC-121. This report is
not approval or acceptance.
