# PROV-GATE2 worklog (verify-only, PROV-015..024)

## Claim
Verify-owned suites GREEN, no product/test edits. Fix-present: claude_oauth.rs:462 emits raw `{LOOPBACK_REDIRECT_URI}`. Verifier decides acceptance.

## Source evidence
- Base rev `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`.
- Fix line `crates/providers/src/claude_oauth.rs:32` const `LOOPBACK_REDIRECT_URI = "http://127.0.0.1:1455/oauth/callback"`; `:461-464` `begin_login` interpolates raw const, passes own `validate_consent_url` → `contains_loopback_redirect` (`:621-647`). Read-only verified, zero edits.
- `crates/providers/src/lib.rs` untouched by this lane (`git diff --stat HEAD -- crates/providers/src/lib.rs` empty; `git status --short -- crates/providers/src/lib.rs ralph.json tests/ralph.json` empty). Frozen tests + ralph.json never edited.
- Impl modules pre-exist: auth_profile, codex_oauth, local_credential_import, request_profile, auth_commands, usage_status, auth_store; catalog `docs/provider-compatibility.json`; fixtures `fixtures/provider_contracts/{openai,anthropic,google}/{request,auth_state}.json` + `manifest.json`.

## GREEN reruns (serial CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1, timeout 110, `--test-threads=1`; 5/5 each ×5 runs, bounds 6/6 ×6)
| suite | file | runs |
|---|---|---|
| auth_profile (PROV-015) | crates/providers/tests/auth_profile.rs | 5/5 ×5 |
| codex_oauth (PROV-016) | crates/providers/tests/codex_oauth.rs | 5/5 ×5 |
| codex_oauth_bounds | crates/providers/tests/codex_oauth_bounds.rs | 6/6 ×6 |
| claude_oauth (PROV-017) | crates/providers/tests/prov_017_claude_oauth.rs | 5/5 ×5 |
| local_credential_import (PROV-018) | crates/providers/tests/prov_018_local_credential_import.rs | 5/5 ×5 |
| request_profile (PROV-019) | crates/providers/tests/prov_019_request_profile.rs | 5/5 ×5 |
| auth_commands (PROV-020) | crates/providers/tests/prov_020_auth_commands.rs | 5/5 ×5 |
| usage_status (PROV-021) | crates/providers/tests/prov_021_usage_status.rs | 5/5 ×5 |
| auth_store (PROV-022) | crates/providers/tests/prov_022_auth_store.rs | 5/5 ×5 |
| provider-compatibility (PROV-023) | crates/providers/tests/prov_023_provider_catalog.rs | 5/5 ×5 |
| contracts (PROV-024) | crates/providers/tests/prov_024_provider_contracts.rs | 5/5 ×5 |

## Test file sha256 (worktree, read-only)
- auth_profile.rs `a23f9f322c959260d88fd0cbf033ce605b064ae8add93b843440a0bbd586756e`
- codex_oauth.rs `aa7459f644f45eae1815fe9c5dfc67fcee45f7ff9df5db8d2aa4369514098851`
- codex_oauth_bounds.rs `9d2c8d873731cdbf062c2531631de81d5790fc93482d324d4e3fc4bbef354e28`
- prov_017 `b5a9cb2990bf471b16ac6424f9843d7401a0ce5318404fc95aac35b9beadc4f5` (matches lane-frozen hash in PROV-017-024.md)
- prov_018 `f14ac9aa86d4fea235d6109ec959b065587814d703ee77c5d05763fb2cf755a4`
- prov_019 `04bad144c0ae2ddcdeac73861688adabca706cd972cbe8d8ad1beaa1ecef302f`
- prov_020 `8d74c4ff0429442e2b3bb714c2745f9f4e6c01c0c30cbafbd617fa14016b6896`
- prov_021 `5881c079922d647d0bc9990ec75ad20641dc1e52aecab4cfd6789cae3d169d4f`
- prov_022 `2213285518157417c532d5a2a9213cbd773af4aa2416c86711f6151301ae8c55`
- prov_023 `257ef2ead7e9a4399963489c610dbc56d54bcba31b2c9efcc081e0a7cdde538a`
- prov_024 `ba39df082941d113b959a4e984af870c8e93b2790a4352101a7cc4fd00365123`

## RED probe (behavior-stub, ≤3 strategies/suite)
- Log: `/tmp/opencode/wF-GATE2-red.log` (14 lines; binary probe + catalog/contract probes appended; sources `/tmp/opencode/wF-prov023-red.log`, `/tmp/opencode/wF-prov024-red.log`).
- Rust probe (real impl, stub-would-be-Ok inputs must Err — all caught=true): PROV015 Unknown-method, PROV016 no-grant, PROV018 no-consent, PROV019 undocumented-endpoint, PROV020 inspect-unknown, PROV021 empty-field, PROV022 absolute-escape, PROV017 empty-pkce.
- PROV023 probes: missing-version→bad-version, http-endpoint→bad-endpoint, secret-value→banned:sk- (all FAIL-closed as expected).
- PROV024 probes: missing-version→bad-fixture, http-url→bad-endpoint, secret-bytes→secret-leak (all FAIL-closed as expected).
- No frozen test file executed in RED against stubbed product code (verify-only lane; product code already GREEN). Probe asserts error arms exist and fire; real-suite T03/T04 negative tests cover same arms.

## Decisions
- Zero edits: fix already present on arrival; verify-only reruns. No `todo!/unimplemented!/stub` in owned src files (grep clean).
- Pre-existing warnings untouched (`auth.rs:4` unused `Duration`, outside owned paths).
- Mem ~2.3-3.0Gi avail during runs; one cargo command at a time.

## Remaining unknowns
- Acceptance verifier-owned. Workdir has ~396 sibling-lane modified files; none touched by this lane.
