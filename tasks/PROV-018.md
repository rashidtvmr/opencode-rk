# PROV-018

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-041.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: PROV-018-T01, PROV-018-T02, PROV-018-T03, PROV-018-T04, PROV-018-T05.
Ownership locks: crates/providers/src/local_credential_import.rs only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations.
Suggested module: crates/providers/src/local_credential_import.rs (new module in crate opencode-rk-providers; lib.rs wiring left to integrator).

## User-observable outcome

Consent-based import of ~/.codex/auth.json and ~/.claude/.credentials.json or configured directories: validates schema and file permissions, copies into a protected location only after explicit user consent. Import is never automatic or silent; tokens never logged.

## Source evidence

- crates/providers/src/auth.rs:6-18 AuthMethod (ApiKey, BearerToken, OAuth2 shapes import targets).
- crates/providers/src/auth.rs:40-101 AuthHandler and in-memory refresh (imported material lands here, never persisted by this slice).
- crates/providers/src/integration.rs:1-5 explicitly stops before credential storage/external OAuth; :11-18 bounded constants.
- crates/providers/src/config.rs:7-22 ProviderConfig (provider_id routing for imported credentials).
- crates/providers/src/lib.rs:4-48 provider modules (no local_credential_import yet).
- docs/SECURITY.md:14-32 brokered permissions, no direct secret access/unrestricted env; :62-72 human authority for OAuth.
- docs/research/REQ-041-provider-compatibility-research.md:1 REQ-041 official provider compatibility and CLI authentication scope.

## Observable contract

- `ImportSource { CodexDefault, ClaudeDefault, ConfigDir { path } }`: allowlisted source only; arbitrary paths rejected.
- `ImportRequest { source: ImportSource, consent: UserConsent }`: consent is an explicit human grant value; absent consent never imports.
- `validate_schema(bytes) -> Result<CredentialKind, ImportError>`: checks JSON shape and required fields only; returns kind (api-key or oauth) without exposing secrets.
- `check_permissions(path_meta) -> Result<(), ImportError>`: rejects world-readable files (mode 0777-style) with `TooPermissive`; caller supplies metadata, no ambient fs reads inside pure validators.
- `plan_import(request, schema, perms) -> ImportPlan { provider, kind, dest_label }`: redacted plan with destination label only, zero secret bytes; execution/copy owned by a later storage slice.
- Deterministic: same request plus fixture bytes yield byte-identical plan; no wall-clock, no network.
- Suggested module boundary: crates/providers/src/local_credential_import.rs owning ImportSource, UserConsent, CredentialKind, ImportPlan, ImportError, validate_schema, check_permissions, plan_import; shared lib.rs wiring left to integrator.

## Failure states

- Missing consent: `Err(ImportError::ConsentRequired)`; nothing read, nothing copied.
- Non-allowlisted path: `Err(ImportError::PathNotAllowed)`; nothing read.
- Malformed JSON or missing required fields: `Err(ImportError::BadSchema)`; no partial plan.
- World-readable source: `Err(ImportError::TooPermissive)`; import refused even with consent.
- Oversize file: `Err(ImportError::TooLarge)` beyond 64 KiB cap; never buffers unbounded input.
- Secret safety: plans, errors, and logs contain zero credential bytes. Tests use disposable fixture dirs and generated datasets only, never the live ~/.codex or ~/.claude paths.

## Resource bounds

- Pure validators plus bounded plan: no Command, no thread, no network; input bytes capped at 64 KiB.
- Path allowlist: exactly the two defaults plus configured dirs under one caller-supplied root; no inherited env secrets.
- No unbounded queue, no unbounded retained output; owner and cancel path defined per crates/providers/src/integration.rs:11-18 bounded-constants precedent.

## Acceptance criteria

- PROV-018-T01: codex happy path: valid codex fixture bytes plus explicit consent yields plan with provider codex and kind; output contains no fixture secret bytes.
- PROV-018-T02: claude happy path: valid claude fixture bytes plus explicit consent yields plan with provider claude-code and kind; output contains no fixture secret bytes.
- PROV-018-T03: consent and path gating: missing consent yields ConsentRequired; non-allowlisted path yields PathNotAllowed; neither reads fixture bytes.
- PROV-018-T04: schema and permission failures: malformed JSON yields BadSchema; world-readable meta yields TooPermissive; oversize input yields TooLarge.
- PROV-018-T05: redaction and isolation: Debug plus serialize of every plan and error contain no fixture secrets; no live home-dir paths touched; no DB writes.

## Test-first execution

- RED: author tests PROV-018-T01..T05 against crates/providers/src/local_credential_import.rs; establish compiling RED (fail: no local_credential_import module).
- GREEN: implement minimum native Rust consent-gated validators and planner; GREEN, refactor, rerun; negative tests (no-consent, bad-path, bad-schema, permissive, oversize, leak scan).
- Evidence: `cargo test -p opencode-rk-providers local_credential_import`; `cargo check --workspace`; frozen test hash plus command manifest; verifier decides acceptance.
