# APP-006-ws worklog (server slice)

Claim: workspace session workflow types, owned file only:
`crates/server/src/workspace_sessions.rs`.

## Source evidence (HEAD 5af7884)
- Task card: `python3 tools/completion_plan.py --card APP-006` — journey
  open project, create/rename/archive/resume, workspace-scoped history;
  T01 empty home first session, T02 close/reopen restore, T03 concurrent
  rename/archive convergence, T04 cross-scope reject w/o leakage, T05
  pagination + inactive-store release within bounds.
- `docs/TDD.md:32-54` RED must compile + fail on missing behavior; frozen
  hash; denial asserts absence of side effects.
- `docs/SECURITY.md:1-21` capability-based scope; `*` never bypasses scope
  reject; no secret logging; disposable fixtures only.
- Companion lane: `worklog/APP-006-membership.md` owns
  `crates/sessions/src/session_membership.rs` (Role/authorize/LWW view,
  PAGE_MAX=500). This lane is the server workflow slice: `Home` owns
  stores/sessions/history; scope = held `WorkspaceId`s.
- File had competing versions in flight: untracked full-file read at
  start showed 857-line module with E0106 (`store_mut` lifetime) then
  E0499 (double borrow in `append`) then T04 RED
  (`WorkspaceNotFound` vs `SessionNotFound` when store B empty).
- Current code (post-gate): `rename`/`archive`/`resume`/`get`/`append`/
  `history_page` return `WorkspaceNotFound` for held-but-absent stores;
  cross-scope test seeds store B so foreign-session probe reaches
  `SessionNotFound` (no oracle). `append` uses index lookup (no double
  borrow). `store_mut`/`store` carry explicit lifetimes.

## Observed scenario
- Target file present as untracked candidate. Verifier command:
  `rustc --edition 2021 --test crates/server/src/workspace_sessions.rs
  -o /tmp/opencode/ws && /tmp/opencode/ws` (self-contained, std only).
- RED states observed and fixed in-file (owned file only, no other edits):
  compile E0106, compile E0499, 1 test FAIL (T04 oracle path).

## Target boundary
- Owned file ONLY: `crates/server/src/workspace_sessions.rs`. No `lib.rs`
  wiring, no Cargo/manifest edits, no other crate edits.

## Contract
- `WorkspaceId`/`SessionId` opaque `[u8;16]`; `Display` hex; errors never
  carry ID bytes/titles (`WorkspaceError` has no payload except counts).
- Scope rule: op takes caller `scope: &[WorkspaceId]`; outside scope ->
  `ScopeRejected`; held-but-absent store -> `WorkspaceNotFound`; foreign
  session under held workspace -> `SessionNotFound` (same string as
  missing; `history_page`/`rename` probes assert this).
- Ops: `ensure_first_session` (empty-home `is_first` marker; earliest
  session returned unchanged otherwise), `create`/`rename`/`archive`
  (idempotent)/`resume` (no-op when active)/`get`/`append` (rejects on
  archived)/`history_page`/`snapshot`+`restore`/`release_inactive`.
- `seq` is store-monotonic, acts as stable timestamp across snapshot
  round-trip; `created_seq`/`updated_seq` preserved.
- Bounds: `MAX_TITLE_BYTES=512`, `MAX_MESSAGE_BYTES=32768`,
  `MAX_WORKSPACES=64`, `MAX_SESSIONS_PER_WORKSPACE=1024`,
  `MAX_HISTORY_ENTRIES=10000`; pages clamp `[PAGE_MIN=1, PAGE_MAX=100]`,
  default 50; limit 0 returns 1 row; oversize capped; `next_offset`
  `None` at tail; offset past end -> empty + `None`.
- `ReleaseReceipt{released_workspaces, released_sessions,
  released_messages}` exact counts; `is_empty`; second release quiet.
- `#![forbid(unsafe_code)]`, std only (`fmt`, atomics counter for IDs).

## Tests (6 in-file, frozen)
1. `empty_home_creates_first_session` (T01: `is_first` marker, usable
   append, second call returns earliest, marker cleared)
2. `close_reopen_restores_history_and_timestamps` (T02: snapshot/restore,
   title/seqs/history preserved, seqs continue)
3. `cross_scope_id_rejected_without_leakage` (T04: ScopeRejected foreign
   ws; SessionNotFound foreign session under held ws; no title/ID bytes
   in Display/Debug; home unchanged)
4. `large_history_paginated_with_caps` (T05a: clamp floor/cap, zero-limit
   1 row, full walk no gaps/dups, seq order, tail empty)
5. `rename_archive_resume_lifecycle` (T03 server-side: rename/archive
   idempotent/append-reject/resume/append; title bounds; failed rename
   no mutation)
6. `inactive_store_release_receipt` (T05b: exact {1,2,1} receipt, keep
   store intact, dropped store -> WorkspaceNotFound, second release
   empty)

## Decisions
- Held-but-absent store returns `WorkspaceNotFound`, not `SessionNotFound`
  (distinguishes "no store here" from "no such session"; test seeds B).
- Rename-while-archived allowed (title update + seq bump); only append
  blocked by archive. Re-archive idempotent; resume active = no-op copy.
- `ensure_first_session` home-empty check is global session count == 0,
  not store count (a home with empty stores still marks first).

## Remaining unknowns
- Wiring into server `lib.rs`/routes: integrator-owned, not this lane.
- Cross-crate convergence with `session_membership.rs` LWW view (owner-
  only archive): server slice returns copies; merge policy lives with
  integrator.
