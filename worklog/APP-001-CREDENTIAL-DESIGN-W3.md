# APP-001-CREDENTIAL-DESIGN-W3 — Credential ownership / broker capability design

- **Task ID:** APP-001-CREDENTIAL-DESIGN-W3
- **Task type:** design / discovery (artifact-only, NO product code)
- **Role:** worker (design author)
- **Status:** blocked (review handoff — awaiting integration-authority decision; no product implementation attempted)
- **Model/route:** xkiro/deepseek/deepseek-v4.1-flash:free
- **Candidate commit:** 798cbff99f4e2a2ad21cc937df428eb6bd871cca
- **Branch:** review/APP-001-CREDENTIAL-DESIGN-W3
- **Scratchpad/artifact (owned):** worklog/APP-001-CREDENTIAL-DESIGN-W3.md
- **Claim:** `tasks/completion/claims.json` row APP-001-CREDENTIAL-DESIGN-W3 → in-progress, session `ses_f1e7b773affeh0ykrz22diqVIj`
- **Authoritative constraints:** `.agents/WORKER.md`, `AGENTS.md`. No builds, no network, no product/tests/controller edits. Append-only scratchpad.

---

## 1. Verified source facts (commit 798cbff)

All facts below were supplied by the orchestrator as verified against 798cbff and re-confirmed by
bounded read/grep on this exact revision. Each is cited as `path:line/symbol`.

1. **`crates/providers/src/auth_store.rs` is planning-only.** Module header: `//! PROV-022:
   fail-closed authentication-storage planning boundary.` It `#![forbid(unsafe_code)]`, opens no
   keyring, reads/writes no file, inspects no environment, uses no network, retains no credential
   bytes. It exposes `SealedSecret<'a>(&'a [u8])` (borrowed, redacted `Debug` →
   `SealedSecret(<redacted>)`), `MAX_PATH_BYTES = 1024`, `MAX_PROVIDER_ID_BYTES = 128`,
   `MAX_SECRET_BYTES = 64 * 1024`, `FILE_MODE_0600 = 0o600`, `ATOMIC_TEMP_SUFFIX = ".tmp"`. It
   returns *typed plans*; a caller with the required capability executes them.
2. **`crates/providers/src/local_credential_import.rs` is planning-only.** Header: "Consent-gated
   planning for importing local provider credentials… a pure boundary. It does not expand a home
   directory, read a file, inspect ambient environment, copy bytes, or write a destination."
   Constants: `MAX_CREDENTIAL_BYTES = 64 * 1024`, `MAX_SOURCE_PATH_BYTES = 1024`, redacted
   destination labels `CODEX_DEST_LABEL`, `CLAUDE_DEST_LABEL`, source labels
   `CODEX_DEFAULT_SOURCE_LABEL`, `CLAUDE_DEFAULT_SOURCE_LABEL` (labels only, never resolved).
3. **Provider/security credential stores are in-memory plaintext `String`s with no zeroize.** No
   `zeroize`/`Zeroize` usage exists in the provider credential path; secrets live as `String` in
   memory and are neither zeroized on drop nor wrapped in a redacting type at the store boundary.
4. **`ProviderConfig` reads the environment for the API key.** `crates/providers/src/config.rs:5`
   `use std::env;`, `:86` `env::var(format!("{}_BASE_URL", prefix))`, `:89`
   `env::var(format!("{}_API_KEY_ENV", prefix))` (indirection: the env var *name* is itself
   configurable), `:92` `_TIMEOUT_SECS`, `:97` `_MAX_TOKENS`, `:102` `_TEMPERATURE`, and `:149`
   `env::var(&self.api_key_env).ok()` — the resolved provider API key is read straight from the
   inherited environment.
5. **`crates/providers/src/responses.rs` (Responses client) likewise depends on env-derived
   config/credentials.**
6. **CLI `creds_configured` is env-only and ignores `data_dir`.**
   `crates/cli/src/daemon_client.rs:776` `pub fn creds_configured(_data_dir: &std::path::Path) ->
   Option<bool>` — the data-dir argument is unused (`_data_dir`). Callers:
   `crates/cli/src/main.rs:234`, `crates/cli/src/onboarding.rs:719,726,731,961`,
   `crates/cli/src/app_start.rs:287,298,312`,
   `crates/cli/tests/installed_default_entrypoint.rs:9`. So "credentials configured" is decided by
   ambient environment, not by persisted state under the configured data directory.
7. **Setup `MemoryAccountStore` is dropped.** `crates/cli/src/onboarding.rs:360`
   `pub struct MemoryAccountStore`, `:376 impl AccountStore for MemoryAccountStore` — the onboarding
   session writes into an in-memory store (`:775` `&'a mut MemoryAccountStore`) that does not
   survive the process. Onboarding completion therefore does not persist a credential/account record.
8. **`PermissionBroker` has no `Credential` intent.** `crates/tools/src/permission.rs` composes
   `allowlist → ext grant → secure gate → broker`, using `opencode_rk_security::{Decision,
   OperationIntent, PermissionBroker}`. The broker/`OperationIntent` surface carries no
   credential-specific intent, so credential read/write/delete cannot be authorized, denied, or
   human-gated as a distinct capability. Broker `Deny`/`RequireHuman` are terminal and `*` never
   lifts the human gate (per module docs).
9. **Daemon bearer is distinct from the provider API key.** `crates/cli/src/daemon_client.rs:610`
   hex length, `:634` "Stop/restart credential policy… `Rotate` mints a fresh bearer", `:643` "True
   only for a 64-char hex bearer", `:655` `out.push_str("Bearer ")`, `:686` blank/forged bearer
   refused loudly. The daemon's own transport bearer is a separate secret domain from the provider
   API key read at `config.rs:149`. They must never be conflated in storage, logging, or lifetime.

---

## 2. Current ownership / lifetime / persistence / failure transitions

| Surface | Owner today | Lifetime | Persistence | Failure transition |
|---|---|---|---|---|
| Provider API key (`config.rs:149`) | Process env (inherited) | Process env block | None — env only | missing → `None` (`.ok()`), later request fails opaque |
| `SealedSecret` (`auth_store.rs`) | Caller of the plan | Caller's borrow (`'a`) | None by design (plan) | validation error → `StoreError` |
| `local_credential_import` source | Caller-supplied label | Caller call | None (pure) | schema/permission error type |
| Onboarding account (`onboarding.rs:360/376`) | `OnboardingSession` | Process / session | `MemoryAccountStore` dropped | lost on exit; nothing to recover |
| "creds configured" (`daemon_client.rs:776`) | env probe | process | env only; `data_dir` unused | `None` ⇒ unknown/unset |
| Daemon bearer (`daemon_client.rs:655`) | daemon restart policy (`Rotate`) | daemon process | descriptor + policy | forged/blank → hard error (`:686`) |

**Gap summary:** there is no owner for a *persisted* provider credential; there is no
zeroize/redaction at the store boundary; the only configured-ness signal ignores `data_dir`; and no
broker capability authorizes credential access.

---

## 3. Threat table

| # | Threat | Current exposure | Required mitigation (proposal) |
|---|---|---|---|
| T1 | Secret lingers in freed heap / core dump / swap | plaintext `String`, no zeroize | zeroize-on-drop `Secret` type; `mlock`-best-effort or process hygiene note |
| T2 | Secret leaks via `Debug`/`log`/panic/`serde` | raw `String` can be formatted | redacting `Debug`/`Display` (`SealedSecret` pattern); serialize-as-redacted |
| T3 | Env inheritance leaks key to child processes | `config.rs:149` reads inherited env | broker-gated read; do not pass env to children; explicit capability |
| T4 | "Configured" decided by ambient env, not data_dir | `creds_configured(_data_dir)` ignores arg | resolve configured-ness from persisted store under `data_dir` |
| T5 | Onboarding appears to persist but drops | `MemoryAccountStore` dropped | atomic File0600 (or keyring) persist on consent |
| T6 | Any tool path reads/writes credentials without gating | no `Credential` intent | add explicit broker `Credential` capability; deny-unless-authorized |
| T7 | Confusing daemon bearer with provider key | two distinct secrets | separate types/stores; never co-locate or co-log |
| T8 | Import from `~/.codex`, `~/.claude` reads arbitrary files | labels only today (safe) | broker capability + explicit consent + schema-bound parse |
| T9 | Crash during write leaves partial/corrupt secret file | n/a (no write today) | write temp `.tmp` (0600) then atomic rename; fsync dir |
| T10 | Deletion leaves recoverable bytes | n/a | delete = overwrite-then-unlink or keyring delete; redact logs |

---

## 4. Resource / path / secret bounds (must be preserved)

- Secret bytes: `MAX_SECRET_BYTES = 64 KiB` (`auth_store.rs`); import input
  `MAX_CREDENTIAL_BYTES = 64 KiB` (`local_credential_import.rs`).
- Path bytes: `MAX_PATH_BYTES = 1024`; import source `MAX_SOURCE_PATH_BYTES = 1024`.
- Provider id: `MAX_PROVIDER_ID_BYTES = 128`.
- File mode: `FILE_MODE_0600 = 0o600`; atomic temp suffix `.tmp`.
- No unbounded retained output; redaction mandatory on every failure path (`AGENTS.md`).

---

## 5. Proposed explicit broker credential capability

Add a distinct, fail-closed capability on the broker/`OperationIntent` surface (integration
proposal — requires security-policy owner review; **not** implemented here):

- Intent variants: `CredentialRead`, `CredentialWrite`, `CredentialDelete`,
  `CredentialImportLocal` (each scoped to one provider id ≤128 B).
- Gate order unchanged: `shape → reserved/human → allowlist → ext → secure → broker`; broker `Deny`
  and `RequireHuman` remain terminal; `*` never satisfies a credential intent.
- Reserved namespace: credential intents are never grantable via wildcard and may be human-only for
  `write`/`delete`/`import` per policy decision (see §8).
- Effects run only on `Ok(())` (`run_if_authorized` invariant), so denial executes zero effects.

---

## 6. Storage proposal — Keyring preferred, File0600 fallback (clearly marked PROPOSAL)

> **PROPOSAL — not accepted, not implemented.** Both options below are design candidates; the
> repository has no keyring dependency accepted today.

- **Preferred: OS keyring.** Store under a per-provider key (`service = opencode-rk`,
  `account = provider_id`). Pros: OS-managed protection, no plaintext at rest. Cons: adds a
  dependency (requires dependency-acceptance sign-off), availability varies headless.
- **Fallback: atomic `File0600`.** Path under `data_dir` (never `/tmp`): write `<name>.tmp` with
  `0o600`, `fsync`, `rename` into place, `fsync` parent dir. On delete: overwrite then unlink.
  Use `ATOMIC_TEMP_SUFFIX = ".tmp"` and `FILE_MODE_0600` from `auth_store.rs`.
- Selection is a policy decision (§8); the fallback must be opt-in and clearly surfaced.

---

## 7. Cross-cutting behaviors

- **Cancellation:** every credential operation is scoped-cancellable; cancel before commit leaves no
  partial secret (temp removed). No detached task without an owner.
- **Deletion:** keyring → delete entry; file → overwrite-then-unlink; verify absence; redact.
- **Migration:** env-only → persisted store must be one-way and idempotent; if both exist, persisted
  store wins and env is only a bootstrap input, never re-consulted after migration.
- **Redaction:** no secret in logs, errors, panics, `Debug`, or metrics; reuse
  `SealedSecret(<redacted>)` discipline.

---

## 8. Serial one-product-file vertical slices (integration order)

Each slice is ONE owned product file, RED-first, frozen test, no stub. Parent stays OPEN until the
frozen parent journey passes on the integrated revision.

- **A — `providers/auth_store.rs`** (foundation): add the zeroizing secret type + redacting
  `Debug`, keeping existing constants/plan API. RED: a test asserting the type redacts on format and
  zeroizes on drop; must compile-fail before impl.
- **B — `providers/config.rs` OR `providers/responses.rs`** (resolve handle via broker): replace
  direct `env::var(&self.api_key_env)` (`config.rs:149`) with a broker-gated credential resolution
  path. RED: resolution denied without `CredentialRead`; allowed with it. Pick exactly one file.
- **C — `cli/main.rs`** (setup persistence): make onboarding persistence wire to the store, not the
  dropped `MemoryAccountStore` (`onboarding.rs:360/376`). RED: after setup, configured-ness survives
  a fresh process.
- **D — `cli/daemon_client.rs`** (restart detection): make `creds_configured(_data_dir)`
  (`:776`) actually consult `data_dir`, not ambient env. RED: env-set + empty data_dir ⇒ not
  configured; persisted store ⇒ configured. Keep daemon bearer (`:655`) separate.
- **E — server caller consumption (if required):** only if a server/daemon caller must consume the
  credential handle; otherwise explicitly deferred.

**Integration order:** A → B → C → D → (E if required). A unblocks B. C and D both read the
persisted store, so C must land before D's RED can go green.

**Each slice acceptance (disposable, no network, no host DB):**
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 rtk cargo test -p <crate> --test <frozen_target>`
- RED captured + hash frozen before impl; GREEN on the integrated revision; frozen tests unedited.
- Workspace guard: `rtk python3 tools/validate_repository.py` before integration.

---

## 9. Open policy decisions (blockers for acceptance)

1. Keyring dependency acceptance vs `File0600`-only fallback (dependency-acceptance owner).
2. Which credential intents are human-only (`write`/`delete`/`import`) vs grantable.
3. Whether env var remains a bootstrap input or is removed entirely post-migration.
4. Whether slice E (server caller consumption) is in scope for APP-001 or deferred.

---

## 10. Status and handoff

**Blocked (review) — no product code, no tests, no controller edits.** Artifact-only lane as
instructed. The parent APP-001 is **NOT** complete: slices A–E above are unwired design proposals,
and no frozen parent journey has passed. Two prior workers did source analysis and wrote nothing;
this lane records the verified facts and a serial, testable integration plan.

**Commands run (all `rtk`-prefixed, no builds/network):**
- `rtk git log -1` / `rtk git branch --show-current` / `rtk git rev-parse HEAD` → 798cbff…, branch
  review/APP-001-CREDENTIAL-DESIGN-W3.
- `rtk find … auth_store.rs local_credential_import.rs config.rs responses.rs daemon_client.rs main.rs permission*.rs` → confirmed paths.
- `rtk sed/auth_store.rs` + `local_credential_import.rs` → planning-only confirmed.
- `rtk grep creds_configured crates/` → daemon_client.rs:776 (`_data_dir` unused) + callers.
- `rtk grep env::var providers/config.rs responses.rs` → config.rs:86,89,92,97,102,149.
- `rtk grep MemoryAccountStore crates/` → onboarding.rs:360,376,775.
- `rtk sed permission.rs` → intent surface, terminal Deny/RequireHuman, `*` never grants.
- `rtk grep bearer daemon_client.rs` → :610,634,643,655,686.

**Hashes:** candidate `798cbff99f4e2a2ad21cc937df428eb6bd871cca`. No frozen test authored in this
lane (design-only). Implementation hash N/A.

**Resources:** read-only, no Cargo/browser/DB; well within the 8 GiB budget; no deviations.

**Unresolved gaps:** §9 policy decisions; slices A–E unimplemented; parent journey unproven.