# SHARE wH wave (SHARE-001..005): Debug-redaction verify + temp-stub-restore + 10-suite GREEN

## State-on-arrival (rev 248f519, workdir dirty by others)
- Owned impls already correct: `share_enterprise.rs` (refusal boundary), `share_store.rs`
  (secret-free lifecycle), `share_policy_lane.rs` / `share_policy2_lane.rs` (transport policy,
  1-line doc drift only), plus prior-wave redaction ports already in workdir:
  `share_merge.rs:87-102` `ShareRecord` manual Debug (kind+key+payload_len),
  `share_queue.rs:26-45` `ShareEvent` manual Debug (session+key+value_len).
  HEAD still has `#[derive(Debug)]` on both (leak); workdir port is the fix.
- `lib.rs` does NOT wire owned lanes (`share_enterprise`/`share_store`/`share_policy_lane`/
  `share_policy2_lane` absent); frozen suites use `#[path]` includes. Untouched throughout.
- Frozen tests + `ralph.json` untouched: `sha256sum -c /tmp/opencode/wH-frozen-prestart.sha256`
  all 10 OK before and after.

## Debug-redaction probe (log /tmp/opencode/wH-probe.log, 116 lines)
- Static: payload-carrying structs do NOT derive Debug — `ShareRecord` (merge + both policy
  lanes), `ShareEvent`, `ShareId`/`ShareSecret` (all), `ShareMeta`, `ShareStore`,
  `SyncBatch` all manual redacted impls (`payload_len`/`value_len`/prefix-only/`{..}`).
  Derives kept only on byte-free types (`RecordKind`, `MergeOutput`, `ShareError`,
  `DataKey`, `QueueCaps`, `QueueError`, `QueueStats`, outcomes, `Endpoint`, `SyncPolicy`,
  `EnterpriseOp`, `BoundaryError` partition names only).
- Exec (serial `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120`): safety/redaction suites
  GREEN — `share_merge_t05`, `share_queue` 5/5, `share_merge` 5/5, `share_store` 5/5,
  `share_enterprise` 5/5, `share_policy_lane` 5/5, `share_policy2_lane` 5/5.
  Verdict: PASS, zero payload/secret bytes in Debug output.

## Temp-stub-restore RED (impls restored byte-identical, hashes match /tmp/opencode/wH-baseline.sha256)
- SHARE-003 wrong-partition (`partition(ShareHttp) => "WRONG-PARTITION-share-http"`):
  RED 2 pass / 3 fail (`t01` :54, `t02` :68, `t04` :178), log /tmp/opencode/wH-SHARE-003-red.log.
- SHARE-004 wrong-partition (`create` overwrite instead of `AlreadyShared`):
  RED 3 pass / 2 fail (`t02` :68, `t04` :163), log /tmp/opencode/wH-SHARE-004-red.log.
  (First cap+1 probe `MAX_SHARES 1024->1025` did NOT fail — suite fills exactly to cap;
  replaced with duplicate-overwrite probe which fails correctly. `<=` seq probe also
  no-fail: no frozen test pins equal-seq; restored without log.)
- SHARE-005 classify-flip (429 => Abort): RED 4 pass / 1 fail (`t02` :58),
  log /tmp/opencode/wH-SHARE-005-red.log.
- Post-restore: enterprise `7f7e7ac4`, store `418be003`, policy_lane `10bbb9fb`,
  policy2 `0345fd4f` — identical to pre-task.

## GREEN (log /tmp/opencode/wH-share.log, 80 lines, 10 suites x5/5 = 50/50)
- `share_merge` 5/5, `share_queue` 5/5, `share_enterprise` 5/5, `share_store` 5/5,
  `share_policy_lane` 5/5, `share_policy2_lane` 5/5, `share_merge_lane` 5/5,
  `share_queue_lane` 5/5, `share_enterprise_lane` 5/5, `share_store_lane` 5/5.
  All serial JOBS=1 THREADS=1 timeout 120, EXIT=0 each. `free -h` checked each step.

## Non-touch confirmation
- NEVER touched: frozen `tests/`, `ralph.json`, `lib.rs` (owned lanes stay unwired),
  `share_merge.rs`/`share_queue.rs` (redaction-lane owned; impl-only ports pre-existing).
  Own-file diff this wave: 0 lines (stub edits reverted). Workdir diffs on tests/src are
  pre-existing rustfmt reflow by other lanes, not mine.
