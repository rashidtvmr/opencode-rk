# SHARE-003

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: none (DISC-002 discovered; enterprise boundary per gap file).
Dependencies: none.
Test obligations: SHARE-003-T01, SHARE-003-T02, SHARE-003-T03, SHARE-003-T04, SHARE-003-T05.

## User-observable outcome

Explicit enterprise-remote boundary declaration: the native binary performs no hosted share/sync/support/GitHub/deployment side effects; every enterprise-remote operation resolves to a typed refusal naming the missing in-surface specification partition. No network, no credentials, no DB mutation, no secret logging.

## Source evidence

- Pinned OpenCode commit `95daf90670b7c039c436c85537da5fbfe2205b41`.
- `sources/enterprise-remote-spec-gap.json` (status `searched-no-qualifying-in-surface-spec`): missing kind `spec`; required partitions `enterprise-share-http`, `function-syncserver-websocket-r2`, `function-support-relay`, `function-github-token-exchange-installation`, `deployment-resource-lifecycle`; rejected candidates `packages/enterprise/README.md`, `[...path].ts` route metadata, `package.json` files, `sst-env.d.ts` files.
- `sources/sharing-ownership-gap.json`: `enterpriseRemoteGap` keeps SHARE-003 cross-surface blocked; partitions `enterprise-share-http-and-support-admin`, `legacy-share-snapshot-migration`; unresolved `enterprise-function-hosted-sync-and-deployment-boundary`, `support-admin-removal-authority`, `legacy-versus-org-share-http-auth-and-failure-policy`.
- `sources/disc-003-reconciliation.json` findings `opencode.sharing` + `opencode.enterprise-remote` (both partial): heterogeneous hosted surface (Durable Object/WebSocket state, R2, support relay, GitHub OIDC/PAT exchange, installation lookup, deployment lifecycle) is not authorized target behavior; SHARE-003 cannot close from cross-family intersection alone.
- PLAN.md sections 5-6: slice independence rules, mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash, GREEN minimum.
- tasks/TOOL-016.md: task-card model (Status/Kind/contract/test obligations).
- Classification: deliberate safer deviation (typed refusal boundary instead of hosted parity); Cloudflare Durable Objects/R2, external Discord/GitHub calls, websocket fanout, and hosted deployment are explicitly not reproduced.

## Observable contract

- `EnterpriseOp = ShareHttp | SyncServerRelay | SupportRelay | GithubTokenExchange | DeploymentLifecycle`; `EnterpriseBoundary::authorize(op) -> Err(BoundaryError::Refused { op, partition, reason })` for every variant, always.
- `partition(op)` maps exactly to the five required gap partitions; `reason` cites the spec-gap file and names the missing `spec` kind; error values carry no credentials, secrets, URLs, or tokens.
- Pure synchronous predicate over one caller-owned op value; no I/O, no fallback, no partial execution (refusal precedes any side effect).
- Suggested module boundary: `crates/share/src/enterprise_boundary.rs` (crate `opencode-rk-share`); worker ships additive fragment only, never edits shared `lib.rs`, `Cargo.toml`, schemas, migrations.

## Failure states

- Any enterprise op attempted: `Err(Refused)` with the exact partition name; zero bytes sent, zero state changed (assert absence of socket/file/DB effects, not just the error string).
- Support-admin removal authority: always refused; no bearer check performed locally (no credential handling at all).
- Legacy-vs-org endpoint selection: refused under `enterprise-share-http`; caller must not silently pick an endpoint.
- Secret safety: boundary accepts no secrets/credentials/tokens as input; logs only op + partition names. No SQLite/OpenCode DB writes. No changes to the user's existing OpenCode database; tests use disposable in-memory fixtures only.

## Resource bounds

- Zero allocation beyond the error value; no queue, no retained output, no connection pool.
- No thread/task/clock/watcher/socket; caller-owned op value dropped on return.
- No retry: refusal is terminal for the op; caller decides next safe local step (no automatic replay of the refused side effect).
- Zero hidden cost: unreferenced boundary compiles to nothing; referencing it performs no I/O.

## Test obligations (frozen)

- SHARE-003-T01 (all ops refused): each of the 5 `EnterpriseOp` variants => `assert!(matches!(err, Refused { .. }))` with the exact expected partition string.
- SHARE-003-T02 (partition mapping): `assert_eq!(partition(op), "enterprise-share-http")` (and each of the other four); reason string names missing kind `spec`.
- SHARE-003-T03 (no side effects): attempted ops leave no socket/file/DB trace (assert fixture dir + test DB untouched); captured network/file Descriptors show zero new handles.
- SHARE-003-T04 (support-admin + endpoint selection): support removal op refused without consuming any bearer input; legacy-vs-org selection op refused rather than defaulting to either endpoint.
- SHARE-003-T05 (safety + determinism): repeated `authorize` calls return identical errors; captured logs contain zero credential/secret/token/URL bytes.

## TDD steps

1. Inspect: PLAN.md 5-6, TDD.md 2-5, both gap files + disc-003 sharing/enterprise-remote findings (done, see evidence).
2. Contract: defined above.
3. Author tests SHARE-003-T01..T05; establish compiling RED (fail: no boundary module).
4. Freeze test hash + command manifest.
5. Implement minimum native Rust refusal boundary.
6. GREEN, refactor, rerun; negative tests (each op variant, no-I/O proof, determinism).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/SHARE-003.md
cargo test -p opencode-rk-share enterprise_boundary
cargo check --workspace
```
