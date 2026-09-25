# PHASE1-VERTICAL-SYNTHESIS-VERIFY — independent static verifier

## Claim and boundary
- Task: `PHASE1-VERTICAL-SYNTHESIS-VERIFY`
- Session: `ses_f2896ea5bffevOmeLxBgzaH0A6`
- Branch: `verify/PHASE1-VERTICAL-SYNTHESIS`, HEAD `7262682e31c1f473912c9483804c9430eb4ee4c5` (matches expected candidate).
- Baseline cited in proposal `8a91a7b49a5a1c948218ad8f176d44e015530dcb` resolves as commit object.
- Read-only verifier. No Cargo/product/test/controller edits. Owned files only: this worklog + own claim row.

## Method
Parsed the single JSON block in `worklog/PHASE1-VERTICAL-SYNTHESIS.md:156-385` with stdlib `json`; rechecked every claimed invariant independently. `git diff --check`, `git cat-file -t` for all cited revisions, `tools/validate_repository.py`, `tools/convergence_gate.py` (read-only).

## Verification results (all PASS)
- Lanes: 20. Roles: 13×`breadth` + 1×`breadth-test-author` (B14) + 4×`integration-spine` + 2×`independent-verifier`. B14 counts toward the 14 breadth slots per capacity block (`breadth:14`); mapping is explicit, not a defect.
- Stages: **83**, dependency edges: **162**. Stated counts 83/162 **match exactly**.
- Stage IDs unique (83/83). All deps resolve (0 unknown). Acyclic (`graphlib` topo order length 83). No dep on a later wave (0 violations).
- `ownedPath`: every stage carries exactly one scalar (string or null); 0 missing/malformed. Null-owned stages (14) are all `authority` / `serialized-main-integration` (2 nulls) / `independent-verifier` / `freeze-authority` — read-only roles, correct.
- Cargo weights: all in {0,1}; 67×1 / 16×0; `cargoSemaphore:1` declared. Correct.
- Same-path analysis: 3 shared paths, all cross-wave or dep-ordered, **0 unordered cross-lane same-wave collisions**:
  - `crates/security/src/lib.rs` wave 2 (B03×2 + I02 `I2-PROTECTED-BASE`): ordered — `PATH-HARDLINK-IMPL` deps include `I2-PROTECTED-BASE`. Note: integration writes before breadth impl here (reversed vs usual order) but edge makes it deterministic; flagged as fragility, not defect.
  - `crates/cli/src/main.rs` wave 2 (B04 + I02): ordered — `I2-CLI-RECEIPT-WIRE` deps include `MAC-ENTRY-IMPL`. Correct spine-after-breadth shape.
  - `scripts/install-oc2.sh` waves 2→3 (B06 → I04 `I4-POSIX-MERGE`): cross-wave, ordered. Correct.
- Implementation RED+freeze ancestry holds **transitively** for all 30+ impl/integration stages. 6 stages lack a *direct* RED+freeze pair (`APR-AUTH/STORAGE/SERVER-IMPL` chain via `V1-APPROVAL-API-FREEZE`, `MAC-TUI-IMPL` via `MAC-ENTRY-IMPL`, `PATH-OPEN-IMPL` freeze via `PATH-HARDLINK-IMPL` chain, `PKG-PRODUCER-IMPL` freeze via `PKG-RUNTIME-IMPL`) — each has full transitive RED+freeze ancestry (19–20 REDs + freeze). Chained-impl shape; suggestion (not defect): add explicit `V1-FREEZE-RED` dep to `MAC-TUI-IMPL` for uniformity.
- External gates closed: 6/6 (`G-AUTHORITY`, `G-LINUX-RUNNER`, `G-WINDOWS-RUNNER`, `G-KEYRING-SEAM`, `G-APPROVAL-PREWIRE` all `blocked`; `G-SIGNING` `external-optional`). All 8 gated stages reference existing gate IDs. Approval chain after `61b0959`: `APR-ROUTE-RED` gated, `V1-APPROVAL-API-FREEZE` gated, downstream digest/auth/schema/coordinator + `I1-APPROVAL-SCHEMA` all depend on the API freeze; Manifest-A REDs (expiry/retention/dispatch) correctly ungated on existing seams. Matches `61b0959` Manifest A/B split.
- Revisions: all 9 cited (`3ad58b0…`, `2f87242…`, `c269715`, `55acf29`, `0fb0707`, `7463dbe`, `61b0959`, `4814357`, `576cda6`) resolve via `git cat-file -t` = commit. Integration order `I4-WAVE2-MERGE → RECOVERY → FILEOPS → BATCH → POSIX → EXACT-REVISION` verified as strict dep chain.
- Platforms: only `windows-x86_64-pc-windows-gnu`; `windows-msvc` in `unsupported`; zero msvc/aarch64-pc-windows/i686 hits in stages. Unsigned: target `unsigned-phase1-candidate`, signing gate non-blocking. Keyring pin exact `=3.6.3` in both body and JSON gate condition.
- Read-only gates: `git diff --check` PASS; `validate_repository.py` FAIL (inherited backlog-exhaustion); `convergence_gate.py` total=59 (proposal cites 58 baseline + notes later-branch accumulation — consistent, both inherited blockers honestly recorded, not acceptance evidence).

## Verdict
**ACCEPT-SCOPED** — proposal-only artifact, no authority/product/test mutation. Zero blocking defects. Non-blocking notes: (1) B14 role-label→breadth-bucket mapping; (2) B03/I02 lib.rs write order (integration-before-breadth, dep-ordered); (3) `MAC-TUI-IMPL` direct-freeze-dep suggestion; (4) `separation`/`contract` prose-level fields, correctly not per-stage keys.

## Evidence commands
- `git rev-parse HEAD` → `7262682e…` ; `git cat-file -t` ×9 → all `commit`
- stdlib JSON parse + graph checks → 20 lanes / 83 stages / 162 edges / acyclic / 0 unknown / 0 later-wave
- `git diff --check` → PASS; `python3 tools/validate_repository.py` → FAIL (inherited); `python3 tools/convergence_gate.py` → total=59 (inherited)
