# Risk Closure Report: Storage V2 Modules

Review base: `docs/storage/REVIEW-MODULES-REAL.md` (Top-5 risks).
Source inspection: `crates/storage/src/{import_v2,execution_v2,retention_v2,writer_v2,snapshot_v2,gc_v2,quota_v2}.rs`.
Verification: `python3 tools/lane_gate.py --run`, `cargo test -p opencode-rk-storage`,
`cargo check --workspace`, `python3 -m unittest tests.bootstrap.test_storage_schema_v2`.

Live result at close: 143 Rust tests green (13 targets), 81 Python contracts OK,
workspace check clean, 12/12 lane-gate lanes PASS.

## Risk Closure Table

| # | Risk | Status | Fix and file:line | Remaining Work |
|---|---|---|---|---|
| 1 | Import silently drops message bodies (blob-backed and oversize inline become zero-byte payloads); `verify_counts` cannot detect it. | **CLOSED** | `import_v2.rs` `page`: blob-backed (`inline IS NULL`) and inline-over-ceiling rows now fail closed with `Err(StorageError::InlinePayloadTooLarge)` instead of writing a zero-byte payload. New `ImportV2::verify_payload_bytes` sums `payloads.raw_bytes` through `messages`/`message_parts` and compares to expected. Tests: `import_rejects_blob_backed_row`, `import_rejects_oversize_inline_row`, `verify_payload_bytes_accepts_exact_sum_rejects_others`. `import_is_resumable` doc now states the multi-session caveat and points callers to `import_session`. | Physical blob transfer (copy the source blob root into the destination CAS) is still not implemented; a blob-backed source is rejected, not migrated. Threading a source session id into `import_is_resumable` is still deferred. |
| 2 | `finish_attempt`/`finish_tool` unguarded terminal rewrites; `record_attempt` accepts attempts on terminal executions. | **CLOSED** | `execution_v2.rs`: `finish_attempt` `WHERE pk=?3 AND state NOT IN (2,3,5)`, `finish_tool` `WHERE pk=?5 AND state NOT IN (2,3,4)`, `record_attempt` conditional `INSERT ... WHERE EXISTS (SELECT 1 FROM executions WHERE pk=?1 AND state IN (0,1,4))`. Tests: `finish_attempt_rejects_terminal_rewrite`, `finish_tool_rejects_terminal_rewrite`, `record_attempt_rejected_on_terminal_execution`. | The `(0,5)` queued-cancel transition is untested in Rust (test gap). `executions.started_at_us` is never written (design gap). |
| 3 | Retention debt: expired receipts, terminal approvals, orphan inline payloads grow unbounded. | **CLOSED** | New `retention_v2.rs`: `sweep_expired_receipts` (keyed on `operation_id`, `retry_until_us<=now`), `sweep_resolved_approvals` (terminal 1/2/4 with `resolved_at_us<=cutoff`, resources cascade), `sweep_orphan_inline_payloads` (per-arm `NOT EXISTS` against all 7 roots, never deletes referenced), `retention_backlog`. All bounded `LIMIT clamp(1,500)`. 6 in-module tests. | No scheduled caller yet; sweeps are invoked manually/operationally. Epoch/checkpoint history still grows (no sweep). |
| 4 | `QuotaV2::admit` orphaned: no write path calls it. | **CLOSED** | `writer_v2.rs`: exported `QuotaBudget`; `append_message_checked` and `append_outbox_event_checked` call `QuotaV2::measure` then `QuotaV2::admit` and map rejection to a named `StorageError` before inserting. `append_message` signature unchanged. Tests: `checked_append_generous_budget_inserts_same_rows_as_plain`, `checked_append_zero_db_budget_rejects_and_writes_nothing`. | `ImportV2` and `AdmissionV2` still call the unchecked paths; wire them when budgets are configured. The `measure`-then-`admit` TOCTOU window is inherent and documented at the call site. |
| 5 | GC trigger-defended but untested through the module; no Rust race test. | **CLOSED** | `gc_v2.rs`: `claim_orphan_payloads` correlation bug fixed; tests `claim_orphan_payloads_deletes_old_orphan_keeps_referenced` and `claim_is_fail_closed_when_referenced` exercise the trigger path in Rust. `docs/STORAGE.md` gc row no longer says stub. | `GcV2` still has no non-test caller, and `finish_deletion` deletes only the `blobs` row (no physical file unlink) - that waits on a CAS writer. |

## Verification

```
$ python3 tools/lane_gate.py --run
PASS       gc_v2                    567 lines, all markers present
PASS       admission_v2             510 lines, all markers present
PASS       execution_v2             589 lines, all markers present
PASS       approvals_v2             424 lines, all markers present
PASS       snapshot_v2              544 lines, all markers present
PASS       import_v2                465 lines, all markers present
PASS       quota_v2                 172 lines, all markers present
PASS       retention_v2             364 lines, all markers present
PASS       test:integration_v2      2 tests
PASS       test:import_v2           4 tests
PASS       test:quota_v2            5 tests
PASS       test:perf_modules_v2     6 tests

$ cargo test -p opencode-rk-storage        # 143 passed across 13 targets
$ python3 -m unittest tests.bootstrap.test_storage_schema_v2   # Ran 81 tests ... OK
$ cargo check --workspace                  # 0 errors
```

## Not claimed

- No blob file is physically unlinked by GC.
- No scheduler invokes the retention sweeps.
- `ImportV2` rejects blob-backed rows rather than migrating them.
- Snapshot read APIs exist but no consumer uses them yet.
- Quota budgets are not yet configured/threaded into admission or import.
