# CONFIRM-P11 — VERIFY-ONLY PROV 12 suites + claude_oauth.rs read-only check

Rev: `b60ceda1eaec7f3a7df0c0eb17328a6eef6368dc`.
Workdir: /home/rashid/projects/opencode-rk.
Dirty-on-arrival: y (ralph.json M 82+/82- pre-existing; FEATURES.md, REL-00x worklogs dirty; sibling-lane untracked CONFIRM-P8/P10, ACCEPTANCE-FLIP-PROPOSAL.md, TWINS-WATCH7.md, GUARD-TRIAGE-20.md, STUB-FMT-11.md). Not this lane.
Lease: VERIFY-ONLY. No product/test edits. No frozen/lib.rs/ralph.json touches by this lane.

## Serial protocol
Prebuilt test binaries, one at a time, `timeout 120`, `--test-threads=1`, `free -h` before runs (~1.0Gi avail, 6.2Gi total). No cargo build (low-mem host). Log `/tmp/opencode/p11-prov.log`.

## Read-only fix check (claude_oauth.rs, no edit)
- `crates/providers/src/claude_oauth.rs:32` (read): `pub const LOOPBACK_REDIRECT_URI: &str = "http://127.0.0.1:1455/oauth/callback";`
- `crates/providers/src/claude_oauth.rs:461-464` (read): `begin_login` interpolates raw `{CONSENT_ORIGIN}?redirect_uri={LOOPBACK_REDIRECT_URI}&code_challenge={pkce_challenge}&code_challenge_method=S256`, delegates to `begin_login_with_url` -> `validate_consent_url`.
- Fix-present: YES.

## Results (12 suites = 62 GREEN, log /tmp/opencode/p11-prov.log)

| suite | tests | note |
|---|---|---|
| auth_profile (PROV-015) | 5/5 | prebuilt binary |
| codex_oauth (PROV-016) | 5/5 | prebuilt binary |
| codex_oauth_bounds | 6/6 | prebuilt binary |
| codex_oauth_bounds2 | 6/6 | prebuilt binary |
| prov_017_claude_oauth | 5/5 | prebuilt binary |
| prov_018_local_credential_import | 5/5 | prebuilt binary |
| prov_019_request_profile | 5/5 | prebuilt binary |
| prov_020_auth_commands | 5/5 | prebuilt binary |
| prov_021_usage_status | 5/5 | prebuilt binary |
| prov_022_auth_store | 5/5 | prebuilt binary |
| prov_023_provider_catalog | 5/5 | prebuilt binary |
| prov_024_provider_contracts | 5/5 | prebuilt binary |
| tap unit (PROV-011, lib binary filter `tap::`) | 5/5 | informational; PROV-011 has no integration target, unit tests live in `src/tap.rs:127-231` |

Integration total: 12 suites, 62/62 passed, 0 failed. With tap unit info: 67 passed.

## Stub scan
`todo!|unimplemented!|stub` (case-insensitive) over `claude_oauth.rs` + `tap.rs`: 0 hits. No stubs.

## Guards
- `git status --porcelain`: lane touched only `worklog/CONFIRM-P11.md` (this file). ralph.json/FEATURES.md/REL-00x diffs pre-existing + sibling-lane drift, untouched.
- No `frozen/` dir in tree; no frozen touch. No `*/lib.rs` edit. No `ralph.json` edit.
- No cargo build/test compile invoked by this lane (prebuilt binaries only); no hash freeze changed.

Edits: this file only.
