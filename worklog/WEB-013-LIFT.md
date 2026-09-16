# WEB-013-LIFT — capabilities-contract evidence lift (VERIFY-ONLY, rev 248f519)

Claim: `GET /api/capabilities` unavailable-boundary pinned by 3 suites (1+3+5=9 GREEN, serial). Story still NOT ACCEPTED per `tasks/WEB-013.md:38` (no research executor/plan-persistence/citation adapter); T01/T03/T04/T05 happy-path/replay remain blocked. Frozen T01-T05 text unchanged (`web_capabilities_api.rs` 0-byte edit, `git status` clean) but behaviorally covered by reason + full suites.

## Source evidence

- `tasks/WEB-013.md:29-38` — manifest reports search/deep_research unavailable; grep never relabeled; no executor → NOT ACCEPTED.
- Impl READ-ONLY: `crates/server/src/lib.rs:135-194` (`web_capabilities`) — search `available:false` reason `local grep is not a web-search adapter`; deep_research `false` reason `no native research execution and citation adapter`; voice `false` reason `no native transcription or realtime audio`; tools/plugins/approvals/attachments `available_for_web_turn:false` with explicit reasons; artifacts `available:true/editing:true/run:false/apply:false`.
- Frozen READ-ONLY: `crates/server/tests/web_capabilities_api.rs` (54 lines, 1 test `web_012_t01_...`, 40-line body pp) — NEVER edited, `git status --porcelain` clean on file.
- Owned-verify-only (no edits this lane): `crates/server/tests/web_capabilities_reason.rs` (78 lines, 3 tests), `crates/server/tests/web_capabilities_full.rs` (126 lines, 5 tests).

## sha256 (all 3, this lane)

- `web_capabilities_api.rs`: `d98546283281a26e89d42fa39b8dfcf88229cc0f620e6b6b48e9269dce4bf984`
- `web_capabilities_reason.rs`: `34a483242f7ec4b2c03349348e9869980a755642da4d6edaaa5151de801043bc`
- `web_capabilities_full.rs`: `39a79a90e8cd002fc1b67a4ea7076594bf3915941925b4f948c5c82a055af938`

## GREEN (serial JOBS=1 THREADS=1, `--test-threads=1`, `timeout 120`, `free -h` pre-checked ~2.5 GiB avail)

Full log: `/tmp/opencode/cA-web013.log` (contains api+reason+full runs, EXIT=0 each).

- `--test web_capabilities_api`: 1 passed (`web_012_t01_daemon_reports_real_tools_and_unavailable_web_adapters`), 0 failed.
- `--test web_capabilities_reason`: 3 passed (`..._non_empty_reasons`, `..._names_adapter_boundary`, `..._unknown_scope_is_not_silently_available`), 0 failed.
- `--test web_capabilities_full`: 5 passed (`web_cap_full_t01_available_flags_pinned`, `t02_reasons_non_empty`, `t03_unknown_scope_never_available`, `t04_surfaces_pinned`, `t05_response_deterministic`), 0 failed.
- Total: **9 passed, 0 failed.** All exits 0.

## Entry-point RED (mutation, /tmp copies only, repo restored byte-identical)

Per `worklog/RED-VALIDITY-WEB013-015.md:17-20,23-26` + `worklog/WEB-013.md:66,84`:

- s1: impl `search.available` + `deep_research.available` false→true → FAIL `web_capabilities_api.rs:49 left: Bool(true) right: false`. Log `/tmp/opencode/uC-WEB013-attempt1.log`.
- s2 (valid survival, records gap): reason-text-only edit → PASS; closed by reason-suite probes (search~grep, research~citation, voice~transcription/audio).
- s3: renamed key `search`→`search_missing` → FAIL `web_capabilities_api.rs:49 left: Null right: false` (missing-key IS pinned). Log `/tmp/opencode/uC-WEB013-attempt3.log`.
- uC flags/keys pin: flag-flip bites + key-drop bites (see s1/s3).
- reason 3/3: owned-test `available==false`→`true` flip → FAIL (log `/tmp/opencode/zE-cap-mut.log`); restored + GREEN 3/3.
- full 5/5: owned-test `artifacts.available true`→`false` flip → FAIL T01 `left: Bool(true) right: false` (log `/tmp/opencode/bC-cap-full-mut.log`); restored + GREEN 5/5.

## Coverage vs WEB-013 T01-T05

- Frozen text unchanged (T01-T05 names from `tasks/WEB-013.md:7`): no frozen T01-T05 suite exists in repo.
- Behaviorally covered: T02 (unavailable-means-unavailable) pinned by frozen api test + reason T01/T02/T03 + full T01/T02/T03/T04; T01/T03/T04/T05 happy-path/a11y/bounds/replay have no executor to pin against — blocked by design, story NOT ACCEPTED.

## Verdict: y — rationale: full-suite-pins-contract

- Serial 9/9 GREEN on current bytes + entry-point RED bites (s1/s3) + uC flags/keys pin + reason 3/3 + full 5/5 mutation kills + sha256 recorded + frozen text untouched.
- Bounds: no impl/frozen/lib.rs/ralph.json edits (`lib.rs` dirty-tree `M` is parallel-lane `pub mod` wiring, zero `web_capabilit` diff lines, untouched). Dirty-tree `??` reason/full files are sibling-lane owned, verify-only here.
