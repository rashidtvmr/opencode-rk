# SHARE-DEDUP: share_* vs share_*_lane twin audit

## Claim

No `pub use` re-export shim conversion performed. Lane twins use
incompatible public symbol names (`Lane*`, `LANE_*`, `merge_lane_records`,
`apply_lane_sync`, `validate_lane_secret`, `LaneCoalescingQueue`, ...),
so a `pub use canonical::*` shim exposes zero symbols the frozen
`#[path]` lane tests import. Conversion without editing frozen tests is
impossible. Twins kept as-is (read-only verify + diff record only).

## Source evidence (rev `248f519` + workdir)

- Diff record: `/tmp/opencode/share_dedupe.diff` (599 lines, 26 `@@`
  hunks, 246 `+`/`-` lines). Regenerator:
  `/tmp/opencode/regen_diff.sh`. Pairs:
  `share_merge`, `share_queue`, `share_store`, `share_enterprise`
  (canonical vs `*_lane`), plus `share_policy_lane` vs
  `share_policy2_lane`.
- Canonical `share_merge.rs:146` exports `validate`,
  `:168` `merge_share_records`, `:212` `apply_sync`,
  `RecordKind/Message/Model/Part/Session/SessionDiff`,
  `ShareId/ShareSecret/ShareRecord/MergeOutput/ShareError`,
  `MAX_BATCHES/MAX_RECORDS_PER_BATCH/MAX_RECORD_BYTES`.
- Lane `share_merge_lane.rs:147` exports `validate_lane_secret`,
  `:172` `merge_lane_records`, `:216` `apply_lane_sync`,
  `LaneRecordKind/LaneMessage/...`, `LaneShareId/...`,
  `LANE_MAX_*`. Frozen `tests/share_merge_lane.rs:8-12` imports
  exactly the `Lane*` names + `apply_lane_sync, merge_lane_records`.
  Same pattern for queue (`LaneCoalescingQueue, LaneDataKey,
  LaneQueueCaps, LaneQueueError, LaneShareEvent` in
  `tests/share_queue_lane.rs:8-10`).
- Normalized diff (`s/Lane//; s/LANE_//` etc.) leaves TRUE drift:
  canonical `ShareRecord`/`ShareEvent` still `#[derive(Debug)]`
  (leaks payload/value bytes); lane twins carry manual redacted
  `Debug` (`payload_len`/`value_len`). Canonical `share_enterprise.rs`
  lacks `#![forbid(unsafe_code)]`, uses `thiserror::Error`; lane uses
  manual `std::fmt::Display + std::error::Error`. `share_store` pair is
  comment-only drift; `policy_lane` vs `policy2_lane` is 1-line doc
  drift (md5 `c5316158` vs `fe13225b`, both 231 lines).
- No outside users: `grep -rln <symbol>` over `crates/` outside
  `crates/sessions/src/share_*` + `crates/sessions/tests/share_*`
  returns none. `tools/lane_gate.py` has no `share` entries.
  `lib.rs:23-25` wires only `share_merge`, `share_policy`
  (visibility enum, unrelated to SHARE-005 transport policy),
  `share_queue`. All 10 frozen suites use `#[path]` includes, never
  `opencode_rk_sessions::share_*` — zero lib impact either way.
- EXT-DEDUP precedent (`worklog/EXT-DEDUP.md`) shimmed only
  byte-identical twins (same symbol names). Not applicable here.

## Why shim rejected (would break frozen tests)

- `#[path] mod share_merge_lane; use share_merge_lane::{LaneShareId,
  ...}` resolves symbols against the shim file's own re-exports.
  Replacing `share_merge_lane.rs` body with `#[path =
  "share_merge.rs"] mod canonical; pub use canonical::*;` exports
  `ShareId`, not `LaneShareId` => `unresolved imports` compile fail
  in the frozen lane test. Same for queue/store/enterprise/policy
  twins. Adding alias `pub use ShareId as LaneShareId` etc. is real
  code in the lane file (type aliases, not zero-cost dedup) and still
  diverges on `validate` vs `validate_lane_secret`,
  `merge_share_records` vs `merge_lane_records` signatures — needs
  wrapper fns, i.e. a second implementation, defeating the dedup.
- `share_policy.rs` (wired, `Visibility/parse_visibility`) vs
  `share_policy_lane.rs`/`share_policy2_lane.rs` (SHARE-005 transport
  `SyncPolicy/classify/decide_delete/...`) are different contracts;
  no shim direction valid. `policy_lane` vs `policy2_lane` are
  identical twins but BOTH are lane-owned (`#[path]` tests
  `share_policy_lane.rs:5-6`, `share_policy2_lane.rs:6-7`); no
  canonical owner exists, and only one (`share_policy_lane`) matches
  a task-card suggested boundary. Unification needs
  integrator+controller call (delete one + migrate its frozen test).

## Tests (GREEN, JOBS=2 THREADS=2 timeout 120 rtk, files untouched)

- `share_merge` 5/5, `share_queue` 5/5, `share_store` 5/5,
  `share_enterprise` 5/5, `share_policy_lane` 5/5,
  `share_policy2_lane` 5/5, `share_merge_lane` 5/5,
  `share_queue_lane` 5/5, `share_store_lane` 5/5,
  `share_enterprise_lane` 5/5 (each `cargo test -p
  opencode-rk-sessions --test <suite>` EXIT=0, `rtk`-filtered
  `cargo test: 5 passed (1 suite, ...)`).
- `cargo check -p opencode-rk-sessions` EXIT=0, 0 errors, 5
  pre-existing `types.rs` dead-code warnings.
- No `src/` or `tests/` file modified (`git status --short` clean
  for all 20 share paths). Frozen `#[path]` tests byte-identical.

## Remaining gaps (integrator/controller)

- Canonical `share_merge.rs:86-92` `ShareRecord` + `share_queue.rs:25`
  `ShareEvent` `#[derive(Debug)]` still leak payload/value bytes;
  lane twins already redacted. Fix = port lane `Debug` impls into
  canonicals (primary-lane convergence, not shim). Frozen primary
  T05s do not pin `format!("{:?}", record)` (see
  `worklog/SHARE-001.md` Verifier-Proposal) — port + add pin.
- Canonical `share_enterprise.rs` missing `#![forbid(unsafe_code)]`
  + `thiserror` vs manual-Display drift vs lane; converge on one.
- `share_policy.rs` naming collision vs SHARE-005 policy lanes;
  rename or re-scope before any wire-up.

## Re-verify 2026-09-16 (rev `248f519`, workdir dirty by others)

- Tests: 10/10 suites 5/5 = 50/50 GREEN, serial JOBS=2
  THREADS=2 `timeout 120` `rtk`-prefixed. Log
  `/tmp/opencode/w8-share.log` (`cargo test: 5 passed (1
  suite, ...)` each). `cargo check -p opencode-rk-sessions`:
  0 errors, 5 pre-existing `types.rs` dead-code warnings.
- Redaction: FAILS on canonicals. `share_merge.rs:86`
  `ShareRecord` still `#[derive(Debug)]` (full `payload`
  bytes). `share_queue.rs:25` `ShareEvent` still
  `#[derive(Debug)]` (full `value` bytes). Lane twins
  redacted (`LaneShareRecord` `payload_len`,
  `LaneShareEvent` `value_len`). No ports/aliases added
  by this run; workdir `share_queue*.rs` diffs are
  rustfmt-only (if-collapse), frozen tests show
  rustfmt-only reflow, no behavior change.
- Twin verdict: UNCHANGED — Lane-symbol incompatible.
  `share_merge`/`share_queue` fns+types all `Lane*`/
  `LANE_*` prefixed; a `pub use` shim exports zero
  symbols frozen `#[path]` lane tests import. `store`
  pair byte-identical modulo 2 doc lines (SAME symbols,
  convertible — needs integrator call). `enterprise`
  pair same symbols, drift = `forbid(unsafe_code)` +
  `thiserror` vs manual Display (convertible —
  converge first). `policy_lane` vs `policy2_lane`
  1-line doc drift only (md5 `c5316158`/`fe13225b`),
  both lane-owned, no canonical owner. NO twins
  deleted (integrator/controller decision).
- Workdir dirty (not mine): `git status` shows ~30
  modified files incl. share tests/src (rustfmt
  reflow). `worklog/SHARE-DEDUP.md` untracked (this
  file). No share `src/`/`tests/` content edits by
  this lane beyond this worklog append.

## S0 Debug-redaction port (canonical ShareRecord/ShareEvent)
- Ported lane Debug impls: `ShareRecord` (share_merge.rs:86) now
  kind+key+payload_len; `ShareEvent` (share_queue.rs:25) now
  session+key+value_len. Impl-only, 26 insertions/7 deletions,
  both <30 lines. Matches lane twins byte-for-byte modulo names.
- Probe (disposable tests/zz_probe_debug_redact.rs, removed after):
  RECORD_DEBUG=`ShareRecord { kind: Message, key: "k1", payload_len: 26 }`,
  EVENT_DEBUG=`ShareEvent { session: "s1", key: ..., value_len: 24, .. }`;
  no SECRET_* bytes, no raw `payload:`/`value:` fields. PASS.
- 10 suites x5/5 GREEN, log /tmp/opencode/s0-share.log;
  `cargo check -p opencode-rk-sessions` 0 errors.
- Hashes: share_merge.rs 8be48c94, share_queue.rs a295125a.

## SHARE-003/004/005 temp-stub-restore re-verify 2026-09-16 (rev `248f519`+dirty, JOBS=2 THREADS=2 timeout 120 rtk)
- Owned ONLY share_enterprise.rs, share_store.rs, share_policy_lane.rs, share_policy2_lane.rs. Untouched: share_merge.rs/share_queue.rs (redaction lane), frozen tests, lib.rs, ralph.json. All 4 files restored byte-identical to pre-task state (cmp vs /tmp/shareback OK; md5 enterprise 3f3ba192, store b97d7929, policy_lane c5316158, policy2 fe13225b). Workdir `git diff` on store/policy_lane/policy2_lane = rustfmt-only reflow, pre-existing, not mine.
- RED stubs (1-line each, then restored): enterprise `partition(ShareHttp)=>"...-WRONG"`; store `>=MAX_SHARES` to `>MAX_SHARES` (cap+1); policy_lane `classify` 2xx `Sent` to `Abort{classify-flip}`; policy2 `CAP_MS` 30_000 to 30_001.
- RED: enterprise 2/3 FAIL (`t01 left "...-WRONG"/right "..."`, `t02 :68`, `t04 :178`) log /tmp/opencode/rF-share_enterprise-red.log; store 4/1 FAIL (`t03 :137 unwrap_err on Ok ShareMeta`, cap+1 accepts 1025th) log /tmp/opencode/rF-share_store-red.log; policy_lane 4/1 FAIL (`t01 :34 left Abort{classify-flip-200}/right Sent`) log /tmp/opencode/rF-share_policy_lane-red.log; policy2 4/1 FAIL (`t02 :81 left 30001/right 30000`) log /tmp/opencode/rF-share_policy2_lane-red.log. All RED logs compile + fail on missing behavior (no stub committed).
- GREEN 5/5 each after restore + lane mirrors: share_enterprise, share_store, share_policy_lane, share_policy2_lane, share_enterprise_lane, share_store_lane. Logs /tmp/opencode/rF-<suite>-green.log (`cargo test: 5 passed (1 suite, ...)`). `cargo check -p opencode-rk-sessions` EXIT=0, 0 errors, 5 pre-existing types.rs dead-code warnings, log /tmp/opencode/rF-share-check.log.
- Twin verdict UNCHANGED per file header claim: enterprise canonical lacks forbid(unsafe_code)+thiserror vs lane manual Display (same symbols); store pair comment-only drift (same symbols); policy_lane vs policy2_lane 1-line doc drift, both lane-owned, no canonical owner. No shim/deletes (integrator call).

## uE redaction-arbitration 2026-09-16 (SOlE WRITER share_merge.rs/share_queue.rs)
- State-on-arrival: ALREADY PORTED in workdir (s0 correct, w8 stale vs HEAD). Workdir share_merge.rs:94 + share_queue.rs:33 carry redacted impls (payload_len/value_len); HEAD still derive(Debug) on both (HEAD merge:86 derive, queue:25 derive). Remaining derive(Debug)s legit: RecordKind/MergeOutput/ShareError/DataKey/QueueCaps/QueueError/QueueStats (no bytes).
- Diff owned by this lane this wave: NONE (0 lines; probe file created+removed only). Workdir-vs-HEAD diff on owned files = s0 port (+13/-2 merge, impl-only <30 lines) + redacted queue impl; verify via `git diff -- crates/sessions/src/share_merge.rs | head -40`.
- Probe: disposable tests/zz_uE_probe.rs (removed after): RECORD_DEBUG=`ShareRecord { kind: Message, key: "k1", payload_len: 42 }`, EVENT_DEBUG=`ShareEvent { session: "s1", key: DataKey {...}, value_len: 42, .. }`, ID/SECRET_DEBUG=`{ .. }`, QUEUE_DEBUG=`{ len: 1, bytes: 42, filtered: 0, evicted: 0, finalized: false, .. }`; assert no SUPERSECRET/TOPSECRET/s3cr3t in any Debug, payload_len/value_len present, no raw payload:/value: fields. PASS 1/1, log tail in /tmp/opencode/uE-probe-raw.log.
- Suites: 10 share suites x5/5 GREEN (audit, count, expiry, invite, links, list, merge, merge_lane, queue, queue_lane), log /tmp/opencode/uE-share.log EXIT=0. `cargo check -p opencode-rk-sessions` 0 errors, 5 pre-existing lib warnings, log /tmp/opencode/uE-check.log.
- Untouched: frozen tests/lib.rs/ralph.json (probe removed; `git status` probe path clean). Serial JOBS=2 THREADS=2 timeout 120 rtk free -h (avail 2.6Gi pre-run, 2.9Gi post).
- Hashes: share_merge.rs 4cb6c492, share_queue.rs 048744e1.
