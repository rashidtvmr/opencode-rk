# WEB-015-LIFT worklog — projects, workspace context, memory disclosure (GREEN-only, NOT ACCEPTED)

Claim: daemon reads canonical `data/catalog.db` format-2 registry, exposes
bounded workspace metadata read-only; session membership + memory/context
authority explicitly unavailable (`session_scope_available=false`,
`memory_available=false`); absent catalog → explicit unavailable, no invented
projects. Project-scoped sessions/files/memory/T05 NOT satisfied.

## Source evidence

- `tasks/WEB-015.md:9-18` — boundary: registry metadata only; no
  workspace→session/file membership, no memory/context config; browser
  must not filter/attribute chats per workspace.
- `crates/server/src/lib.rs:196-236` (`list_workspaces`): catalog absent →
  `workspaces:[]` + `available:false` + "not configured" reason; present →
  id/label/project_root/status/created_at_us only, plus
  `session_scope_available:false`, `memory_available:false`,
  boundary reason.
- `crates/sessions/src/lib.rs:362-391` (`with_workspace_catalog_path`,
  `list_workspaces`): `CatalogV2::open_existing` + `list_workspaces(limit)`,
  `None` when unconfigured.
- `crates/server/tests/web_workspace_api.rs:12-85` (frozen, never edited) —
  T01 canonical-catalog read, T02 missing-catalog explicit unavailable.
- `crates/server/tests/web_workspace_reason.rs:33-97` (verify-only, not
  edited) — missing-catalog reason, present-catalog boundary reason,
  unknown `?scope=` projects no membership.
- `crates/server/tests/web_workspace_full.rs:71-139` (verify-only, not
  edited) — T01 2-seed registry read, T02 missing unavailable, T03 scope
  no-membership, T04 registry-fields-only, T05 deterministic reread.
- Upstream pins: opencode `95daf90670b7c039c436c85537da5fbfe2205b41`,
  9router `17c4cc76877bd1755030a8414f8d0083f48dcccf`
  (`sources/upstream.lock.json:9-27`). No upstream workspace reference
  claimed by card.
- `ralph.json`: WEB-015 `not-started`. Base rev `248f519`
  (`248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`).

## Observed scenario (verify-only rerun, no product edit)

- Serial budget: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`,
  `--test-threads=1`, `timeout 120`, single lane at a time.
- `cargo test -p opencode-rk-server --test web_workspace_api -- --test-threads=1`
  → 2 passed (`web_015_t01_workspace_registry_is_read_from_canonical_catalog`,
  `web_015_t02_missing_catalog_is_explicitly_unavailable`), 0 failed.
- `cargo test -p opencode-rk-server --test web_workspace_reason -- --test-threads=1`
  → 3 passed (`web_015_reason_missing_catalog_carries_explicit_reason`,
  `web_015_reason_present_catalog_carries_boundary_reason`,
  `web_015_reason_unknown_scope_projects_no_membership`), 0 failed.
- `cargo test -p opencode-rk-server --test web_workspace_full -- --test-threads=1`
  → 5 passed (`web_015_full_t01`..`t05`), 0 failed.
- Total: 2 + 3 + 5 = 10 GREEN, 0 failed. Log `/tmp/opencode/cB-web015.log`.
- Product code predates lane → GREEN-only, NOT valid RED. No RED fabricated.
- sha256 (evidence, files untouched):
  - `web_workspace_api.rs` `a1a918f5f62d7a60376b6bb7f4a071bae2aacb98f8bcdbcc1baff7e727a1bd53`
  - `web_workspace_reason.rs` `8b29dcdb0ce200cff28f25307706923acec9f4913396a45f76e144f9325006d8`
  - `web_workspace_full.rs` `2a1cbfe0dbe1ee2fc59b1ecd97844313a26ea78df26f1250755756f2f53743ac`
    (matches prior `worklog/WEB-015.md:78` mutation-restore hash).

## Target boundary

- Owned: this worklog only (`worklog/WEB-015-LIFT.md`). No product/test
  file touched.
- Explicitly NOT owned: workspace routes, catalog v2, session/file
  membership, memory/context disclosure, browser selector, frozen
  `web_workspace_api.rs`, `lib.rs`, `ralph.json`.
- Read-only verified: `crates/server/src/lib.rs:196-236`,
  `crates/sessions/src/lib.rs:362-391`, frozen
  `crates/server/tests/web_workspace_api.rs`, verify-only siblings
  `web_workspace_reason.rs` + `web_workspace_full.rs`.

## Tests

- None frozen by this lane. Evidence rerun only:
  - `crates/server/tests/web_workspace_api.rs` (85 lines, 2 tests).
  - `crates/server/tests/web_workspace_reason.rs` (97 lines, 3 tests).
  - `crates/server/tests/web_workspace_full.rs` (139 lines, 5 tests).
- T01 scoped-load/context-breakdown, T02 cross-workspace leak, T03 a11y,
  T04 bounds, T05 persistence: no frozen suite (no membership/memory
  authority exists to test against).

## Decisions

- `ponytail:` registry-only read; browser shows boundary explicitly,
  never filters chats by workspace until membership authority exists.
- Unavailable files/paths/cross-workspace access must fail explicitly
  (card T02) — no daemon path for this yet.

## Remaining unknowns

- Authoritative workspace→session/file membership + persisted
  memory/context config schema/service (integration proposal needed).
- Frozen T01..T05 RED/GREEN owned by future lane; verifier decides.
- Story stays NOT ACCEPTED per `tasks/WEB-015.md:15-18`.
