# PROV-014 worklog

## Claim

Implement the dependency-free PROV-014 provider debug-export slice as a bounded, redacted JSONL bundle with deduplicated definitions and a self-contained offline renderer.

## Source evidence

- Candidate base revision: `991ae5bb550cb102552bb7eec70b9789f1f77d22`.
- `tasks/PROV-014.md`: REQ-038, five test obligations, no authored dependencies.
- `docs/upcoming-features/claude-code-reverse-harness.md:7,9-15,18-19,38-41,82-85`: reverse source pin `c0d99ea1ab7168c12ba74838cfea355ce10f6c56`; parser/viewer pipeline, prompt/tool definition dedupe, read-only observed log shape, and recommendation for a bounded redacted offline bundle with no CDN dependency.
- Current code: `crates/providers/src/debug_export.rs` is a one-line stub and is already exported from `crates/providers/src/lib.rs`.

## Observed scenario

No native debug-export behavior exists yet. The provider crate has request metrics and tap-adjacent modules, but no bounded redacted artifact suitable for offline bug reports.

## Target boundary

- Product implementation: `crates/providers/src/debug_export.rs` only.
- Independent RED tests: `crates/providers/tests/debug_export_bundle.rs` only.
- Status/evidence bookkeeping: this worklog and `tasks/PROV-014.md`; `ralph.json` already records PROV-014 as `in-progress`.

## Tests

- Independent RED file: `crates/providers/tests/debug_export_bundle.rs`.
- Frozen SHA-256: `793b36ac24d86e34d4d207313e19a0dc65a79ed4b8d161822aa1d4cbda60885c`.
- Initial authoring run against the one-line stub failed to compile with unresolved expected debug-export symbols; this was authoring feedback only.
- After a signature-only scaffold, `cargo test -p opencode-rk-providers --test debug_export_bundle` compiled and failed behaviorally: 0 passed, 5 failed. This is the frozen RED baseline.
- GREEN: `cargo test -p opencode-rk-providers --test debug_export_bundle` -> 5 passed, 0 failed.
- Regression: `cargo test -p opencode-rk-providers --lib --test config_from_env --test debug_export_bundle` -> provider lib 45 passed, config 1 passed, debug export 5 passed.
- Plan validation: `python3 tools/validate_plan.py` -> `validate_plan: OK stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `git diff --check` for the PROV-014 product/test/task/worklog paths passed.
- A whole-provider test run also passed every provider test except the intentionally frozen ROUTE-012 RED suite, which remains gated and is unrelated to PROV-014.

## Decisions

- Export remains pure/in-memory over caller-provided bounded records; no network calls or detached tasks.
- Redaction happens before serialization.
- Repeated definitions are represented once and referenced thereafter.
- Output budget exhaustion must emit an explicit truncation marker instead of retaining unbounded data.
- Offline renderer must be self-contained and avoid CDN/network dependencies.
- Redaction recursively replaces every non-empty caller-supplied secret value before serialization and never logs the original secret.
- Both record and byte budgets count the explicit truncation marker; completed records are evicted only as needed to make that marker fit.
- The exporter performs no filesystem or network I/O and retains no state after the call.

## Remaining unknowns

- Formal acceptance remains verifier/controller-owned; this worklog records implementation and test evidence only.
