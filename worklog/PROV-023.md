# PROV-023 worklog (verify-only, dedicated)

## Claim
`docs/provider-compatibility.json` (2929 B, canonical key-ordered) satisfies `tasks/PROV-023.md` (versioned compatibility catalog). Frozen suite `prov_023_provider_catalog.rs` 5/5 GREEN. GREEN-on-first-run, no independent RED. Verifier decides.

## Source evidence
- Base rev `248f519`. Card owns catalog surface only; touches no product code (file pre-existing, verified only). Wired N/A (docs/JSON).
- Recorded spelling: file uses `catalog_version: "1"` accepted as the version field.
- Bounds: 3 providers (≤32), ≤16 endpoints each, 2929 B (≤256 KiB); canonical re-serialize identical; no `undocumented`/spoof/secret strings.

## Observed scenario
First run GREEN vs existing file. No valid RED.

## Target boundary
- Frozen test untracked, 5 #[test]; sha256 `257ef2ead7e9a4399963489c610dbc56d54bcba31b2c9efcc081e0a7cdde538a` (bundle `aaa184a6...`; drift noted, GREEN).
- Zero edits.

## Tests
- Rerun batch 021–024 → 20 passed, 0 failed. Serial JOBS=2 THREADS=2 timeout 120.

## Decisions
- Verify-only.

## Remaining unknowns
- Acceptance verifier-owned. ralph.json untouched.
