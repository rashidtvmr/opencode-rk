# PROV-019 scratchpad

claim: PROV-019 @ ses_f386f5b1fffeYqbqZ1uC13paZz, scratchpad worklog/PROV-019.md.

## Source evidence (commit 881d42b; files at HEAD)
- task: tasks/PROV-019.md (contract: RequestProfile/AuthHeaderKind/EndpointKind/RequestError/profile_for/headers_for/with_diagnostics, bounds/timeout/retries/redaction/prohibitions, pure/deterministic/bounded).
- owned impl: crates/providers/src/request_profile.rs (412 lines; #![forbid(unsafe_code)]; consts MAX_ENDPOINT_BYTES=2048, MAX_AUTH_HEADERS=8, MIN/DEFAULT/MAX_TIMEOUT_MS=1000/30000/120000, DEFAULT_MAX_RETRIES=1, MAX_RETRIES=3; EndpointKind{ChatCompletions,Messages}; AuthHeaderKind{Bearer,ApiKeyHeader}; RedactedMarker::Redacted serde "[REDACTED]"; RequestProfile{provider_id,endpoint,auth_headers,timeout_ms,max_retries}; RequestProfileOptions+Default; RequestError{UnknownProvider,UndocumentedEndpoint,HeaderNotAllowed,BadEndpoint,BadBounds}; RedactedDiagnostic{provider_id,endpoint,attempt,error_code:Option<String>}; EndpointSelector{Kind,Name,Owned}+From impls; profile_for/profile_for_with_bounds/profile_for_with_options/profile_for_with_limits/validate_profile/RequestProfile::validate/headers_for/with_diagnostics+with_error_code/reject_raw_headers/profile_for_with_headers/documented_entry/validate_endpoint/is_https_endpoint/validate_bounds/error_code).
- frozen tests: crates/providers/tests/prov_019_request_profile.rs (sha256 04bad144..., 94 lines, T01..T05; uses profile_for_with_bounds + profile_for_with_headers which exist in impl).
- wiring: crates/providers/src/lib.rs:50 `pub mod request_profile;` present at HEAD (lib.rs NOT owned; no edit).
- ledger: tasks/completion/claims.json has no PROV-019 row before this lane; claimed in-progress this session.

## Observed scenario
- Impl file already committed at 1be93d3 with full API incl. test-only helpers used by frozen tests (profile_for_with_bounds, profile_for_with_headers). No RED needed to be fabricated: impl present.
- Tree dirty (pre-existing, other lanes): `M AGENTS.md`, `M crates/providers/src/responses.rs`, `?? .phase1-plan-transfer-20260921/`, `?? docs/plans/`. Owned impl + frozen tests clean.

## Target boundary
- Frozen T01..T05 green, zero test edits. No changes to lib.rs/Cargo/tests/verifier. Only owned product file if repair needed, else verify-only.

## Tests
- RED-receipt limitation: no compiling RED established (impl pre-exists and matches frozen API); honest record instead of fabricated failure. Frozen test hash sha256 04bad144c0ae2ddcdeac73861688adabca706cd972cbe8d8ad1beaa1ecef302f. Impl hash sha256 77767d121f5ac9b60ea004cacac3acb0dd111a3fe33fe82972507ac848af28a5.
- Focused cmd: `rtk cargo test -p opencode-rk-providers --test prov_019_request_profile -- --test-threads=1` => 5 passed (1 suite) x2 runs.
- Adversarial: grep request_profile.rs for todo/unimplemented/stub/placeholder/mock/unsafe/net/env/fs/secret/log tokens => only benign matches (forbid(unsafe_code), secret-free docs); zero diff on owned impl + frozen tests.
- lane_gate.py: storage-wave only, no PROV-019 lane entry; per orders not run (would verify unrelated lanes, no scoping flags for this lane).

## Decisions
- Claim first, then verify. Run focused suite; adversarial checks (Debug/serde leak scan, determinism, bounds) only if green needs probing. No broad builds.

## Remaining unknowns
- lane_gate.py exact invocation (do not guess); commit/push feasibility vs orchestrator dirty tree.
- No commit/push by this lane: owned impl + frozen tests zero-diff; only scratchpad + ledger row changed, and tree carries foreign dirty files (AGENTS.md, responses.rs, untracked dirs). Landing left to orchestrator to avoid sweeping unrelated changes. Status completed reflects green frozen suite, not acceptance.
