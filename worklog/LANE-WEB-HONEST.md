# LANE-WEB-HONEST — honest bundle + capabilities match

Claim: LANE-WEB-HONEST, session ses_f423cceb0ffeKndZg63lApY3Fy, owned file crates/server/src/web_assets.rs only.
Status: in-progress -> completed only on GREEN with zero test edits.

## Source evidence
- crates/server/src/web_assets.rs:10 — `include_dir!("$CARGO_MANIFEST_DIR/web_dist")`; missing dir fails build, but empty/stale bundle (no index.html) falls through to bare 404 — dishonest, masks missing bundle as missing route.
- crates/server/src/lib.rs:181-250 — `web_capabilities`: tools deny-by-default via OPENCODE_RK_TURN_TOOLS, plugins/approvals/search/research/voice false, attachments draft_ingest true + transmit false, artifacts edit true + run/apply false. Matches real write-paths (turn executor gated, no provider attachment adapter, no transcription/search adapters, no safe run executor). DO NOT TOUCH (not owned).
- crates/server/src/web_turn_adapter.rs — capability_report mirrors same flags; current() has tool_adapter false etc.
- Frozen tests (read-only): tests/web_assets.rs (WEB-006-T03: / -> 200 title, SPA fallback 200, /api/unknown -> 404), tests/web_capabilities_api.rs, tests/web_capabilities_reason.rs, tests/web_capabilities_full.rs, tests/agent_loop_turns.rs E4.
- web_dist present with index.html + assets/ (index-XBArXcct.js 494KB).

## Target boundary
Own: web_assets.rs only (+ scratchpad + ledger row). No lib.rs, no test edits, no other lanes.

## Change
- `bundle_available()`: true iff embedded bundle carries index.html (real write-path check).
- `BUNDLE_MISSING_MESSAGE` const naming web_dist/index.html + rebuild fix.
- `serve` keeps exact signature; guards order: api->404, traversal->400, bundle-missing->503 message, then identical asset/SPA/404 flow.
- Sync `serve_inner(path, lookup)` seam so missing-bundle path is unit-testable; closure bound Send+Sync, bytes &'static (real + fixture both 'static).

## Tests (in-file, zero edits post-freeze)
8 tokio tests: entry 200, hashed asset immutable cache, SPA fallback, dotted 404, api stays 404 without bundle, traversal 400 before bundle gate, missing bundle 503 + message, embedded bundle carries entry doc.

## Decisions
- 503 (not 404) for missing bundle: fail closed, explicit, keeps /api/* 404 compat.
- No capabilities JSON change: lib.rs not owned; existing flags already honest; bundle gate exposed via bundle_available() for future wiring.

## Unknowns
- None blocking. Capabilities wiring to bundle_available() left to integrator (other lane owns lib.rs).
