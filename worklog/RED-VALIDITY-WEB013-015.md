# RED-VALIDITY WEB-013 + WEB-015 — stub-bite investigation (2026-09-16)

Base rev: `248f519`. Workdir dirty from parallel lanes (205 files changed), but
owned files verified byte-identical to pristine copies before/after every attempt:
`sha256 lib.rs=4d8136fe…`, `web_capabilities_api.rs=d9854628…`,
`web_workspace_api.rs=a1a918f5…` (note: `web_workspace_api.rs` carries a pre-existing
working-tree-only import reformat, +4/-1, from another lane — NOT mine, never touched).
Impl: `crates/server/src/lib.rs:135-194` (`web_capabilities`), `:196-236` (`list_workspaces`).
Tests: `crates/server/tests/web_capabilities_api.rs` (1 test, checks `search`/`deep_research`/`voice`
`available==false`, tools `available_for_web_turn==false`), `web_workspace_api.rs` (T01 canonical-catalog
read + `session_scope_available==false`/`memory_available==false`; T02 missing catalog → `available==false` + empty list).
Prior claim "RED-VALIDITY 0/5 fail" NOT reproduced for these suites — every semantically meaningful stub bites
on attempt 1. The `false` flags ARE pinned; precise reason-STRINGS mostly are NOT.

## WEB-013 verdict: PINNING = YES (unavailable-boundary; reason-string precision = NO)

- Attempt 1 — `web_capabilities()`: `search.available` + `deep_research.available` false→true.
  → FAIL bites: `panicked at web_capabilities_api.rs:49:5, left: Bool(true), right: false`.
  Log: `/tmp/opencode/uC-WEB013-attempt1.log` (extracted from `/tmp/opencode/stub1-full.log`).
  Valid RED recipe: flip either flag to `true` → test fails. Restored byte-identical (sha256 match).
- Attempt 2 — reason text only: `"local grep is not a web-search adapter"` → `"x"`.
  → PASS (no bite). Test asserts only `value["search"]["available"]==false`, never the reason string.
  The same trivially passes for any reason text. Log: `/tmp/opencode/uC-WEB013-attempt2.log`.
- Attempt 3 — renamed key `"search"` → `"search_missing"`.
  → FAIL bites: `web_capabilities_api.rs:49:5, left: Null, right: false`.
  Log: `/tmp/opencode/uC-WEB013-attempt3.log`. Missing-key regression IS pinned (indexing `Null != false`).
- GREEN restore: pristine `lib.rs` → `web_capabilities_api` 1 passed. Verified serially, JOBS=2 THREADS=2.

Scope note: test is named `web_012_t01…` and covers the whole capabilities manifest (tools/plugins/approvals/
attachments/search/deep_research/voice). WEB-013 T01 (lifecycle happy path) / T03 (a11y) / T04 (bounds-cancel) /
T05 (replay) have NO dedicated server test — no executor/plan/persistence exists per `tasks/WEB-013.md:29-38`.
So: unavailable-boundary pinning = YES; full T01–T05 contract = NOT pinned (nothing to pin against yet).

## WEB-015 verdict: PINNING = YES (3/3 stub strategies bite)

- Attempt 1 — `list_workspaces()`: `"session_scope_available": false` → `true` (both arms).
  → FAIL bites: `panicked at web_workspace_api.rs:59:5, left: Bool(true), right: false` (T01; T02 still ok).
  Log: `/tmp/opencode/uC-WEB015-attempt1.log`. Restored byte-identical.
- Attempt 2 — missing-catalog arm invents workspace (`"workspaces": []` → `[{INVENTED…}]`).
  → FAIL bites: `panicked at web_workspace_api.rs:84:5, assertion failed: body["workspaces"]…is_empty()`
  (T02; T01 still ok). Log: `/tmp/opencode/uC-WEB015-attempt2.log`. No-invention boundary IS pinned.
- Attempt 3 — `"project_root": workspace.project_root` → `"/LEAKED/OTHER/WS"`.
  → FAIL bites: `panicked at web_workspace_api.rs:57:5, left: String("/LEAKED/OTHER/WS"), right: "/workspace/main"`.
  Log: `/tmp/opencode/uC-WEB015-attempt3.log`. Registry-fidelity IS pinned.
- GREEN restore: pristine `lib.rs` → `web_workspace_api` 2 passed. Verified serially, JOBS=2 THREADS=2.

Scope note: pinned surface is registry-metadata-only (T01-partial + T02-absent-catalog). T01 full
(scoped sessions/files/instructions + context-source breakdown), T02 cross-workspace leak, T03 a11y, T04
bounds/eviction, T05 persistence have NO server test — no membership/memory authority exists per
`tasks/WEB-015.md:9-18`. Those remain non-pinning by absence, not by weakness.

## Non-pinning gaps + additive (NEW-FILE, NOT applied) test patch proposal

Do NOT edit frozen `web_capabilities_api.rs` / `web_workspace_api.rs`. If the verifier wants the gaps closed,
add a NEW file `crates/server/tests/web_research_workspace_gaps.rs` (draft content, unapplied):

```rust
// Gap probes: reason-string precision (WEB-013-T02) + unavailable-means-unavailable (WEB-015-T02 reason).
use std::sync::Arc;
use axum::{body::{to_bytes, Body}, http::Request, http::StatusCode};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::Value;
use tempfile::tempdir;
use tower::ServiceExt;

async fn get(app: axum::Router, uri: &str) -> Value {
    let r = app.oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    serde_json::from_slice(&to_bytes(r.into_body(), 64*1024).await.unwrap()).unwrap()
}

#[tokio::test]
async fn gap_search_reason_names_grep_boundary() {
    let dir = tempdir().unwrap();
    let app = router(AppState { sessions: SessionService::new(Arc::new(Storage::open_in_memory(dir.path().join("b")).unwrap())), catalog: Arc::new(Catalog::default()) });
    let v = get(app, "/api/capabilities").await;
    // pins WEB-013-T02: reason must state grep is NOT web search (attempt-2 stub currently passes w/o this)
    assert!(v["search"]["reason"].as_str().unwrap().contains("grep"), "search reason names the grep boundary");
    assert!(v["deep_research"]["reason"].as_str().unwrap().contains("citation"), "research reason names citation gap");
}

#[tokio::test]
async fn gap_missing_catalog_carries_explicit_reason() {
    let dir = tempdir().unwrap();
    let app = router(AppState { sessions: SessionService::new(Arc::new(Storage::open_in_memory(dir.path().join("b")).unwrap())), catalog: Arc::new(Catalog::default()) });
    let v = get(app, "/api/workspaces").await;
    assert_eq!(v["available"], false);
    assert!(v["reason"].as_str().unwrap().contains("not configured"), "missing catalog needs explicit reason");
    assert_eq!(v["session_scope_available"], false);
    assert_eq!(v["memory_available"], false);
}
```

## Evidence inventory

- Pristine backups: `/tmp/opencode/WEB-lib.rs.pristine`, `/tmp/opencode/WEB-cap-test.pristine`, `/tmp/opencode/WEB-ws-test.pristine`.
- Attempt logs: `/tmp/opencode/uC-WEB013-attempt1.log` (flag-flip FAIL), `-attempt2.log` (reason-text PASS),
  `/tmp/opencode/uC-WEB013-attempt3.log` (missing-key FAIL), `/tmp/opencode/uC-WEB015-attempt1.log`
  (scope-flag FAIL), `-attempt2.log` (invented-workspace FAIL), `-attempt3.log` (project_root-leak FAIL),
  `/tmp/opencode/stub1-full.log` (full attempt-1 output incl. panic detail).
- All mutations restored; final `diff -q` clean on all three owned files; final GREEN rerun:
  `web_capabilities_api` 1 passed, `web_workspace_api` 2 passed (serial, `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 110`).
- `lib.rs`, `ralph.json`, frozen tests NOT edited (only transient impl stubs, always restored). No GREEN code written —
  investigation-only lane; product code predates lane per prior worklogs.

## Bottom line for verifier

- WEB-013 (unavailable boundary): **pinning YES** — flag-flip and key-drop stubs fail loudly. Reason-string precision: NO.
- WEB-015 (registry-only boundary): **pinning YES 3/3** — scope-flag, phantom-workspace, field-leak stubs all fail.
- Prior "0/5 fail" likely came from weak stub choices (reason-text-only edits like my attempt 2, which genuinely
  doesn't bite) or from probing the absent happy-path surface (T01/T03/T04/T05 have no code and no tests by design —
  stories NOT ACCEPTED). Do NOT treat these suites as non-pinning; do NOT weaken them.
