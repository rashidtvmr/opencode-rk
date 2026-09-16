# SHARE-001..005 — share lanes (candidate worklog)

## Claims
- Own: `crates/sessions/src/share_merge.rs` (SHARE-001), `share_queue.rs` (SHARE-002),
  `share_enterprise.rs` (SHARE-003), `share_store.rs` (SHARE-004),
  `share_policy_lane.rs` + `share_policy2_lane.rs` (SHARE-005 primary + fallback).
  Lane-mirror copies (`*_lane.rs` src) owned by parallel fallback lanes; NOT edited here.
- Did NOT edit any frozen test file; did NOT touch `lib.rs`, `Cargo.toml`, schemas, migrations.
- Pre-existing dirty file `crates/tools/src/plugin_transform.rs` left untouched (not mine).

## Source evidence (pin 95daf90670b7c039c436c85537da5fbfe2205b41)
- `packages/enterprise/src/core/share.ts` merge + secret validation => SHARE-001
  `merge_share_records`/`apply_sync` (last-write-wins per `(kind,key)`, sorted output, ct secret eq).
- `packages/opencode/src/share/share-next.ts:112-204` coalescing queue => SHARE-002
  `CoalescingQueue::{push,drain,requeue,finalize}` (retain-on-failure deviation from upstream drop).
- `sources/enterprise-remote-spec-gap.json` (`searched-no-qualifying-in-surface-spec`) => SHARE-003
  `EnterpriseBoundary::authorize` always `Refused` with exact 5 partitions.
- `packages/core/src/share/sql.ts:1-13` + `share-next.ts:224-359` => SHARE-004 secret-free `ShareStore`.
- `share-next.ts:206-359` transport => SHARE-005 `SyncPolicy::classify`/`decide_delete`/`decide_create`,
  `request_target`, `should_send`, arithmetic backoff only.
- disc-003 `opencode.sharing` + `opencode.enterprise-remote` (both partial): pure merge/secret
  validation separated from DB persistence, migration, HTTP transport, auth, subscription
  lifetime, support-admin behavior. Classification per card: native V2 (001), partial+bounded
  deviation (002/005), safer refusal deviation (003), safer secret-free deviation (004).

## Observed scenario
- All 10 frozen suites already present in tree (`share_{merge,queue,enterprise,store}` +
  `share_{merge,queue,enterprise,store}_lane` + `share_policy_lane` + `share_policy2_lane`),
  5 tests each, `#[path]`-included src (not wired via `lib.rs` except
  `share_merge`/`share_queue`, which are `pub mod` — pre-existing integrator state).
- Fresh audit found 2 real payload-leak defects vs card contracts:
  1. `share_merge.rs` `ShareRecord` derived `Debug` printed full `payload: Vec<u8>` —
     violates SHARE-001-T05 ("no record-payload bytes beyond keys" in captured logs).
  2. `share_queue.rs` `ShareEvent` derived `Debug` printed full `value: Vec<u8>` —
     violates SHARE-002-T05 ("captured logs contain zero event-value bytes").
- Fresh audit found 2 latent payload-leak defects vs card contracts (NOT pinned by
  frozen assertions — see gaps below):
  1. `share_merge.rs` `ShareRecord` derived `Debug` printed full `payload: Vec<u8>` —
     violates SHARE-001 contract ("no record-payload bytes beyond keys" in captured logs).
  2. `share_queue.rs` `ShareEvent` derived `Debug` printed full `value: Vec<u8>` —
     violates SHARE-002 contract ("captured logs contain zero event-value bytes").
- Fix (only change this session): manual `Debug` impls rendering key + `payload_len`/
  `value_len` only (mirrors `share_policy_lane.rs` + queue/store/enterprise lanes, which were
  already redacted). No test edits. All 10 frozen suites were GREEN before AND after;
  this closes a latent contract violation the frozen assertions do not pin.

## Target boundary
- Pure sync fns, no network/DB/clock/thread/global state. Caller owns inputs/outputs.
- Caps: SHARE-001 `16/10000/1MiB` pre-map-check; SHARE-002 `4096/8MiB/256 sessions`;
  SHARE-004 `MAX_SHARES=1024`, URL 2048B, https-only; SHARE-005 arithmetic backoff cap 30_000.
- No secret logging (ct-eq, zeroize-on-drop, redacted Debugs); no user-DB writes (in-memory only).

## Tests (frozen, hashes below)
- RED: suites compile; pre-fix they passed except the leak assertions they now cover —
  the leak was a genuine GREEN-gap, not a RED re-run. No test file modified this session.
- GREEN (this session, `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`, rev 248f519d + workdir edits):
  `share_merge` 5/5, `share_queue` 5/5, `share_enterprise` 5/5, `share_store` 5/5,
  `share_policy_lane` 5/5, `share_policy2_lane` 5/5, `share_merge_lane` 5/5,
  `share_queue_lane` 5/5, `share_store_lane` 5/5, `share_enterprise_lane` 5/5.
  `cargo check -p opencode-rk-sessions` clean (no errors; no new warnings).
- Test sha256 (frozen, unmodified): share_merge e734c7bb / _lane 14e8f3c2 / share_queue 735d23a5 /
  queue_lane 2a3c0563 / enterprise 56da9c60 / enterprise_lane 81fcb932 / store 966de946 /
  store_lane da7f1b5c / policy_lane d9f9be16 / policy2_lane 5035a344.
- Src sha256 (post-fix): share_merge c97d3aab / share_queue 948cb863 / share_store d3d7f191
  / share_enterprise 7f7e7ac4 / policy_lane acc318b1 / policy2 f89ddca3.

## Decisions
- Fixed in primary `share_merge.rs`/`share_queue.rs` (the `pub mod` wired copies the
  verifier exercises via `share_merge`/`share_queue` tests). Lane mirrors already redacted.
- `git diff` also shows `crates/tools/src/plugin_transform.rs` modified — NOT mine,
  pre-existing workdir dirt; left alone per ownership rule.

## Remaining unknowns / gaps
- HONEST GAP (SUPERSEDED — re-audited 2026-09-15, see Verifier-Proposal below):
  prior note claimed no test pins the Debug fix. False for lane mirrors: lane
  T05s DO pin `format!("{:?}", record)` (share_merge_lane.rs:231-238) and
  key-only logs (share_queue_lane.rs:168-184). TRUE gap is narrower: PRIMARY
  `tests/share_merge.rs` T05 never `format!("{:?}", ShareRecord)`, PRIMARY
  `tests/share_queue.rs` T05 never `format!("{:?}", ShareEvent/queue)`. A
  revert of ONLY the primary `src/share_merge.rs`+`src/share_queue.rs` Debug
  impls stays GREEN on the primary suites (lane suites cover lane copies, not
  the `pub mod` primaries the verifier exercises). Probe + proposal below.
- `share_queue.rs` `CoalescingQueue` Debug is counts-only (stricter than contract); fine.
- Integrator must still unify `share_policy.rs` (visibility enum, unrelated) vs
  `share_policy_lane.rs`/`share_policy2_lane.rs` (SHARE-005 transport policy) naming.
- `crates/share` (`opencode-rk-share`) per card "suggested boundary" does not exist;
  lanes live in `opencode-rk-sessions` — integrator/contract decision, not a code gap.
- Upstream `sources/` trees not re-fetched this session; relied on card-cited blobs + gap files.

## Probe (2026-09-15, standalone, NOT frozen suite)
- Source: `/tmp/opencode/share_debug_probe.rs` (wrapper crate
  `/tmp/opencode/share_probe/` with `main.rs` copy, `Cargo.toml`; `#[path]`
  includes of the 5 primary src files, same pattern as frozen tests).
- Run: `cargo run --manifest-path /tmp/opencode/share_probe/Cargo.toml`
  => EXIT=0, 19 PASS lines + `ALL DEBUG-REDACTION PROBES PASSED`.
  Full output saved: `/tmp/opencode/probe_final.txt`.
- Key lines (proving no payload bytes leak, key+len only):
  - `PASS merge::ShareRecord: ShareRecord { kind: Message, key: "probe-key-1", payload_len: 36 }`
  - `PASS merge::MergeOutput: MergeOutput { records: [ShareRecord { kind: Message, key: "probe-key-1", payload_len: 36 }], skipped: 0 }`
  - `PASS queue::ShareEvent: ShareEvent { session: "s-probe", key: DataKey { kind: "message", id: "m-probe" }, value_len: 32, .. }`
  - `PASS queue::CoalescingQueue: CoalescingQueue { len: 1, bytes: 32, filtered: 0, evicted: 0, finalized: false, .. }`
  - `PASS store::ShareMeta: ShareMeta { session: ..., share_id: ShareId("sh***"), public_url: "https://share.example.com/s/item", .. }`
  - `PASS policy::SyncBatch: SyncBatch { share_id: ShareId { .. }, base_seq: 11, records: [ShareRecord { key: "k-probe-5", payload_len: 34 }] }`
- Probe asserts absence of canary bytes AND of `payload:`/`value:` field names
  (a `Vec<u8>` Debug renders as `[34, 123, ...]` under a `payload:`/`value:`
  field; note a derived-`Debug`-on-`Vec<u8>`-of-printable-bytes red herring:
  `format!("{:?}", vec![65u8])` renders `[65]`, never `"A"`, so the load-bearing
  assertion is the canary-absence check, with field-name as backstop).
- Negative-control note: a `#[derive(Debug)]` struct with `payload: Vec<u8>`
  does NOT render raw bytes as text (renders numeric list), so a "revert stays
  GREEN" demonstration must use JSON-sequence payloads; the probe's canary
  covers the exact T05 marker shape (`{"body":"<MARK>"}`), which is what leaks.

## GREEN (2026-09-15 re-run, `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`)
- `share_merge` 5/5, `share_queue` 5/5, `share_enterprise` 5/5, `share_store` 5/5,
  `share_policy_lane` 5/5, `share_policy2_lane` 5/5, `share_merge_lane` 5/5,
  `share_queue_lane` 5/5, `share_store_lane` 5/5, `share_enterprise_lane` 5/5
  (each `cargo test -p opencode-rk-sessions --test <suite>` EXIT=0).
- `cargo check -p opencode-rk-sessions` EXIT=0 (5 pre-existing `types.rs`
  dead-code warnings only, untouched by this lane).
- No frozen test file edited (verified: `git diff --name-only` shows no
  `crates/sessions/tests/*` paths).

## Verifier-Proposal (ADDITIVE frozen-test patch — verifier applies, NOT this lane)
- Rationale: lane mirrors already pin record-Debug (`share_merge_lane.rs:231-238`
  asserts `format!("{:?}", probe)` lacks `BODY_MARK`); the PRIMARY suites do not.
  Add the same pin to the two PRIMARY T05s so a revert of
  `src/share_merge.rs` / `src/share_queue.rs` Debug impls goes RED.
- Suggested diff 1 — `crates/sessions/tests/share_merge.rs`, in
  `share_merge_t05_safety_purity_cancel` after the existing `for rendered` loop
  (after line 220), insert:

```diff
@@ -217,6 +217,15 @@ fn share_merge_t05_safety_purity_cancel() {
         assert!(!rendered.contains(SECRET), "secret leak: {rendered}");
         assert!(!rendered.contains(BODY_MARK), "payload leak: {rendered}");
     }
+    // Record Debug renders key + payload length only, never payload bytes
+    // (mirrors share_merge_lane T05 lines 230-238; pins the manual Debug impl).
+    let probe = rec(
+        RecordKind::Message,
+        "m-1",
+        &format!(r#"{{"body":"{BODY_MARK}"}}"#),
+    );
+    let rendered = format!("{:?}", probe);
+    assert!(rendered.contains("m-1"), "key must stay visible: {rendered}");
+    assert!(rendered.contains("payload_len"), "len must stay visible: {rendered}");
+    assert!(!rendered.contains(BODY_MARK), "payload leak: {rendered}");
+    assert!(!rendered.contains("payload:"), "payload field must stay redacted: {rendered}");
     let out = merge_share_records(&[existing.as_slice(), incoming.as_slice()], &idle).unwrap();
     assert_eq!(out.records.len(), 2);
```

- Suggested diff 2 — `crates/sessions/tests/share_queue.rs`, in
  `share_queue_t05_safety_no_side_effects` after the `logs` leak loop
  (after line 184, before the fixture-dir check), insert:

```diff
@@ -181,6 +181,17 @@ fn share_queue_t05_safety_no_side_effects() {
     for line in &logs {
         assert!(!line.contains(MARK), "value leak: {line}");
     }
+    // Event/queue Debug renders keys + value_len only, never value bytes
+    // (pins the manual Debug impls on ShareEvent + CoalescingQueue).
+    let probe = ev("s1", "message", "m1", &format!(r#"{{"body":"{MARK}"}}"#));
+    let rendered = format!("{:?}", probe);
+    assert!(rendered.contains("m1"), "key must stay visible: {rendered}");
+    assert!(rendered.contains("value_len"), "len must stay visible: {rendered}");
+    assert!(!rendered.contains(MARK), "value leak: {rendered}");
+    assert!(!rendered.contains("value:"), "value field must stay redacted: {rendered}");
+    let mut q2 = CoalescingQueue::new(CAPS, accept_all);
+    q2.push(probe).unwrap();
+    let qrendered = format!("{:?}", q2);
+    assert!(!qrendered.contains(MARK), "queue leak: {qrendered}");
+    assert!(!qrendered.contains("m1"), "queue debug must be counts-only: {qrendered}");
     let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
     assert_eq!(entries.len(), 1);
```

- Expected effect: reverting the manual `Debug` impls in
  `src/share_merge.rs` (`ShareRecord`) or `src/share_queue.rs` (`ShareEvent` /
  `CoalescingQueue`) to `#[derive(Debug)]` turns the corresponding PRIMARY T05
  RED (canary appears under `payload:`/`value:` numeric-list rendering), while
  the current redacted impls stay GREEN. No other suite touched.

## RED-probe run (2026-09-16, SHARE-003/004/005 lane; 001/002 untouched by design)
- Scope: only non-accepted SHARE tasks (003/004/005). `share_merge.rs` /
  `share_queue.rs` owned by redaction lane: one transient revert-probe each,
  restored byte-identical (pre/post `git diff --stat` unchanged: merge +15/-2,
  queue +13/-5 vs HEAD, both pre-existing workdir dirt). No `lib.rs`,
  `ralph.json`, frozen-test, or cross-slice edits.
- Method: single-line behavior mutations in workdir src, frozen suite run,
  immediate restore + sha256 re-verify. Bounds: serial,
  `CARGO_BUILD_JOBS=2`, `--test-threads=2`, `timeout 120`, `rtk`-prefixed.
- RED results (all fail-for-behavior, then restored GREEN):
  - `share_enterprise.rs` partition `enterprise-share-http` -> `WRONG-PARTITION`:
    `share_enterprise` 2/5 (T01, T02, T04 FAILED; T03, T05 pass — no side
    effects/determinism hold regardless).
  - `share_store.rs` cap `>= MAX_SHARES` -> `>= MAX_SHARES + 1`:
    `share_store` 4/5 (T03 FAILED only; lifecycle/cascade/secrecy/determinism hold).
  - `share_policy_lane.rs` `classify` 2xx -> `Abort`: `share_policy_lane` 4/5
    (T01 FAILED only; retry/abort/delete/retention/determinism hold).
  - `share_policy2_lane.rs` same mutation: `share_policy2_lane` 4/5 (T01
    FAILED only). Restored sha `0345fd4f…` == pre-run.
- RED gap re-confirmed (pre-existing, redaction lane owns): `#[derive(Debug)]`
  restored on `ShareRecord`/`ShareEvent` (manual impls removed) => primary
  `share_merge` 5/5 and `share_queue` 5/5 still GREEN (`redm.log`/`redq.log`
  under `/tmp/opencode/`). Primary T05s pin only error/envelope Debugs, never
  `format!("{:?}", record/event)`. Verifier-Proposal diffs above still the fix.
- GREEN (post-restore): `share_enterprise` 5/5, `share_store` 5/5,
  `share_policy_lane` 5/5, `share_policy2_lane` 5/5, `share_merge` 5/5,
  `share_queue` 5/5, `share_enterprise_lane` 5/5, `share_store_lane` 5/5.
  `cargo check -p opencode-rk-sessions` clean (5 pre-existing `types.rs`
  dead-code warnings). No frozen test edited (test-file diffs vs HEAD are
  rustfmt-only dirt by other lanes).
- Src sha256 (post-run, == pre-run): enterprise `7f7e7ac4…`, store
  `418be003…`, policy_lane `10bbb9fb…`, policy2 `0345fd4f…`.
- Disclosure: transient probes touched `share_merge.rs`/`share_queue.rs`
  (redaction-lane files) for the revert check only; bytes restored exactly,
  zero lasting change. `share_enterprise.rs` has no `#![forbid(unsafe_code)]`
  (drift vs lane twin, integrator call); `share_store.rs`/`policy_lane.rs`/
  `policy2_lane.rs` carry it. `thiserror` is a workspace dep (approved).
- Gaps (integrator/controller, unchanged): `crates/share` does not exist
  (lane-variants live in `opencode-rk-sessions`); `share_policy.rs`
  (visibility enum) collides by name with SHARE-005 `share_policy_lane.rs` +
  `share_policy2_lane.rs` (twins differ by 1 doc line, both lane-owned, no
  canonical); enterprise canonical-vs-lane `thiserror`-vs-manual drift.
