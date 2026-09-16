# WEB-015-FINAL — workspace registry boundary evidence (VERIFY-ONLY, rev 248f519)

Claim: `GET /api/workspaces` registry-only boundary pinned by 3 suites (2+3+5=10 GREEN, serial). Story still NOT ACCEPTED per `tasks/WEB-015.md:15-18` (no workspace→session/file membership, no memory/context authority); T01-full/T02-leak/T03-a11y/T04-bounds/T05-persistence happy-path remain blocked. Frozen `web_workspace_api.rs` never edited.

## Source evidence

- `tasks/WEB-015.md:9-18` — boundary: catalog format-2 registry metadata only; no membership, no memory/context config; browser must not filter/attribute chats per workspace.
- Impl READ-ONLY: `crates/server/src/lib.rs:196-236` (`list_workspaces`) — absent → `available:false` + `workspaces:[]` + "not configured" reason; present → id/label/project_root/status/created_at_us only, plus `session_scope_available:false`, `memory_available:false`, boundary reason.
- Sessions READ-ONLY: `crates/sessions/src/lib.rs:362-391` (`with_workspace_catalog_path`, `list_workspaces`) — `CatalogV2::open_existing` + `list_workspaces(limit)`, `None` when unconfigured.
- Frozen READ-ONLY: `crates/server/tests/web_workspace_api.rs` (2 tests: T01 canonical-catalog read, T02 missing-catalog explicit unavailable) — NEVER edited.
- Owned-verify-only (no edits this lane): `crates/server/tests/web_workspace_reason.rs` (3 tests), `crates/server/tests/web_workspace_full.rs` (5 tests).
- Upstream pins: opencode `95daf90670b7c039c436c85537da5fbfe2205b41`, 9router `17c4cc76877bd1755030a8414f8d0083f48dcccf` (`sources/upstream.lock.json:9-27`). No upstream workspace reference claimed.
- `ralph.json`: WEB-015 `not-started`. Base rev `248f519` (`248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`).

## sha256 (this lane, files untouched)

- `web_workspace_api.rs`: `a1a918f5f62d7a60376b6bb7f4a071bae2aacb98f8bcdbcc1baff7e727a1bd53`
- `web_workspace_reason.rs`: `8b29dcdb0ce200cff28f25307706923acec9f4913396a45f76e144f9325006d8`
- `web_workspace_full.rs`: `2a1cbfe0dbe1ee2fc59b1ecd97844313a26ea78df26f1250755756f2f53743ac` (matches prior `worklog/WEB-015.md:78` mutation-restore hash)

## GREEN (serial JOBS=1 THREADS=1, `--test-threads=1`, `timeout 120`, `free -h` pre-checked ~2.6 GiB avail)

Full log: `/tmp/opencode/dB-web015.log` (contains api+reason+full runs, EXIT=0 each).

- `--test web_workspace_api`: 2 passed (`web_015_t01_workspace_registry_is_read_from_canonical_catalog`, `web_015_t02_missing_catalog_is_explicitly_unavailable`), 0 failed.
- `--test web_workspace_reason`: 3 passed (`web_015_reason_missing_catalog_carries_explicit_reason`, `web_015_reason_present_catalog_carries_boundary_reason`, `web_015_reason_unknown_scope_projects_no_membership`), 0 failed.
- `--test web_workspace_full`: 5 passed (`web_015_full_t01`..`t05`), 0 failed.
- Total: **10 passed, 0 failed.** All exits 0.

## Entry-point RED (s3 0/5) + uC 3/3 strategies bite + reason/full mutation kills

Per `worklog/RED-VALIDITY-WEB-PROV.md:40` + `worklog/RED-VALIDITY-WEB013-015.md:34-45` + `worklog/WEB-015.md:71,78`:

- s3: `create_project` returns `ProjectId(u64::MAX)` without insert (`crates/sessions/src/project_context.rs:171`) → RED 0 pass / 5 fail; `web015_t01` panics `:22` (`add_session` `Unavailable` on ghost id), T02-T05 same gate. Log `/tmp/opencode/s3-web-015-red.log`. GREEN restore `/tmp/opencode/s3-web-015-green.log`: 5 passed.
- uC s1: impl `session_scope_available` false→true → FAIL `web_workspace_api.rs:59 left: Bool(true) right: false`. Log `/tmp/opencode/uC-WEB015-attempt1.log`.
- uC s2: missing-catalog arm invents workspace (`[]` → `[{INVENTED}]`) → FAIL `web_workspace_api.rs:84 is_empty()`. Log `/tmp/opencode/uC-WEB015-attempt2.log`. No-invention boundary pinned.
- uC s3: `project_root` passthrough → `"/LEAKED/OTHER/WS"` → FAIL `web_workspace_api.rs:57 left: String("/LEAKED/OTHER/WS") right: "/workspace/main"`. Log `/tmp/opencode/uC-WEB015-attempt3.log`. Registry-fidelity pinned.
- uC flags/keys pin: flag-flip bites + phantom-workspace bites + field-leak bites (3/3).
- reason 3/3: owned-test `available==false`→`true` flip → FAIL `left: Bool(false) right: true` (log `/tmp/opencode/zE-ws-mut.log`); restored + GREEN 3/3.
- full 5/5: owned-test T01 `available true`→`false` flip → FAIL `left: Bool(true) right: false` (log `/tmp/opencode/bD-ws-full-mut.log`); restored + GREEN 5/5.

## Coverage vs WEB-015 T01-T05

- Frozen text (T01-T02 names from frozen api suite): registry read + missing-unavailable pinned; reason/full suites pin reason-string precision, unknown-scope no-membership, registry-fields-only, deterministic reread.
- Behaviorally covered: T02-absent-catalog (explicit unavailable, no invention) pinned by frozen T02 + reason T01/T03 + full T02/T03; T01-partial (registry fidelity) pinned by frozen T01 + full T01/T04. T01-full (scoped sessions/files/instructions + context-source breakdown), T02 cross-workspace leak, T03 a11y, T04 bounds/eviction, T05 persistence have no membership/memory authority to pin against — blocked by design, story NOT ACCEPTED.

## Verdict: Y — rationale: full-suite-pins-contract

- Serial 10/10 GREEN on current bytes + s3 entry-point RED 0/5 + uC 3/3 bites + reason 3/3 + full 5/5 mutation kills + sha256 recorded + frozen text untouched.
- Bounds: no impl/frozen/lib.rs/ralph.json edits (`lib.rs` dirty-tree `M` is parallel-lane state, zero `list_workspaces` diff lines by this lane; `web_workspace_api.rs` sha matches LIFT rerun). Owned file only: `worklog/WEB-015-FINAL.md`.
