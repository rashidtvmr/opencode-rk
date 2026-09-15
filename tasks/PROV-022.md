# PROV-022

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-041.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: PROV-022-T01, PROV-022-T02, PROV-022-T03, PROV-022-T04, PROV-022-T05.
Ownership locks: crates/providers/src/auth_store.rs only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations.
Suggested module: crates/providers/src/auth_store.rs (new module in crate opencode-rk-providers; lib.rs wiring left to integrator).

## User-observable outcome

Hardened auth storage: OS keyring where available with 0600-file fallback, atomic refresh persistence, path allowlist enforcement, and no inherited secrets. Failures report codes, never secret bytes.

## Source evidence

- crates/providers/src/auth.rs:6-18 AuthMethod; :40-101 AuthHandler and in-memory refresh.
- crates/providers/src/integration.rs:1-5 explicitly stops before credential storage/external OAuth; :11-18 bounded constants.
- crates/providers/src/config.rs:7-22 ProviderConfig (provider_id, api_key_env routing).
- crates/providers/src/lib.rs:4-48 provider modules (no auth_store yet).
- docs/SECURITY.md:14-32 brokered permissions, no direct secret access/unrestricted env, no unrestricted inherited environment.
- docs/research/REQ-041-provider-compatibility-research.md:1 REQ-041 official provider compatibility and CLI authentication scope.

## Observable contract

- `StoreBackend { Keyring, File0600 }`: explicit backend selection; keyring preferred, file fallback documented in status.
- `StoreRequest { provider_id, backend: StoreBackend, dest: AllowlistedPath }`: dest must sit under the caller-supplied storage root; absolute escapes rejected.
- `plan_store(request) -> StorePlan { provider_id, backend, dest_label, mode: 0o600 }`: redacted plan with label only, zero secret bytes; actual secret bytes travel only through a caller-owned sealed buffer, never through logs or errors.
- `plan_refresh_persist(provider_id) -> RefreshPlan { provider_id, atomic: true }`: refresh writes via temp-file plus rename; partial writes never observed.
- `status_of(provider_id, backend) -> StoreStatus { provider_id, backend, exists: bool, error: Option }`: redacted and serializable.
- `validate_dest(root, dest) -> Result<(), StoreError>`: rejects absolute paths, `..` escapes, and non-allowlisted roots.
- Deterministic: same request yields byte-identical plan; no ambient env reads (no inherited secrets); no network.
- Suggested module boundary: crates/providers/src/auth_store.rs owning StoreBackend, StoreRequest, StorePlan, StoreStatus, StoreError, plan_store, plan_refresh_persist, status_of, validate_dest; shared lib.rs wiring left to integrator.

## Failure states

- Non-allowlisted or escaping dest: `Err(StoreError::PathNotAllowed)`; nothing written.
- Empty provider id: `Err(StoreError::EmptyProvider)`; nothing written.
- Keyring unavailable: documented fallback to File0600 with mode 0o600; status reports fallback backend honestly.
- Oversize secret buffer beyond 64 KiB: `Err(StoreError::TooLarge)`; nothing written.
- Secret safety: plans, statuses, errors, and logs contain zero secret bytes. Tests use disposable fixture dirs only; no inherited env secrets; no live keyring writes required (backend trait faked in tests).

## Resource bounds

- Bounded paths (max 1024 bytes), bounded buffers (max 64 KiB), atomic write via temp plus rename; no unbounded retained output.
- No background threads, no network, no ambient env reads; caller owns buffers and cancellation.
- No unbounded queue; owner and cancel path defined per crates/providers/src/integration.rs:11-18 bounded-constants precedent.

## Acceptance criteria

- PROV-022-T01: file fallback happy path: plan_store with File0600 under fixture root yields plan with mode 0o600 and dest label; fixture write lands with 0o600 permissions and byte-identical content.
- PROV-022-T02: atomic refresh: plan_refresh_persist yields atomic true; simulated crash mid-write leaves either old or new bytes, never partial (temp-plus-rename observable in fixture dir).
- PROV-022-T03: path allowlist: absolute and `..`-escape dests yield PathNotAllowed with nothing written; empty provider yields EmptyProvider.
- PROV-022-T04: keyring fallback honesty: keyring-unavailable fake reports File0600 status; oversize buffer yields TooLarge with nothing written.
- PROV-022-T05: redaction and isolation: Debug plus serialize of plans, statuses, and errors contain no fixture secrets; no env secrets read; no files outside disposable fixture dir; no DB writes.

## Test-first execution

- RED: author tests PROV-022-T01..T05 against crates/providers/src/auth_store.rs; establish compiling RED (fail: no auth_store module).
- GREEN: implement minimum native Rust allowlisted planner plus atomic file write; GREEN, refactor, rerun; negative tests (escape, empty, oversize, fallback, leak scan).
- Evidence: `cargo test -p opencode-rk-providers auth_store`; `cargo check --workspace`; frozen test hash plus command manifest; verifier decides acceptance.
