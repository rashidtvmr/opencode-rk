# CONFIRM-P12 — SHARE verify-only (rev b60ceda1)

Scope: VERIFY-ONLY. No source edits. No stub edits. Owned file only: this report.
Serial: CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1, `timeout 120`, `--test-threads=1`, `free -h` checked before each suite.
Guards: frozen tests, lib.rs, ralph.json, merge/queue untouched (0 edits by this lane; `git status -- crates/sessions/` clean; `git diff --name-only` over all share src/tests + lib.rs empty; ralph.json dirt pre-existing, not mine).

## Results (all GREEN, 0 failed) — log /tmp/opencode/p12-share.log
- share_merge 5/5 (T01 last-write-wins sorted, T02 determinism, T03 secret validation, T04 caps+invalid, T05 safety/purity/cancel) EXIT=0
- share_merge_lane 5/5 EXIT=0
- share_queue 5/5 EXIT=0
- share_queue_lane 5/5 EXIT=0
- share_enterprise 5/5 EXIT=0
- share_enterprise_lane 5/5 EXIT=0
- share_store 5/5 EXIT=0
- share_store_lane 5/5 EXIT=0
- share_policy_lane 5/5 (SHARE-005 primary) EXIT=0
- share_policy2_lane 5/5 (SHARE-005 fallback) EXIT=0
- Totals: 10 suites, 50/50 PASS, 0 failed. `grep -c "test result: ok"` = 10. No FAILED/error/panicked lines.

## Stub scan
`grep -rnE "todo!|unimplemented!|panic\(\"stub|mock-" crates/sessions/src/share_*` → clean (no stubs).

## Redaction probe — PASS (PROBE_EXIT=0)
Standalone crate /tmp/opencode/p12probe (includes real src/share_merge.rs + src/share_queue.rs + serde_json dep), synthetic canaries only:
- `merge ShareRecord debug redacted: PASS :: ShareRecord { kind: Message, key: "k-PROBE-CANARY-9X7", payload_len: 29 }` (key + len visible; canary bytes + `payload:` field absent)
- `queue ShareEvent debug redacted: PASS :: ShareEvent { session: "s1", key: DataKey {...}, value_len: 27, .. }` (key + len visible; canary + `value:` absent)
- `queue CoalescingQueue counts-only: PASS :: CoalescingQueue { len: 1, bytes: 27, ... }` (no canary, no key)
- `ALL REDACTION PROBES PASSED`. Note: the 2 "canary" grep hits in p12-share.log are these probe PASS lines themselves (expected); test-output sections contain zero secret/payload bytes.

## Source state observed (read-only)
- share_merge.rs:94-102 manual `Debug` for ShareRecord renders kind+key+payload_len only; ct_eq secret compare; caps 16/10000/1MiB.
- share_queue.rs manual `Debug` for ShareEvent (value_len) + counts-only CoalescingQueue.
- share_store.rs secret-free ShareMeta/ShareStore, MAX_SHARES=1024, https-only.
- share_policy_lane.rs / share_policy2_lane.rs pure status/seq policy, no secret params, redacted Debugs.
- share_enterprise.rs typed Refused boundary, no I/O.

## Known gap (not changed, verifier-owned)
Primary T05s pin only error/log Debugs, never `format!("{:?}", ShareRecord/ShareEvent)`; lane mirrors do. Revert-to-derive on primaries stays GREEN. Fix = verifier-applied additive patch proposed in worklog/SHARE-001.md:124-178.
