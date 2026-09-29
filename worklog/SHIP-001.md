# SHIP-001: Reproducible platform artifacts and signed distribution

status: claimed in-progress, assessed BLOCKED (awaiting ledger transition)
session: ses_f1160020fffedLN4J9DoxOgYqy
lane branch: lane/SHIP-001-phase1
lane HEAD: 8a91a7b49a5a1c948218ad8f176d44e015530dcb
owned file: .github/workflows/release.yml (does not exist yet)

## Claim
- Claimed via tools/completion_claims.py claim() with session ses_f1160020fffedLN4J9DoxOgYqy, scratchpad worklog/SHIP-001.md.
- Ledger row verified: tasks/completion/claims.json SHIP-001 = in-progress, this session.
- No other session holds SHIP-001 (fence clear).

## Source evidence (exact)
- Plan story: tasks/completion/delivery.json:12: SHIP-001 title "Reproducible platform artifacts and signed distribution", deps [APP-010, TUI-011], paths ["release", ".github/workflows/release.yml"], 5 test obligations SHIP-001-T01..T05 (derived by tools/completion_plan.py:146).
- Loader status: plan['stories']['SHIP-001'] present, mandatory True, status not-started (plan loader forces not-started; progress lives in ledger per .agents/WORKER.md).
- Ready check: SHIP-001 NOT in ready_tasks() output (deps APP-010 + TUI-011 marked completed in lane ledger, but plan-level readiness evaluated False in this worktree context; orchestrator pickability delegated to parent).
- Deps ledger: APP-010 completed (ses_f42269ebaffejbo6GG4ng10o5V), TUI-011 completed (ses_orch_TUI011b). Content of dep evidence not re-verified by this lane (verifier-owned).
- No SHIP-001 task card under tasks/ (glob tasks/**/*SHIP* empty). No tests/release/ tree. No .github/workflows/release.yml. No frozen RED suite referencing SHIP-001-T01..T05 anywhere (grep tests/ tools/ validation/ docs/ fixtures/ for SHIP-001: only delivery.json + USER_GUIDE mention).
- Existing related artifacts (not SHIP-001 acceptance): scripts/install-oc2.sh (+ .ps1, install-opencode2 variants), crates/cli TUI-011 dist checks, .github/workflows/ci.yml (planning job pinned checkout 11d5960a326750d5838078e36cf38b85af677262) + completion.yml. APP-010 install-oc2.sh identity/rollback coverage noted in claims.
- Repository-protection authority: .github/CODEOWNERS (58 lines) has NO entry for .github/workflows/release.yml; .github/protection-policy.json (102 lines) protectedPaths (56 paths) does NOT list .github/workflows/release.yml either. PROTECTED_PATHS in tools/validate_protection_policy.py:83-140 likewise absent release.yml. So the owned path is outside the current integration-authority protected set.

## Observed scenario
- Target boundary per prompt: own exactly .github/workflows/release.yml + scratchpad + claim row. Required content: bounded supported-platform builds, checksums/SBOM/provenance, actual authorized signing or explicit blocking, corruption/arch checks, rollback preservation. No pretend signatures or mocked success.
- Gate state: tools/lane_gate.py LANES covers only storage gc/admission/execution/approvals/snapshot/import/quota/retention + 4 test targets. No SHIP-001 lane entry; lane_gate cannot PASS this lane.
- Canonical guard: tools/validate_repository.py REQUIRED_CHECKS = ruleset import, readback fixtures, protection, backlog exhaustion, DISC-003 manifest, plan. Does not reference release.yml, so creating the file does not by itself break the canonical chain, but ci.yml EXPECTED_PLANNING_BLOCK / workflow_contract_errors constrains only ci.yml, not a new workflow file.
- Protection risk: creating .github/workflows/release.yml under .github/ without CODEOWNERS/protection-policy coverage introduces an unowned workflow path. Per AGENTS.md + docs/REPOSITORY_PROTECTION.md, enforcement-path changes require canonical guard + ownership/review process; source policy alone is not platform proof. This lane is forbidden from editing policy/controller/manifests, so it cannot lawfully add that coverage itself.

## Target boundary (what a lawful implementation would need)
- New file .github/workflows/release.yml only: pinned actions SHAs, bounded matrix (supported Linux/macOS/Windows targets), cargo build --locked, artifact upload, sha256 checksums, SBOM/license/provenance emission, installer arch/corruption rejection (delegating to scripts/install-oc2.sh fail-closed gates), signing steps that use real authorized identities/secrets when present and hard-block otherwise (no fake signatures), rollback-preservation notes.
- Must NOT: edit tests, policy files, controller state, manifests, other workflows, scripts, or crates.

## Tests
- Frozen RED: NONE EXISTS for SHIP-001-T01..T05. No test file, no manifest hash, no command manifest frozen by controller/test-author.
- TDD contract (docs/TDD.md sections 3-4): RED suite must compile and fail for missing behavior before implementation; frozen hash recorded. Absent.
- Worker rule: existing/frozen tests immutable; new tests authored only by independent test-author role before freeze. This lane received no frozen suite and may not invent product tests as substitutes (PLAN.md section 6: declarative task still needs executable validators + captured failing fixture, owned by test role, not implementer).
- Therefore implementation now would violate "no implementation without lawful compile-valid behavioral RED already frozen".

## Decisions
- NO FILE WRITTEN to .github/workflows/release.yml. Reason: (1) no frozen RED suite exists for SHIP-001-T01..T05; writing the workflow would be untested implementation, violating TDD contract and the prompt's own "implement only if lawful RED already frozen" condition. (2) Signing-authority evidence missing: no authorized macOS notarization / Windows signing identity or secrets-grant record available to this lane; SECURITY.md sections 5-6 prohibit fabricating credentials/consent; a workflow with pretend signing would violate the no-mock rule. Correct terminal state is BLOCKED, not untested GREEN.
- No commit/push of implementation. Only ledger claim + this scratchpad exist as lane output.

## Remaining unknowns / exact blockers
1. BLOCKER-1 missing-frozen-RED: no test-author-frozen compile-valid behavioral RED for obligations SHIP-001-T01 (clean builders compile pinned app + bundle assets), T02 (checksum/SBOM/license/provenance identify inputs), T03 (real authorized signing or blocked), T04 (arch/corruption rejection), T05 (upgrade preserves data + rollback). Need: frozen test paths + sha256 + command manifest from trusted controller/test-author.
2. BLOCKER-2 missing-signing-authority: no record of authorized Apple notarization identity / Windows code-signing identity, secret-grant mechanism, or explicit block-if-absent policy approval. Per prompt, actual identities or explicit blocking required; lane holds neither. Need: orchestrator/human-supplied signing authority decision + secret wiring owned outside this lane.
3. BLOCKER-3 protection-coverage: .github/workflows/release.yml is a new .github/ workflow path with no CODEOWNERS owner and no protection-policy.json protectedPaths entry. Need: integration-authority decision (via docs/REPOSITORY_PROTECTION.md process + canonical guard) on ownership/required-checks for the new workflow before or with landing.
4. NOTE readiness: ready_tasks() returns False for SHIP-001 in this worktree; orchestrator to rule on pickability before re-delegation.

## Next safe step (for orchestrator)
- Route to independent test-author role to author + freeze SHIP-001-T01..T05 executable validators (workflow static checks + installer-behavior fixtures with failing capture), record frozen sha + commands, resolve signing-authority + protection-coverage decisions, then re-delegate implementation lane.

## Pre-freeze correction (ses_f114747d5ffe4NBs8gtP1q3PDv) 2026-09-29
- Reclaimed SHIP-001 not-started via claim(); owned file only release/test_release_contract.py.
- Correction: old T04 used valid host shell script with foreign identity (NOT wrong arch; executes natively). Replaced with deterministic nonempty executable in actual foreign binary format: ELF64/x86_64 bytes on Darwin arm64, Mach-O ARM64 bytes on Linux/other; asserts fixture magic + host mismatch before archive install; installer must reject (nonzero) and preserve preexisting binary byte-identically (sha256 equal). T05 identity-failing rollback kept separate; other assertions untouched.
- RED run: python3 -m unittest release.test_release_contract -v => 6 tests, 5 FAIL / 1 ok (T04-sanity rc0). Fail reasons: T01 release.yml absent; T02 release.yml absent; T03 'codesign' missing in installer code; T04 preexisting binary deleted on rejection (foreign ELF/Mach-O install ran + rm -f); T05 v1 binary not restored after failed upgrade. py_compile OK.
- New SHA-256 RED: 2decf1d282a8975343183fec934b75ccd4355c83b1c98d576ddd110083345a6e
- Blockers: impl (installer backup-restore + staged identity/arch gate) + freeze authority + signing authority. Setting blocked. No workflow/installer/policy edits.
