# PROV-GATE3 worklog (verify-only, PROV-015..024)

## Claim
Verify-owned suites GREEN, no product/test edits. Fix-present: claude_oauth.rs:462 emits raw `{LOOPBACK_REDIRECT_URI}`. Verifier decides acceptance.

## Source evidence
- Base rev `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`.
- Fix line `crates/providers/src/claude_oauth.rs:32` const `LOOPBACK_REDIRECT_URI = "http://127.0.0.1:1455/oauth/callback"`; `:461-464` `begin_login` interpolates raw const into `{CONSENT_ORIGIN}?redirect_uri={LOOPBACK_REDIRECT_URI}&code_challenge=...&code_challenge_method=S256`, passes own `validate_consent_url`. Read-only verified, zero edits.
- `crates/providers/src/lib.rs` untouched by this lane (`git diff --stat HEAD -- crates/providers/src/lib.rs` empty; `git status --short -- crates/providers/src/lib.rs ralph.json tests/ralph.json` empty). Frozen tests + ralph.json never edited.
- No `todo!/unimplemented!/stub` (case-insensitive grep count 0) in 8 owned src files (auth_profile, codex_oauth, claude_oauth, local_credential_import, request_profile, auth_commands, usage_status, auth_store).

## GREEN reruns (serial CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1, timeout 120, `--test-threads=1`; 1 run each this lane)
| suite | target | result |
|---|---|---|
| auth_profile (PROV-015) | --test auth_profile | 5/5 |
| codex_oauth (PROV-016) | --test codex_oauth | 5/5 |
| codex_oauth_bounds | --test codex_oauth_bounds | 6/6 |
| claude_oauth (PROV-017) | --test prov_017_claude_oauth | 5/5 |
| local_credential_import (PROV-018) | --test prov_018_local_credential_import | 5/5 |
| request_profile (PROV-019) | --test prov_019_request_profile | 5/5 |
| auth_commands (PROV-020) | --test prov_020_auth_commands | 5/5 |
| usage_status (PROV-021) | --test prov_021_usage_status | 5/5 |
| auth_store (PROV-022) | --test prov_022_auth_store | 5/5 |
| provider-compatibility (PROV-023) | --test prov_023_provider_catalog | 5/5 |
| contracts (PROV-024) | --test prov_024_provider_contracts | 5/5 |

## Test file sha256 (worktree, read-only; all match GATE2)
- auth_profile.rs `a23f9f322c959260d88fd0cbf033ce605b064ae8add93b843440a0bbd586756e`
- codex_oauth.rs `aa7459f644f45eae1815fe9c5dfc67fcee45f7ff9df5db8d2aa4369514098851`
- codex_oauth_bounds.rs `9d2c8d873731cdbf062c2531631de81d5790fc93482d324d4e3fc4bbef354e28`
- prov_017 `b5a9cb2990bf471b16ac6424f9843d7401a0ce5318404fc95aac35b9beadc4f5`
- prov_018 `f14ac9aa86d4fea235d6109ec959b065587814d703ee77c5d05763fb2cf755a4`
- prov_019 `04bad144c0ae2ddcdeac73861688adabca706cd972cbe8d8ad1beaa1ecef302f`
- prov_020 `8d74c4ff0429442e2b3bb714c2745f9f4e6c01c0c30cbafbd617fa14016b6896`
- prov_021 `5881c079922d647d0bc9990ec75ad20641dc1e52aecab4cfd6789cae3d169d4f`
- prov_022 `2213285518157417c532d5a2a9213cbd773af4aa2416c86711f6151301ae8c55`
- prov_023 `257ef2ead7e9a4399963489c610dbc56d54bcba31b2c9efcc081e0a7cdde538a`
- prov_024 `ba39df082941d113b959a4e984af870c8e93b2790a4352101a7cc4fd00365123`

## RED probe (behavior-stub, 3 strategies max, weakest-first 019/023/024)
- Log: `/tmp/opencode/xG-GATE3-red.log` (static + desk-sim + real-arm-execution; no frozen test run against stubbed product).
- S1 static-stub-kill: Ok-stub violates prov_019 T03 (UndocumentedEndpoint/HeaderNotAllowed/BadEndpoint/BadBounds/UnknownProvider) + T01/T02/T05 no-`sk-` asserts; prov_023 T04 banned-string/undocumented asserts; prov_024 T04 secret-scan + T05 explicit unknown-Err.
- S2 stub-simulation: desk execution, Ok-on-invalid-input fails first negative assert per suite; zero repo mutation.
- S3 real-arm execution: GREEN runs this lane show error arms fire (all caught=true); stub lacking them cannot pass.

## Decisions
- Zero edits: fix already present on arrival; verify-only reruns.
- Mem ~2.3-2.6Gi avail during runs; one cargo command at a time.

## Remaining unknowns
- Acceptance verifier-owned. Workdir has ~100 sibling-lane modified files; none touched by this lane.
