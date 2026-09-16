# PROV-024 worklog (verify-only, dedicated)

## Claim
`fixtures/provider_contracts/{openai,anthropic,google}/{request,auth_state}.json` + `manifest.json` satisfy `tasks/PROV-024.md` (offline differential fixtures). Frozen suite `prov_024_provider_contracts.rs` 5/5 GREEN. GREEN-on-first-run, no independent RED. Verifier decides.

## Source evidence
- Base rev `248f519`. Card paths auth/config/integration/oauth/registry all exist; slice touches no product code (fixtures pre-existing, verified only).
- No product-code diff helper: `diff_shapes` is test-side comparison (header-name sets + endpoint shapes).
- Bounds: 7 files, 2314 B total (per-file ≤16 KiB, dir ≤128 KiB).

## Observed scenario
First run GREEN vs existing fixtures. No valid RED.

## Target boundary
- Frozen test untracked, 5 #[test]; sha256 `ba39df082941d113b959a4e984af870c8e93b2790a4352101a7cc4fd00365123` (bundle `e22e1dfa...`; drift noted, GREEN).
- Zero edits.

## Tests
- Rerun batch 021–024 → 20 passed, 0 failed. Serial JOBS=2 THREADS=2 timeout 120.

## Decisions
- Verify-only.

## Remaining unknowns
- Acceptance verifier-owned. ralph.json untouched.
