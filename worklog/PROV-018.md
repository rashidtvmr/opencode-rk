# PROV-018 — Local Credential Import

## Claim
Session: ses_f389670a7ffeuc9N40EFR1AM4H
Scratchpad: worklog/PROV-018.md
Status: completed

## Source Evidence
- `crates/providers/src/local_credential_import.rs` — full implementation present
- `crates/providers/tests/prov_018_local_credential_import.rs` — frozen tests (5 tests)

## Observed State
Implementation was already complete (prior work). All invariants implemented:
- `ImportSource { CodexDefault, ClaudeDefault, ConfigDir { path } }` — allowlist enforced
- `UserConsent::Granted` required; `NotGranted` → `ConsentRequired`
- `validate_schema()` — size-bounded (64 KiB), JSON parse, api_key/oauth shape detection
- `check_permissions()` — rejects mode & 0o077 != 0 (group/other bits)
- `plan_import()` — consent gate → path gate → permissions gate → deterministic redacted plan
- No secret bytes in plan, error, Debug, or serialized output

## Test Evidence
Command: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 rtk cargo test -p opencode-rk-providers --test prov_018_local_credential_import`
Result: 5 passed (1 suite, 0.00s)

- prov_018_t01_codex_happy_path: PASS
- prov_018_t02_claude_happy_path: PASS
- prov_018_t03_consent_and_path_gating: PASS
- prov_018_t04_schema_and_permission_failures: PASS
- prov_018_t05_redaction_and_isolation: PASS

## Decisions
- No code edits made (implementation already correct, tests frozen and passing)
- Claimed, verified, updated to completed per WORKER.md protocol

## Remaining Unknowns
None. lib.rs wiring left to integrator per task spec.
