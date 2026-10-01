# Merge strategy run — 2026-10-01

## Phase 0–5 snapshot

- Frozen base: `8d5866e328306dbf8041d91bb41fed9a38c0971c`.
- Phase 0 SAFE root and full bundle are preserved under the approved controller root.
- Phase 0 captured 190 worktrees, 23 dirty worktrees, 419 local heads, and opaque dirty archives.
- Frozen manifest: `378be96dccc82ac775444a58524d36c0013793581a81f12f2d21a7c19128f67e`.
- Manifest inputs: 221 eligible tips in nine buckets; process-only/evidence/review dispositions remained excluded.
- L0 accounting: 16 accepted, 205 quarantined.
- L1/L2/L3/L4 reductions retained 10 initial accepted tips plus 4 retry acceptances, 6 unmerged tips, and 207 quarantined retry tips.
- Final retry head before cleanup: `d8489d63baf569ab749aaa48cbe7e23bdde6db39`.
- Merge-ready cleanup commit: `82308b9010caf459392ff65f8ee0dc135531e111`.
- Nine `.phase1-package-transfer/part-02` through `part-10` files were classified as process-only base64 transport artifacts and removed from the prospective tree only; their blobs, refs, and history remain preserved.
- Independent source-bound Cargo check for `82308b9` passed; receipt: `audit/verification/cargo-bound-823-20261001T193948Z-a9a90e30/receipt.json`.

## Scope and status

This is a strategy merge snapshot, not product acceptance or release completion. Native G5, interruption, permission, retry follow-up, release, and full runtime gates remain independently open unless separately accepted by trusted receipts.

Provider dirty bytes remain preserved in canonical `main-v2`; their known hashes are recorded outside this document. `sources/upstream.lock.json` remained unchanged through the merge reduction and cleanup.

## Phase 5 landing

- Prepared source SHA: `82308b9010caf459392ff65f8ee0dc135531e111`.
- Phase 5 integration receipt: `audit/integration/main-v2-advance-82308b9.json`.
- Current canonical `main-v2` after landing: `82308b9010caf459392ff65f8ee0dc135531e111`.
- Final Cargo verification and any later release/retry gates are separate from this metadata landing.
