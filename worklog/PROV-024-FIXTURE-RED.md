# PROV-024 fixture RED lane

## Claim

- Task: `PROV-024`
- Session: `ses_f32d353c2ffePoAP8P93AgDut9`
- Branch: `lane/PROV-024-fixtures`
- Owned paths: `tests/bootstrap/test_prov024_provider_contracts.py`, this scratchpad, own ledger row.
- Scope exclusion: no fixture, product, controller, network, task-card, or frozen-test edits.

## Source evidence

- Base revision: `dd7670dc78e5738a8dee1ca64c308b8618e8c181`.
- `tasks/PROV-024.md:27-35`: manifest v1, six provider files, redacted request/auth state, shape diff, 16 KiB file and 128 KiB directory bounds.
- `tasks/PROV-024.md:37-48`: `bad-fixture`, `bad-endpoint`, `secret-leak`, `over-cap`, `UnknownProvider`, offline and bounded behavior.
- `docs/research/REQ-041-provider-compatibility-research.md:48-58`: documented provider auth, HTTPS, structured redaction, no credential material.
- `crates/providers/src/config.rs:31-72`: repository provider IDs include `openai`, `anthropic`, `google`, `deepseek`; this fixture contract selects the documented third provider `google`.
- `docs/provider-compatibility.json:3-19,35-48,62-74`: OpenAI, Anthropic, Google documented provider catalog.

## Contract implemented

Test-local stdlib validator. No network, subprocess, product parser, or secret access.

- Manifest version `"1"`, exact providers and exact six-file inventory.
- OpenAI, Anthropic, Google request method, HTTPS URL template, bounded headers, named `REDACTED` secret headers, body schema reference.
- Four fixed auth states: `logged-out`, `pending-consent`, `ready`, `expired`; consent/device values redacted; ready/expired contain only `expires_at` plus state.
- Raw secret scan: `sk-`, bearer values, token fields, private-key bytes.
- Deterministic shape diff. Unknown provider raises `UnknownProvider`.
- Canonical key ordering, duplicate-key rejection, finite JSON, file/directory/header bounds.
- Temporary negative generated bundles assert `bad-endpoint`, `secret-leak`, `over-cap`; missing directory asserts the single `bad-fixture` RED boundary.

## Serialized fixture implementation order

```json
["manifest.json", "openai/request.json", "openai/auth_state.json", "anthropic/request.json", "anthropic/auth_state.json", "google/request.json", "google/auth_state.json"]
```

## RED evidence

The repository already contains the seven fixture files from `248f519`, despite the requested fresh missing-fixture RED lane. Therefore the repository-root positive tests run GREEN; a missing fixture root fails only with `bad-fixture: missing provider_contracts directory`. No fixture files were changed. Ledger status remains `blocked`, not `completed`, because no valid repository-root RED can be established without deleting or changing out-of-scope fixtures.

## Tests and hash

- `python3 -m py_compile tests/bootstrap/test_prov024_provider_contracts.py`: PASS.
- `python3 -m unittest tests.bootstrap.test_prov024_provider_contracts -v`: 7/7 PASS against pre-existing fixtures; negative and missing-root checks pass.
- `PROV024_FIXTURE_ROOT=<missing-temp-path> python3 tests/bootstrap/test_prov024_provider_contracts.py`: expected RED; repository-positive checks fail only with `bad-fixture: missing provider_contracts directory`.
- Missing-root reproduction captured: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/prov024-red-final.log`, exit 1, 5 expected positive-path errors all `bad-fixture: missing provider_contracts directory`; negative tests pass.
- Frozen test SHA-256: `750e3a32e24a10c0b6188c23a4cbed77d9301cb43a58fcc987376d5be4d5feac`.

## Remaining blocker

Fresh RED requires the fixture directory to be absent in the candidate tree or an authorized test-only baseline. Ownership forbids deleting existing fixture files. Verifier/controller must decide whether existing fixtures are accepted as prior implementation or provide a clean no-fixture base.
