# PROV-023-RUNTIME-RED

## Claim
- Task: PROV-023 (RED author lane)
- Session: ses_f31e1ec20ffejsuBLUqe3nRbsb
- Branch: lane/PROV-023-runtime-red-20260923 @ 5e0e64c
- Scratchpad: worklog/PROV-023-RUNTIME-RED.md
- Owned file: crates/providers/tests/prov_023_runtime_catalog.rs

## Source evidence
- tasks/PROV-023.md:3 Runtime optional False; users + PROV-019 profiles resolve only documented entries; reject malformed/unknown, never guess.
- crates/providers/src/lib.rs:50 `pub mod request_profile` - only runtime request-path seam.
- crates/providers/src/request_profile.rs:162-167 `profile_for`, 187-206 `profile_for_with_options`, 95-109 `RequestError::{UnknownProvider,UndocumentedEndpoint,BadEndpoint,BadBounds}`.
- crates/providers/src/request_profile.rs:302-350 `documented_entry` hardcodes openai/anthropic only; no catalog read.
- crates/providers/src/catalog_sync.rs:54 `plan_catalog_sync` - snapshot diff planner, takes `&[CatalogEntry]`, no bytes/version/https/secret handling.
- docs/provider-compatibility.json:1-90 version "1", providers openai/anthropic/google (google generate-content).
- crates/providers/tests/prov_023_provider_catalog.rs:1-125 frozen docs-only JSON checks (test-local lookup, no runtime caller).
- grep `provider-compatibility` across crates/docs/tools hits only the frozen test - no product reader.

## Observed scenario
Runtime request path resolves openai/anthropic from hardcoded strings. Catalog documents third provider `google`/`generate-content`. No public API accepts catalog bytes, so missing/malformed/bad-version/over-cap catalog rejection has no callable seam.

## Target boundary
Consumer-side RED via existing public seam `request_profile::profile_for` only. No file I/O, no network, no secrets, no test-local lookup reimplementation, no product edits, no existing-test edits.

## Tests (owned file)
- T01 resolves documented third provider google/generate-content (expects Ok; RED fails with UnknownProvider).
- T02 unknown provider fails closed UnknownProvider, no guessed URL.
- T03 unknown endpoint kind fails closed UndocumentedEndpoint.
- T04 resolved profile rendering carries no secret bytes.

## Decisions
- T01 asserts https + host properties, not exact template, to avoid over-pinning placeholder substitution design.
- Loader-side rejection (missing/bad-version/over-cap bytes) left as documented gap: needs new public loader API from GREEN owner; cannot be RED-tested without fabricating an uncompilable seam.

## Remaining unknowns
- GREEN owner + product path assignment (integration authority decision).
- Whether GREEN wires profile_for to catalog internally or adds loader API first.

## RED result
- CMD: CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 cargo test -p opencode-rk-providers --test prov_023_runtime_catalog -- --test-threads=1
- T01 FAIL (UnknownProvider on google), T02-T04 PASS. File sha256 recorded at freeze.
