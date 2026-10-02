# T1 Output Persistence Contract

## Root Cause

In `crates/storage/tests/typed_tool_history_same_batch_stop.rs`, the SQLite
trigger `same_batch_abort_a` was created without a `kind` condition:

```sql
WHEN NEW.call_id='same-batch-A'
```

This fires on **every** insert to `typed_tool_records` for call_id `same-batch-A`,
including the `kind='call'` admission row inserted by `admit_tool_round`. Since
admission is a single atomic transaction that inserts both A and B call records
and their `messages` rows, the trigger aborts the entire admission transaction
**before** any tool executes. This is the exact reason no A file effect is
observed and no A output event appears.

## Correction

Trigger narrowed to `kind='output'`:

```sql
WHEN NEW.kind='output' AND NEW.call_id='same-batch-A'
```

Source authority: `crates/storage/src/lib.rs` lines 292-297 (`append_tool_pair`
admission inserts `kind='call'` rows) and lines 430-433 (`append_tool_output`
inserts `kind='output'` rows). The `typed_tool_records` schema (line 1387)
constrains `kind IN ('call','output')`.

## Semantics After Correction

1. `admit_tool_round`: admits both A and B call rows + both messages atomically (no fault).
2. Server executes A's `write` tool -> A file effect persists.
3. Server calls `append_tool_output` for A -> trigger fires on `kind='output'` ->
   transaction rolls back -> A output row absent -> server returns `internal_error`.
4. Server stops: B never executes, B output never persisted, no follow-up provider round.

## Exact DB Assertions

- **A file effect**: present (tool executes before persistence).
- **B file effect**: absent (server returns after A output failure).
- **`messages` role='tool' count**: baseline + 2 (both A and B admitted).
- **`typed_tool_records` kind='call' for A/B**: exactly 2 rows (both admitted).
- **`typed_tool_records` kind='output' for A/B**: 0 rows (A output rolled back, B never attempted).
- **Provider requests**: exactly 1 initial + 0 follow-ups.

## Salved Worktree Provenance

Dirty worktree `storage/typed-tool-history-same-batch-stop` was NOT located
via `git worktree list --porcelain`. The worktree at path
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/APP016-same-batch-author`
was inspected but its trigger still lacks the `kind='output'` condition.
Correction implemented from base `03ef20dd` in branch `T1-correction`.

## Verification

```
OC2_TEST_BINARY=/private/var/.../install/bin/oc2 \
  RUST_TEST_THREADS=1 \
  target/debug/deps/typed_tool_history_same_batch_stop-<hash> --nocapture
```
Result: PASS (1 passed, 0 failed).

Control test `typed_tool_history_http` also PASS.

## Commit

Base: `03ef20dd`
Candidate: see `git log` (HEAD on branch `T1-correction`)
Old test SHA-256: `8d4b23eed211305386da1623eb5d3e273f38dea37d7ae5bdf85f1b1486ddf4ae`
New test SHA-256: `f61977be2b1eca0e49fc3355c63e0151351b3a6fa69c49a5773e58edfee2f0ca`
Binary verified: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/oc2-contract-6dcabf83/install/bin/oc2` SHA-256 `1586406af60ad78a178750c7289568058a8ee3b88d24c044199ab155c18b5ad5`
Rustfmt: clean. `git diff --check`: pass.
