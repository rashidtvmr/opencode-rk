# SHARE-003 — enterprise-remote refusal boundary

## Claim
Own `crates/sessions/src/share_enterprise.rs` (lane-variant; task-card `crates/share/src/enterprise_boundary.rs` never existed — no `opencode-rk-share` crate). Pure typed refusal: every `EnterpriseOp` resolves to `Err(Refused{op,partition,reason})`, zero I/O. No test/lib.rs edits this session.

## Source evidence (pin 95daf90670b7c039c436c85537da5fbfe2205b41)
- `sources/enterprise-remote-spec-gap.json` (status `searched-no-qualifying-in-surface-spec`, missing kind `spec`): 5 required partitions `enterprise-share-http`, `function-syncserver-websocket-r2`, `function-support-relay`, `function-github-token-exchange-installation`, `deployment-resource-lifecycle`. Rejected candidates: README/route-metadata/package.json/sst-env files.
- `sources/sharing-ownership-gap.json` `enterpriseRemoteGap`: cross-surface blocked; legacy-vs-org auth/failure policy unresolved.
- `disc-003 opencode.sharing` + `opencode.enterprise-remote` (both partial): hosted surface (Durable Object/WebSocket/R2/support relay/GitHub OIDC/deployment) not authorized target behavior.
- Classification: deliberate safer deviation (typed refusal, not hosted parity).

## Observed scenario
- `src/share_enterprise.rs` (`EnterpriseOp::{ShareHttp,SyncServerRelay,SupportRelay,GithubTokenExchange,DeploymentLifecycle}`, `EnterpriseBoundary::authorize(op)->Err(Refused)`, `partition(op)` exact 5 strings, reason cites gap file + missing `spec`). `#[path]`-included by `tests/share_enterprise.rs`; lane mirror `share_enterprise_lane.rs` exists. No `#![forbid(unsafe_code)]` on canonical (drift vs lane twin, noted in SHARE-001.md:222).
- Frozen suites: `share003_t01_all_ops_refused`, `t02_partition_mapping_and_reason`, `t03_no_side_effects`, `t04_support_admin_and_endpoint_selection`, `t05_safety_and_determinism`.

## Target boundary
- Pure sync predicate over one caller-owned op; no I/O/fallback/partial execution; refusal precedes side effects.
- Zero allocation beyond error value; no thread/clock/socket/pool; no retry (terminal refusal); accepts no secrets (logs op+partition names only).
- No DB writes; disposable fixtures only.

## Tests (frozen, verify-only rerun)
- Suites: `tests/share_enterprise.rs` + `tests/share_enterprise_lane.rs`, 5 tests each.
- GREEN (2026-09-16, `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`, rev `248f519` + workdir dirt, serial, `timeout 120 rtk`):
  `share_enterprise` 5/5, `share_enterprise_lane` 5/5 (each `cargo test -p opencode-rk-sessions --test <suite>` EXIT=0).
- Frozen hashes (sha256): test `share_enterprise.rs` `f8e6df55…`, lane `e2d06e7a…`; src `share_enterprise.rs` `7f7e7ac4…`, lane `b1bef9e0…`. No frozen test edited.
- RED history: SHARE-001.md:186-217 — partition mutation (`enterprise-share-http`→`WRONG-PARTITION`) gives `share_enterprise` 2/5 (T01/T02/T04 RED), restored byte-identical GREEN.

## Decisions
- No code change: implementation GREEN; dedicated worklog only.
- `crates/share` non-existence is controller mapping decision, not code gap.

## Remaining unknowns
- Canonical-vs-lane `forbid(unsafe_code)` + `thiserror`-vs-manual drift; integrator unifies.
- `crates/share` acceptance mapping pending controller allowlist.
