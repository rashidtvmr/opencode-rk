# PROV-024

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-041.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: PROV-024-T01, PROV-024-T02, PROV-024-T03, PROV-024-T04, PROV-024-T05.
Ownership locks: fixtures/provider_contracts/ only (task card only; fixture contents plus one differential test fragment owned here, no product code edits).
Suggested module: fixtures/provider_contracts/ (new redacted offline fixtures for OpenAI, Anthropic, and one additional provider; no live credentials).

## User-observable outcome

Offline differential request-shape and auth-state fixtures for OpenAI, Anthropic, and other supported providers: redacted request shapes plus auth-state snapshots let tests diff provider behavior without network and without live credentials.

## Source evidence

- crates/providers/src/config.rs:7-22 ProviderConfig (provider_id, base_url, api_key_env, timeout_secs).
- crates/providers/src/auth.rs:6-18 AuthMethod; :40-101 AuthHandler and in-memory refresh.
- crates/providers/src/oauth_flow.rs:1-65 planner validates HTTPS metadata only and performs no network.
- crates/providers/src/integration.rs:1-5 explicitly stops before credential storage/external OAuth; :11-18 bounded constants.
- crates/providers/src/lib.rs:4-48 provider modules.
- crates/providers/src/registry.rs and model_route.rs for provider/model routing.
- docs/SECURITY.md:14-32 brokered permissions, no direct secret access/unrestricted env.
- docs/research/REQ-041-provider-compatibility-research.md:1 REQ-041 official provider compatibility and CLI authentication scope.

## Observable contract

- `fixtures/provider_contracts/<provider>/request.json`: documented method, https URL template, named auth headers with REDACTED values, bounded body schema reference; no real secrets.
- `fixtures/provider_contracts/<provider>/auth_state.json`: redacted lifecycle state (logged-out, pending-consent, ready-shape with expires_at only, expired); zero token bytes.
- `fixtures/provider_contracts/manifest.json`: `{ version: "1", providers: ["openai", "anthropic", <third>], files: [...] }`; consumers reject missing version.
- Differential check `diff_shapes(a, b) -> ShapeDiff { same_endpoint_shape: bool, header_name_diff: Vec<String>, notes }`: compares shapes only, never secrets; deterministic given same fixtures.
- Minimum coverage: openai, anthropic, plus one additional supported provider; each with request.json plus auth_state.json.
- Bounded: per-file max 16 KiB, total dir max 128 KiB; header names max 16 per fixture.
- Deterministic: canonical JSON key order; same fixtures yield byte-identical diff output; no network, no clock.
- Suggested file boundary: fixtures/provider_contracts/ owning all fixtures plus manifest; one test fragment (owned here) loads and diffs them.

## Failure states

- Missing manifest version or provider dir: test fails with `bad-fixture`; whole set rejected, never partially applied.
- Non-HTTPS URL template in any fixture: test fails with `bad-endpoint`.
- Secret-like bytes detected (`sk-`, `sk-ant-`, `Bearer ` plus token, `refresh_token` value): test fails closed with `secret-leak`; fixtures rejected.
- Over-cap file or dir: test fails with `over-cap`; fixtures rejected whole.
- Unknown provider queried: diff helper returns explicit UnknownProvider, never synthesizes a shape.

## Resource bounds

- Static fixtures only; test reads once, O(bytes); no network, no subprocess, no retained state.
- Bounded files and counts; no unbounded queue, no unbounded retained output; zero runtime cost outside tests.

## Acceptance criteria

- PROV-024-T01: openai fixture valid: request.json has https URL, named auth headers with REDACTED values, body schema ref; auth_state.json has redacted lifecycle with no token bytes.
- PROV-024-T02: anthropic fixture valid: same shape checks as T01 for anthropic paths; manifest lists all three providers with exact file inventory.
- PROV-024-T03: differential: diff_shapes openai vs anthropic reports header-name diff non-empty and same_endpoint_shape false deterministically across reruns.
- PROV-024-T04: leak and cap scan: repo-wide scan of fixtures finds no secret patterns; every file within 16 KiB and dir within 128 KiB.
- PROV-024-T05: offline proof: differential test passes with network disabled (no socket use); unknown-provider diff yields UnknownProvider.

## Test-first execution

- RED: author tests PROV-024-T01..T05 as fixture validation plus differential checks; establish compiling RED (fail: no fixtures/provider_contracts/).
- GREEN: write minimum redacted fixtures plus manifest meeting caps; GREEN, refactor, rerun; negative tests (bad-endpoint, secret-leak, over-cap, unknown-provider).
- Evidence: fixture tests pass offline; `git status --short -- fixtures/provider_contracts tasks/PROV-024.md`; frozen test hash plus command manifest; verifier decides acceptance.
