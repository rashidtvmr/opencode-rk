# WEB-014 + WEB-017 suite verdict — boundary-only claim REFUTED, no new files

Base rev `248f519`. Cards REQUIRE 5-test suites (`tasks/WEB-014.md:7`,
`tasks/WEB-017.md:7`, T01..T05 contract `tasks/WEB-014.md:17-28`,
`tasks/WEB-017.md:17-28`). "2-test" files are supplemental HTTP API
boundary coverage, NOT the T01-T05 suites. Full suites exist, frozen, GREEN.

## Evidence

- WEB-014 T01-T05: `crates/sessions/tests/chat_nav_lane.rs` (250 lines, 5 tests:
  `web014_t01`..`web014_t05`, impl `crates/sessions/src/chat_nav_lane.rs` 427 lines).
  Impl hash `96f79cef53d40e3276c52562ab3328da7b3e0ae28f5e46d0a047829c68a3317f`;
  test hash `83e16f9bb20b6a7f45de19c5bf7b81b3c7726811f41761063c3159bd060249ad`
  (match `worklog/RED-VALIDITY-WEB-PROV.md:17,27`).
- WEB-017 T01-T05: `crates/server/tests/web_artifact.rs` (186 lines, 5 tests:
  `artifact_t01`..`artifact_t05`, impl `crates/server/src/web_artifact.rs` 511 lines).
  Impl hash `7f9f1382f83b4b7826a617d224ae30ede51153bbadfa92a1e922275c1bfad386`;
  test hash `19aa65d4d17bb29dcaf2356e91fbe6a7c3c1735b5b9a36a658a375396d2dbb71`
  (match RED-VALIDITY worklog).
- Supplemental HTTP boundary: `crates/server/tests/session_history_api.rs`
  (161 lines, 2 tests) + `crates/server/tests/web_artifact_api.rs` (198 lines, 2 tests).
- RED receipts (`worklog/RED-VALIDITY-WEB-PROV.md:39,42`): WEB-014 stub
  (`upsert_chat`→`NotFound`) → 3 pass/2 fail; WEB-017 stub
  (`open_artifact`→`DocumentEmpty`) → 1 pass/4 fail. Partial-fail = valid
  suite-bite (negative-path/a11y tests need no happy-path entry).
- Prior `worklog/WEB-014.md` (2-test claim) is STALE — predates RED-VALIDITY lane.

## Verify runs (serial, JOBS=2 THREADS=2, timeout 110, this lane, GREEN)

- `cargo test -p opencode-rk-sessions --test chat_nav_lane` → 5 passed, 0 failed.
- `cargo test -p opencode-rk-server --test web_artifact` → 5 passed, 0 failed.
- `cargo test -p opencode-rk-server --test session_history_api --test web_artifact_api`
  → 2+2 passed, 0 failed.

## Verdict

Cards require 5-test suites AND 5-test suites exist covering every T01-T05
contract point. Condition for additive file (only-2-exist) is FALSE.
No files created, no frozen tests/lib.rs/ralph.json touched. Target boundary:
this worklog only. Story acceptance stays with verifier (browser
regression runner-unverified per cards).
