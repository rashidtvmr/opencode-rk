# SHARE xI wave (SHARE-001..005): Debug-redaction verify + temp-stub-restore + 10-suite GREEN

## State-on-arrival (rev 248f519, workdir dirty by others)
- Owned impls correct, unchanged this wave: `share_enterprise.rs` (`7f7e7ac4`, refusal
  boundary), `share_store.rs` (`418be003`, secret-free lifecycle), `share_policy_lane.rs`
  (`10bbb9fb`) / `share_policy2_lane.rs` (`0345fd4f`, transport policy, fmt-only drift).
- Pre-existing workdir ports (not mine): `share_merge.rs` `ShareRecord` manual Debug
  (kind+key+payload_len), `share_queue.rs` `ShareEvent` manual Debug (session+key+value_len).
  HEAD has `#[derive(Debug)]` on both; workdir port is the fix. Left untouched.
- `lib.rs` wires only `share_merge`/`share_queue` (lines 23/25); owned lanes
  (`share_enterprise`/`share_store`/`share_policy_lane`/`share_policy2_lane`) stay unwired,
  frozen suites use `#[path]` includes. Untouched throughout.
- Frozen `tests/`, `ralph.json` untouched (test-file diffs in workdir are other lanes'
  rustfmt reflow; `git status --short -- ralph.json` clean; temp probe file deleted).
- Own-file diff this wave: 0 lines (3 temp stubs reverted, `sha256sum -c` all 6 OK).

## Debug-redaction probe (log /tmp/opencode/xI-probe.log, 9 lines, EXIT=0)
- Static: payload-carrying structs never derive Debug. Manual redacted impls confirmed:
  `share_merge.rs` ShareId/ShareSecret/ShareRecord; `share_queue.rs` ShareEvent/CoalescingQueue;
  `share_store.rs` ShareId/ShareSecret/ShareMeta/ShareStore; both policy lanes
  ShareId/ShareRecord/SyncBatch. Derives kept only on byte-free types (RecordKind,
  MergeOutput, ShareError, DataKey, QueueCaps/Stats, outcomes, Endpoint, SyncPolicy,
  EnterpriseOp, BoundaryError partition names).
- Exec (temp `tests/xi_probe_tmp.rs`, deleted after run, serial JOBS=1 THREADS=1 timeout 120):
  1/1 PASS. Asserted zero SECRET/PAYLOAD/VALUE/URL-query/URL-fragment/id-suffix bytes in all
  10 Debug renders; `payload_len`/`value_len` markers present (shape, not bytes).

## Temp-stub-restore RED (impls restored byte-identical, `sha256sum -c xI-baseline.sha256` 6/6 OK)
- SHARE-003 wrong-partition (`partition(ShareHttp) => "WRONG-PARTITION-share-http"`):
  RED 2 pass / 3 fail (t01, t02, t04), log /tmp/opencode/xI-SHARE-003-red.log (76 lines).
- SHARE-004 duplicate-overwrite (drop `AlreadyShared` guard): RED 3 pass / 2 fail (t02, t04),
  log /tmp/opencode/xI-SHARE-004-red.log (65 lines).
- SHARE-005 classify-flip (429 falls to Abort): RED 4 pass / 1 fail (t02),
  log /tmp/opencode/xI-SHARE-005-red.log (63 lines).
- No TEMP-STUB/WRONG-PARTITION strings remain in `crates/sessions/src/`.

## GREEN (log /tmp/opencode/xI-share.log, 490 lines, 10 suites x5/5 = 50/50, all EXIT=0)
- share_merge, share_queue, share_enterprise, share_store, share_policy_lane,
  share_policy2_lane, share_merge_lane, share_queue_lane, share_enterprise_lane,
  share_store_lane. All serial JOBS=1 THREADS=1 timeout 120; `free -h` checked each step
  (avail stayed >2Gi; no swap stall).

## Non-touch confirmation
- NEVER touched: frozen `tests/`, `ralph.json`, `lib.rs`, `share_merge.rs`/`share_queue.rs`
  (redaction ports pre-existing, impl-only, <30 lines each — no new port needed).
