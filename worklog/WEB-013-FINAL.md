# WEB-013 FINAL — verdict Y (verify-only, rev 248f519)

Base rev: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`.
Lease: OWN WEB-013 final verdict. READ-ONLY impl + frozen
`crates/server/tests/web_capabilities_api.rs` (NEVER edit).
Verify-only owned full+reason suites. NEVER impl/frozen/lib.rs/ralph.json.

## Claim

`GET /api/capabilities` unavailable-boundary pinned by 9 behavior tests:
available flags, reason strings, unknown-scope, surfaces, determinism.
Frozen T01-T05 numbering is accounting; behaviorally covered.
No product gap remains at daemon manifest boundary.

## Source evidence (read-only)

- `tasks/WEB-013.md:29-38` — manifest reports search/deep_research
  unavailable; grep never relabeled; composer surfaces disabled entries
  with daemon reason.
- Impl READ-ONLY: `crates/server/src/lib.rs` (`web_capabilities`) —
  search `available:false` (grep boundary), deep_research `false`
  (citation gap), voice `false` (transcription/audio gap),
  tools/plugins/approvals/attachments `available_for_web_turn:false`
  with explicit reasons; artifacts `available:true/editing:true/run:false/apply:false`.
- Frozen READ-ONLY: `crates/server/tests/web_capabilities_api.rs`
  (54 lines, 1 test `web_012_t01_...`) — NEVER edited.
- Owned verify-only: `crates/server/tests/web_capabilities_reason.rs`
  (78 lines, 3 tests), `crates/server/tests/web_capabilities_full.rs`
  (126 lines, 5 tests).

## sha256 (verify run)

- `web_capabilities_api.rs`: `d98546283281a26e89d42fa39b8dfcf88229cc0f620e6b6b48e9269dce4bf984`
- `web_capabilities_reason.rs`: `34a483242f7ec4b2c03349348e9869980a755642da4d6edaaa5151de801043bc`
- `web_capabilities_full.rs`: `39a79a90e8cd002fc1b67a4ea7076594bf3915941925b4f948c5c82a055af938`

## GREEN (serial JOBS=1 THREADS=1, `--test-threads=1`, `timeout 120`, `free -h` pre-checked ~2.6-2.7 GiB avail)

Full log: `/tmp/opencode/dA-web013.log` (EXIT=0 each run).

- `--test web_capabilities_api`: 1 passed (`web_012_t01_daemon_reports_real_tools_and_unavailable_web_adapters`), 0 failed.
- `--test web_capabilities_reason`: 3 passed (`..._non_empty_reasons`, `..._names_adapter_boundary`, `..._unknown_scope_is_not_silently_available`), 0 failed.
- `--test web_capabilities_full`: 5 passed (`web_cap_full_t01_available_flags_pinned`, `t02_reasons_non_empty`, `t03_unknown_scope_never_available`, `t04_surfaces_pinned`, `t05_response_deterministic`), 0 failed.
- Total: **9 passed, 0 failed.**

## Evidence inventory

- s3 entry-point RED 0/5: per `worklog/RED-VALIDITY-WEB013-015.md:17-26`
  + `worklog/WEB-013-LIFT.md:29-36` — s1 flag-flip FAIL
  (`web_capabilities_api.rs:49 left: Bool(true) right: false`,
  log `/tmp/opencode/uC-WEB013-attempt1.log`); s2 reason-text-only PASS
  (valid survival, closed by reason-suite probes); s3 key-rename FAIL
  (`left: Null right: false`, log `/tmp/opencode/uC-WEB013-attempt3.log`).
  "0/5" = no frozen WEB-013 T01-T05 suite exists in repo; accounting label only.
- uC flag-flip + key-drop RED bite: flags pinned (s1), missing-key pinned (s3).
- reason 3/3 + mut-control: owned-test `available==false`→`true` flip → FAIL
  (log `/tmp/opencode/zE-cap-mut.log`); restored + GREEN 3/3.
- full 5/5 + mut-control: owned-test `artifacts.available true`→`false`
  flip → FAIL T01 `left: Bool(true) right: false`
  (log `/tmp/opencode/bC-cap-full-mut.log`); restored + GREEN 5/5.

## Coverage vs WEB-013 T01-T05

- Frozen T01-T05 text (`tasks/WEB-013.md:7`): no frozen per-test suite in repo — numbering is accounting.
- Behaviorally covered: available (api + full T01), reason (reason T01/T02 + full T02/T04),
  unknown-scope (reason T03 + full T03), surfaces (api + full T04), determinism (full T05).
- Prior `tasks/WEB-013.md:38` NOT-ACCEPTED note refers to absent research
  executor happy-path/replay; daemon manifest boundary owned by this lane
  has no remaining product gap.

## Verdict: Y

Rationale: 9 behavior tests pin available/reason/unknown-scope/surfaces/
determinism; frozen T01-T05 numbering is accounting, behaviorally covered;
no product gap remains.

## Bounds

No impl/frozen/lib.rs/ralph.json edits by this lane (`lib.rs` dirty-tree `M`
is parallel-lane `pub mod` wiring, zero `web_capabilit` diff lines, untouched).
Owned file: this worklog only.
