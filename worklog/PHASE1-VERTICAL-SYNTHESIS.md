# PHASE1-VERTICAL-SYNTHESIS

## Claim and boundary

- Task: `PHASE1-VERTICAL-SYNTHESIS`
- Session: `ses_f3c4de578ffelQv59xDXmOs03B`
- Branch: `plan/PHASE1-VERTICAL-SYNTHESIS`
- Base: `origin/main` at `8a91a7b49a5a1c948218ad8f176d44e015530dcb`
- Owned artifact: this worklog plus this task's ledger row.
- This is a proposal only. It does not edit `ralph.json`, task cards, controller
  state, accepted flags, protected policy, source, tests, or verifier config.

## Source plan revisions

| Area | Revision | Status |
|---|---|---|
| macOS product | `68ff545` | landed planning artifact |
| security/durability | `baf1e85` | landed planning artifact |
| packaging/release | `e8da066` | landed planning artifact |
| Windows GNU | `a039296` | landed planning artifact |
| worker/wave DAG | `9a7d792` | landed planning artifact |
| Linux | `b924d9a` | landed audited planning artifact; SHA-256 `58b12ddc32b76a13b1a1e31bb09c4ef36910d7ab22b69082bfd55b3390e1df27` |

Supporting accepted or scoped candidate evidence includes package RED
`ca7a2df`, protected-path verifier `8916868`, restart verifier `48d0351`,
recovery child RED `2d0a6fe`, batch verifier `30de11a`, cancellation verifier
`0fb0707`, approval correction verifier `d859916`, and integration-guard plan
`576cda6`. These revisions are not treated as integrated `origin/main` proof.

Post-map implementation candidates are recovery child-bound `3ad58b0` and the
POSIX package consumer `2f87242`. They are pushed candidate branches, not
independent acceptance and not `origin/main` integration evidence.

The approval/resume R1-R6 prewire correction is landed at `61b0959` (artifact
SHA-256 `324effe1035077ba1b99db3fdebd7d4ac55086a7a069f3281ebc28318bec7e13`).
It authorizes no API-dependent RED: expiry, state-3 retention, and concrete
dispatch can use current public seams; principal/origin, canonical digest,
schema/opaque ID, atomic claim, durable cap, outbox integration, keyring, and
device signature remain blocked until an implementation-independent compiling
seam and authority decision exist.

## Observable release boundary

The proposed task graph must converge on one installed unsigned `oc2` journey:
fresh disposable HOME, singleton authenticated daemon, setup when credentials are
absent, real native OpenTUI, shared daemon turn execution, provider fixture,
brokered tool authorization, durable persistence, exit/restart/resume, second
client observation, denial/interruption/daemon-restart/terminal restoration, and
an independent rerun on the exact integrated revision. Linux additionally
requires real Landlock execution; Windows support is only
`x86_64-pc-windows-gnu`. Signing and notarization remain external.

## Scheduling invariants

- 20 slots: at most 14 breadth, at least 4 integration-spine, and at least 2
  independent test/verifier lanes.
- Global Cargo-heavy semaphore: 1. Default `CARGO_BUILD_JOBS=2` and
  `RUST_TEST_THREADS=2`, reduced to 1 for storage and constrained runs.
- Each stage owns one product or test file, plus its scratchpad and claim row.
- Shared `lib.rs`, Cargo manifests/lockfiles, schemas, workflow files, route
  registries, and controller state are serialized integration work.
- Lifecycle: source evidence -> compiling behavioral RED -> frozen hash -> GREEN
  implementation -> independent verifier -> serialized integration -> rerun on
  exact `origin/main`.
- A feature chain is vertical, but one worker cannot own test authoring,
  implementation, verification, and acceptance.
- Parent tasks remain open while any note admits a repair child, missing caller,
  unwired path, partial proof, runner gap, or external authority.

## Current authority blockers

- Canonical repository validation has 51 pre-existing backlog/ownership errors.
- Convergence has 58 pre-existing ledger findings on the `origin/main` planning
  baseline; later branch totals can be larger because off-plan rows accumulate.
- No lane-safe authority exists here to rewrite controller state or accepted
  flags. Integration requires the controller/repository owner process described
  by `576cda6`.
- Keyring persistence remains blocked by the missing implementation-independent
  behavioral seam; dependency pin, if later authorized, must be exactly
  `keyring = 3.6.3`.
- Linux and Windows installed acceptance require real target runners. Signing and
  notarization require external identities and remain outside this graph.

## Synthesis checklist

1. Imported and deduplicated the six domain maps.
2. Reconciled task IDs, single-file stage ownership, runner requirements,
   Cargo weight, frozen evidence, and parent completion rules.
3. Produced proposal-only task/Ralph JSON without modifying authority files.
4. Validated JSON parsing, unique IDs, dependency closure, acyclicity, wave
   ordering, 14/4/2 lane allocation, and concurrent path ownership.
5. Recorded the critical path, wave exits, no-ETA rule, and external blockers.

## Cross-map audit findings to reconcile

The source maps are proposals, not authority. They cannot be concatenated
blindly:

- `9a7d792` has dangling dependencies (`PH1-CATALOG-IMPL`,
  `PH1-BOUNDS-IMPL`, and `PH1-V-LOCAL` instead of an actual stage ID). The
  final proposal must reject every unknown dependency.
- `e8da066` makes the producer depend on SBOM while the SBOM task depends on
  the producer. The final graph will serialize both responsibilities in one
  producer-file chain or prewire distinct files; it will contain no cycle.
- The POSIX frozen T01-T05 suite can drive its consumer from deterministic
  fixtures and therefore does not need to wait for a real producer. Producer
  output is mandatory later for package-binding and installed acceptance.
- `e8da066` assigns an existing/frozen installed test as an implementation
  file in one proposal. Existing tests remain verifier-owned and immutable;
  product repairs must name real caller files instead.
- `a039296` similarly gives `WGNU-008` the same E2E file as implementation and
  test ownership. The synthesis splits Windows product callers from the
  independent installed verifier.
- `a039296` makes signing part of its final release decision. This project may
  produce only an **unsigned** Phase 1 candidate; Authenticode, Apple signing,
  and notarization stay external and cannot block an honestly labeled unsigned
  candidate.
- `68ff545` cites a branch candidate revision rather than the planning base and
  was originally written to the wrong worktree. The recovered artifact is
  useful discovery input, not integrated source truth. Every executable task
  must reverify its cited caller on its actual base.
- `68ff545` notes `installed_default_entrypoint.rs` contains `todo!()` bodies.
  A controller/test-author decision is required before any repair/refreeze;
  implementers cannot edit or silently replace that frozen test.
- `baf1e85` correctly keeps APP-012 open and identifies keyring as
  authority-blocked. The final graph must preserve that blocker and must pin a
  future authorized Cargo dependency exactly as version `=3.6.3`.

These corrections are synthesis constraints. They do not mutate or invalidate
the source planning branches.

## Proposed task/Ralph payload

The JSON below is the authority-review payload. It is intentionally checked in
only as a worklog artifact. After review, integration authority may translate
each `stage` into a task object and each `laneId` into scheduler capacity; this
lane does not write those authoritative files.

Stage rules:

- Dependencies are stage IDs in this payload. `externalGates` are explicit
  non-worker decisions or runner receipts and are never silently marked done.
- A stage leases exactly one `ownedPath`, or `null` for read-only verification,
  authority, or runner evidence. Scratchpad and own claim-row writes are
  implicit and are not shared product ownership.
- Stages in one lane execute serially. A new session must claim every stage;
  `separation` prevents the RED author, implementer, verifier, and integration
  authority from collapsing into one worker.
- `cargoWeight: 1` acquires the global semaphore. Only one such stage runs at a
  time. `cargoWeight: 0` is text/static/runner orchestration only.
- Existing/frozen tests are read-only. New RED files must compile and fail for
  the missing behavior before `V1-FREEZE-RED` records hashes. If a proposed RED
  is already GREEN, the verifier records existing behavior and cancels the
  unnecessary implementation stage; nobody fabricates failure.

```json
{
  "schema": "phase1.vertical_synthesis.v1",
  "status": "proposal-only-not-accepted",
  "baseline": "8a91a7b49a5a1c948218ad8f176d44e015530dcb",
  "target": "unsigned-phase1-candidate",
  "platforms": ["macos-arm64", "linux-x64", "linux-arm64", "windows-x86_64-pc-windows-gnu"],
  "unsupported": ["windows-msvc", "automatic-signing", "automatic-notarization"],
  "capacity": {"total": 20, "breadth": 14, "integration": 4, "verifier": 2, "cargoSemaphore": 1},
  "externalGates": [
    {"id": "G-AUTHORITY", "state": "blocked", "owner": "repository-and-controller-authority", "evidence": "576cda6", "condition": "reconcile 51 repository errors and 58 baseline convergence findings without weakening policy"},
    {"id": "G-LINUX-RUNNER", "state": "blocked", "owner": "release-infrastructure", "condition": "real Linux runners prove x64/arm64 package execution and actual Landlock attachment"},
    {"id": "G-WINDOWS-RUNNER", "state": "blocked", "owner": "release-infrastructure", "condition": "real x86_64-pc-windows-gnu runner proves DLL loading, ConPTY, installer transaction and APP-012"},
    {"id": "G-KEYRING-SEAM", "state": "blocked", "owner": "contract-authority", "evidence": "4814357", "condition": "implementation-independent behavioral seam exists before RED; if authorized pin keyring exactly =3.6.3"},
    {"id": "G-APPROVAL-PREWIRE", "state": "blocked", "owner": "contract-and-integration-authority", "evidence": "61b0959", "condition": "approve an implementation-independent compiling black-box seam for principal/origin, digest, durable claim and resume; never land compile-only types or real implementation before RED"},
    {"id": "G-SIGNING", "state": "external-optional", "owner": "release-owner", "condition": "credentials and platform identities; not required for the honestly labeled unsigned candidate"}
  ],
  "waves": [
    {"wave": 1, "goal": "authority review, compiling behavioral REDs, frozen hashes, and independent re-verification of existing candidates", "exit": "G-AUTHORITY is approved for integration; every runnable RED compiles and fails for cause; V1-FREEZE-RED records immutable hashes; candidate branches are independently classified"},
    {"wave": 2, "goal": "minimal real implementations wired through existing callers under one Cargo-heavy command at a time", "exit": "focused frozen suites are GREEN without test edits; no stub, detached process, unbounded queue, direct secret access, or ambiguous side-effect replay remains in a candidate"},
    {"wave": 3, "goal": "serialized shared-file integration, real platform package execution, exact-revision installed APP-012 verification, and unsigned receipt", "exit": "origin/main exact revision passes local/macOS plus required Linux and Windows-GNU receipts; unresolved keyring/signing gaps remain explicit; parent stays open if any mandatory journey is absent"}
  ],
  "lanes": [
    {
      "laneId": "B01-APPROVAL",
      "role": "breadth",
      "stages": [
        {"id": "APR-EXPIRY-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/security/tests/app012_approval_expiry_red.rs", "deps": [], "cargoWeight": 1, "contract": "equality expiry denies through existing app_policy public seam"},
        {"id": "APR-RETENTION-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/storage/tests/app012_approval_retention_red.rs", "deps": ["APR-EXPIRY-RED"], "cargoWeight": 1, "contract": "state 3 is retained and counted through existing RetentionV2 seam"},
        {"id": "APR-DISPATCH-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/tools/tests/app012_concrete_dispatch_red.rs", "deps": ["APR-RETENTION-RED"], "cargoWeight": 1, "contract": "non-read tools cannot fall through unbrokered and denial has zero side effect"},
        {"id": "APR-SEC-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/security/src/app_policy.rs", "deps": ["APR-EXPIRY-RED", "V1-FREEZE-RED"], "cargoWeight": 1},
        {"id": "APR-RETENTION-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/storage/src/retention_v2.rs", "deps": ["APR-RETENTION-RED", "V1-FREEZE-RED"], "cargoWeight": 1},
        {"id": "APR-TOOLS-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/tools/src/executor.rs", "deps": ["APR-DISPATCH-RED", "V1-FREEZE-RED"], "cargoWeight": 1},
        {"id": "APR-ROUTE-RED", "wave": 2, "type": "test-author", "ownedPath": "crates/server/tests/app012_approval_resume_red.rs", "deps": ["APR-SEC-IMPL", "APR-RETENTION-IMPL", "APR-TOOLS-IMPL"], "cargoWeight": 1, "externalGate": "G-APPROVAL-PREWIRE", "contract": "authority-approved compiling black-box test of the real router/turn boundary; no invented imports, fake coordinator, or compile failure"},
        {"id": "APR-DIGEST-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/security/src/approval_digest.rs", "deps": ["APR-ROUTE-RED", "V1-APPROVAL-API-FREEZE"], "cargoWeight": 1},
        {"id": "APR-AUTH-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/server/src/approval_auth.rs", "deps": ["APR-DIGEST-IMPL", "V1-APPROVAL-API-FREEZE"], "cargoWeight": 1},
        {"id": "APR-STORAGE-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/storage/src/approvals_v2.rs", "deps": ["APR-AUTH-IMPL", "I1-APPROVAL-SCHEMA", "V1-APPROVAL-API-FREEZE"], "cargoWeight": 1},
        {"id": "APR-SERVER-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/server/src/approval_coordinator.rs", "deps": ["APR-STORAGE-IMPL", "V1-APPROVAL-API-FREEZE"], "cargoWeight": 1}
      ]
    },
    {
      "laneId": "B02-OWNERSHIP",
      "role": "breadth",
      "stages": [
        {"id": "OWN-GENERATION-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/storage/tests/app012_generation_enforcement_red.rs", "deps": [], "cargoWeight": 1},
        {"id": "OWN-LOCK-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/storage/tests/app012_workspace_owner_lock_red.rs", "deps": ["OWN-GENERATION-RED"], "cargoWeight": 1},
        {"id": "OWN-GENERATION-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/storage/src/execution_v2.rs", "deps": ["OWN-GENERATION-RED", "V1-FREEZE-RED"], "cargoWeight": 1},
        {"id": "OWN-LOCK-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/storage/src/workspace_lock.rs", "deps": ["OWN-LOCK-RED", "V1-FREEZE-RED"], "cargoWeight": 1}
      ]
    },
    {
      "laneId": "B03-FILESYSTEM",
      "role": "breadth",
      "stages": [
        {"id": "PATH-HARDLINK-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/security/tests/app012_protected_hardlink_red.rs", "deps": [], "cargoWeight": 1},
        {"id": "PATH-OPEN-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/security/tests/app012_authorize_open_red.rs", "deps": ["PATH-HARDLINK-RED"], "cargoWeight": 1},
        {"id": "PATH-HARDLINK-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/security/src/lib.rs", "deps": ["PATH-HARDLINK-RED", "I2-PROTECTED-BASE", "V1-FREEZE-RED"], "cargoWeight": 1},
        {"id": "PATH-OPEN-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/security/src/lib.rs", "deps": ["PATH-OPEN-RED", "PATH-HARDLINK-IMPL"], "cargoWeight": 1}
      ]
    },
    {
      "laneId": "B04-MACOS-APP",
      "role": "breadth",
      "stages": [
        {"id": "MAC-INSTALLED-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/cli/tests/phase1_macos_installed.rs", "deps": [], "cargoWeight": 1, "contract": "new compiling installed test; never edits disputed frozen installed_default_entrypoint.rs"},
        {"id": "MAC-ENTRY-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/cli/src/main.rs", "deps": ["MAC-INSTALLED-RED", "V1-FREEZE-RED"], "cargoWeight": 1},
        {"id": "MAC-TUI-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/cli/src/tui_entry.rs", "deps": ["MAC-ENTRY-IMPL"], "cargoWeight": 1}
      ]
    },
    {
      "laneId": "B05-PACKAGE-PRODUCER",
      "role": "breadth",
      "stages": [
        {"id": "PKG-RUNTIME-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/cli/tests/build_revision_receipt_red.rs", "deps": [], "cargoWeight": 1},
        {"id": "PKG-PRODUCER-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/cli/tests/package_producer_red.rs", "deps": ["PKG-RUNTIME-RED"], "cargoWeight": 1},
        {"id": "PKG-RUNTIME-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/cli/build.rs", "deps": ["PKG-RUNTIME-RED", "V1-FREEZE-RED"], "cargoWeight": 1},
        {"id": "PKG-PRODUCER-IMPL", "wave": 2, "type": "implementation", "ownedPath": "scripts/package-oc2.py", "deps": ["PKG-PRODUCER-RED", "PKG-RUNTIME-IMPL", "SBOM-IMPL"], "cargoWeight": 0}
      ]
    },
    {
      "laneId": "B06-POSIX-CONSUMER",
      "role": "breadth",
      "stages": [
        {"id": "POSIX-UPGRADE-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/cli/tests/packaged_upgrade_rollback_red.rs", "deps": [], "cargoWeight": 1},
        {"id": "POSIX-UPGRADE-IMPL", "wave": 2, "type": "implementation", "ownedPath": "scripts/install-oc2.sh", "deps": ["POSIX-UPGRADE-RED", "V1-FREEZE-RED", "V1-POSIX-CANDIDATE"], "cargoWeight": 0, "candidateBase": "2f87242d6371cc5059facb75ff6756b0ece6ed68"}
      ]
    },
    {
      "laneId": "B07-WINDOWS-GNU",
      "role": "breadth",
      "stages": [
        {"id": "WGNU-NATIVE-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/opentui-bridge/tests/windows_gnu_build.rs", "deps": [], "cargoWeight": 1},
        {"id": "WGNU-INSTALL-RED", "wave": 1, "type": "test-author", "ownedPath": "tests/release/install_opencode2.ps1", "deps": ["WGNU-NATIVE-RED"], "cargoWeight": 0},
        {"id": "WGNU-CONPTY-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/cli/tests/windows_conpty.rs", "deps": ["WGNU-INSTALL-RED"], "cargoWeight": 1},
        {"id": "WGNU-NATIVE-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/opentui-bridge/build.rs", "deps": ["WGNU-NATIVE-RED", "V2-WGNU-FREEZE-RED"], "cargoWeight": 1},
        {"id": "WGNU-INSTALL-IMPL", "wave": 2, "type": "implementation", "ownedPath": "scripts/install-oc2.ps1", "deps": ["WGNU-INSTALL-RED", "WGNU-NATIVE-IMPL", "V2-WGNU-FREEZE-RED"], "cargoWeight": 0},
        {"id": "WGNU-CONPTY-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/cli/src/windows_conpty.rs", "deps": ["WGNU-CONPTY-RED", "V2-WGNU-FREEZE-RED"], "cargoWeight": 1}
      ]
    },
    {
      "laneId": "B08-SBOM-RELEASE",
      "role": "breadth",
      "stages": [
        {"id": "NATIVE-MATRIX-RED", "wave": 1, "type": "test-author", "ownedPath": "tests/bootstrap/test_native_artifact_matrix.py", "deps": [], "cargoWeight": 0},
        {"id": "SBOM-RED", "wave": 1, "type": "test-author", "ownedPath": "tests/bootstrap/test_release_manifest_contract.py", "deps": ["NATIVE-MATRIX-RED"], "cargoWeight": 0},
        {"id": "NATIVE-BUILD-IMPL", "wave": 2, "type": "implementation", "ownedPath": "scripts/build-native-artifacts.sh", "deps": ["NATIVE-MATRIX-RED", "V1-FREEZE-RED"], "cargoWeight": 0},
        {"id": "SBOM-IMPL", "wave": 2, "type": "implementation", "ownedPath": "scripts/write-release-manifest.py", "deps": ["SBOM-RED", "V1-FREEZE-RED"], "cargoWeight": 0}
      ]
    },
    {
      "laneId": "B09-LINUX-LANDLOCK",
      "role": "breadth",
      "stages": [
        {"id": "LANDLOCK-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/security/tests/phase1_landlock_enforcement.rs", "deps": [], "cargoWeight": 1, "runner": "linux"},
        {"id": "LANDLOCK-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/security-landlock/src/lib.rs", "deps": ["LANDLOCK-RED", "V2-LINUX-FREEZE-RED"], "cargoWeight": 1, "runner": "linux"}
      ]
    },
    {
      "laneId": "B10-AUTH-WEB",
      "role": "breadth",
      "stages": [
        {"id": "AUTHWEB-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/server/tests/phase1_authenticated_web.rs", "deps": [], "cargoWeight": 1},
        {"id": "AUTHWEB-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/server/src/daemon.rs", "deps": ["AUTHWEB-RED", "V1-FREEZE-RED"], "cargoWeight": 1}
      ]
    },
    {
      "laneId": "B11-LIVE-TURN",
      "role": "breadth",
      "stages": [
        {"id": "TURN-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/server/tests/phase1_live_turn.rs", "deps": [], "cargoWeight": 1},
        {"id": "TURN-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/server/src/turn_service.rs", "deps": ["TURN-RED", "APR-SERVER-IMPL", "V1-FREEZE-RED"], "cargoWeight": 1}
      ]
    },
    {
      "laneId": "B12-DURABLE-CLIENTS",
      "role": "breadth",
      "stages": [
        {"id": "SETUP-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/providers/tests/phase1_account_setup.rs", "deps": [], "cargoWeight": 1},
        {"id": "MULTICLIENT-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/server/tests/phase1_multiclient_restart.rs", "deps": ["SETUP-RED"], "cargoWeight": 1},
        {"id": "SETUP-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/providers/src/account_setup.rs", "deps": ["SETUP-RED", "V1-FREEZE-RED"], "cargoWeight": 1, "contract": "in-app setup uses brokered credential input; durable OS-secret persistence remains blocked by G-KEYRING-SEAM"},
        {"id": "MULTICLIENT-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/server/src/app_runtime.rs", "deps": ["MULTICLIENT-RED", "OWN-LOCK-IMPL", "V1-RECOVERY-CANDIDATE", "V1-FREEZE-RED"], "cargoWeight": 1},
        {"id": "KEYRING-DISCOVERY", "wave": 2, "type": "blocked-discovery", "ownedPath": "worklog/PHASE1-KEYRING-SEAM-PROPOSAL.md", "deps": [], "cargoWeight": 0, "externalGate": "G-KEYRING-SEAM"}
      ]
    },
    {
      "laneId": "B13-RESOURCES",
      "role": "breadth",
      "stages": [
        {"id": "RESOURCE-RED", "wave": 1, "type": "test-author", "ownedPath": "crates/foundation/tests/resource_ledger_linux.rs", "deps": [], "cargoWeight": 1, "runner": "linux"},
        {"id": "RESOURCE-IMPL", "wave": 2, "type": "implementation", "ownedPath": "crates/foundation/src/resource_ledger_linux.rs", "deps": ["RESOURCE-RED", "V2-LINUX-FREEZE-RED"], "cargoWeight": 1, "runner": "linux"}
      ]
    },
    {
      "laneId": "B14-INSTALLED-E2E",
      "role": "breadth-test-author",
      "stages": [
        {"id": "E2E-MAC-RED", "wave": 1, "type": "independent-test-author", "ownedPath": "tests/e2e/local_application_macos.rs", "deps": ["MAC-INSTALLED-RED"], "cargoWeight": 1, "runner": "macos-arm64"},
        {"id": "E2E-LINUX-RED", "wave": 1, "type": "independent-test-author", "ownedPath": "tests/e2e/local_application_linux.rs", "deps": ["E2E-MAC-RED"], "cargoWeight": 1, "runner": "linux-x64"},
        {"id": "E2E-WGNU-RED", "wave": 1, "type": "independent-test-author", "ownedPath": "tests/e2e/local_application_windows_gnu.rs", "deps": ["E2E-LINUX-RED"], "cargoWeight": 1, "runner": "windows-x86_64-pc-windows-gnu"}
      ]
    },
    {
      "laneId": "I01-SHARED-CONTRACTS",
      "role": "integration-spine",
      "stages": [
        {"id": "I1-AUTHORITY-RECONCILE", "wave": 1, "type": "authority", "ownedPath": null, "deps": [], "cargoWeight": 0, "externalGate": "G-AUTHORITY"},
        {"id": "I1-APPROVAL-SCHEMA", "wave": 2, "type": "serialized-integration", "ownedPath": "crates/storage/schema/v2/workspace.sql", "deps": ["APR-ROUTE-RED", "V1-APPROVAL-API-FREEZE", "I1-AUTHORITY-RECONCILE"], "cargoWeight": 1},
        {"id": "I1-LANDLOCK-WORKSPACE", "wave": 2, "type": "serialized-integration", "ownedPath": "Cargo.toml", "deps": ["LANDLOCK-IMPL", "I1-AUTHORITY-RECONCILE"], "cargoWeight": 1},
        {"id": "I1-LANDLOCK-DEPENDENCY", "wave": 2, "type": "serialized-integration", "ownedPath": "crates/security/Cargo.toml", "deps": ["I1-LANDLOCK-WORKSPACE"], "cargoWeight": 1}
      ]
    },
    {
      "laneId": "I02-CALLER-WIRING",
      "role": "integration-spine",
      "stages": [
        {"id": "I2-PROTECTED-BASE", "wave": 2, "type": "serialized-integration", "ownedPath": "crates/security/src/lib.rs", "deps": ["V1-SCOPED-CANDIDATES", "I1-AUTHORITY-RECONCILE"], "cargoWeight": 1, "candidateRevision": "7463dbe"},
        {"id": "I2-SERVER-WIRE", "wave": 2, "type": "serialized-integration", "ownedPath": "crates/server/src/lib.rs", "deps": ["APR-SERVER-IMPL", "TURN-IMPL", "AUTHWEB-IMPL", "MULTICLIENT-IMPL", "I1-AUTHORITY-RECONCILE"], "cargoWeight": 1},
        {"id": "I2-SECURITY-WIRE", "wave": 2, "type": "serialized-integration", "ownedPath": "crates/security/src/os_backend.rs", "deps": ["LANDLOCK-IMPL", "I1-LANDLOCK-DEPENDENCY"], "cargoWeight": 1},
        {"id": "I2-FOUNDATION-WIRE", "wave": 2, "type": "serialized-integration", "ownedPath": "crates/foundation/src/lib.rs", "deps": ["RESOURCE-IMPL"], "cargoWeight": 1},
        {"id": "I2-CLI-RECEIPT-WIRE", "wave": 2, "type": "serialized-integration", "ownedPath": "crates/cli/src/main.rs", "deps": ["PKG-RUNTIME-IMPL", "MAC-ENTRY-IMPL", "I1-AUTHORITY-RECONCILE"], "cargoWeight": 1}
      ]
    },
    {
      "laneId": "I03-PACKAGE-BINDING",
      "role": "integration-spine",
      "stages": [
        {"id": "I3-ARTIFACT-MANIFEST", "wave": 2, "type": "serialized-integration", "ownedPath": "crates/opentui-bridge/native/artifacts.json", "deps": ["PKG-PRODUCER-IMPL", "NATIVE-BUILD-IMPL", "WGNU-NATIVE-IMPL", "I1-AUTHORITY-RECONCILE"], "cargoWeight": 0},
        {"id": "I3-RELEASE-WORKFLOW", "wave": 3, "type": "serialized-integration", "ownedPath": ".github/workflows/release.yml", "deps": ["I3-ARTIFACT-MANIFEST", "POSIX-UPGRADE-IMPL", "WGNU-INSTALL-IMPL", "I1-AUTHORITY-RECONCILE"], "cargoWeight": 0},
        {"id": "I3-UNSIGNED-RECEIPT", "wave": 3, "type": "serialized-integration", "ownedPath": "release/phase1-unsigned-receipt.json", "deps": ["I3-RELEASE-WORKFLOW", "V1-MACOS-INTEGRATED", "V2-LINUX-INTEGRATED", "V2-WGNU-INTEGRATED"], "cargoWeight": 0}
      ]
    },
    {
      "laneId": "I04-MAIN-INTEGRATION",
      "role": "integration-spine",
      "stages": [
        {"id": "I4-WAVE2-MERGE", "wave": 3, "type": "serialized-main-integration", "ownedPath": null, "deps": ["I2-SERVER-WIRE", "I2-SECURITY-WIRE", "I2-FOUNDATION-WIRE", "I2-CLI-RECEIPT-WIRE", "I3-ARTIFACT-MANIFEST", "OWN-GENERATION-IMPL", "OWN-LOCK-IMPL", "PATH-OPEN-IMPL", "SETUP-IMPL", "MAC-TUI-IMPL", "WGNU-CONPTY-IMPL", "I1-AUTHORITY-RECONCILE"], "cargoWeight": 1},
        {"id": "I4-RECOVERY-MERGE", "wave": 3, "type": "serialized-main-integration", "ownedPath": "crates/storage/src/facade.rs", "deps": ["I4-WAVE2-MERGE", "V1-RECOVERY-CANDIDATE"], "cargoWeight": 1, "candidateRevision": "3ad58b0554aecb79080064a7fa70b0071b1ba2a1"},
        {"id": "I4-PROTECTED-FILEOPS-MERGE", "wave": 3, "type": "serialized-main-integration", "ownedPath": "crates/tools/src/file_ops.rs", "deps": ["I4-RECOVERY-MERGE", "V1-SCOPED-CANDIDATES"], "cargoWeight": 1, "candidateRevision": "c269715"},
        {"id": "I4-BATCH-MERGE", "wave": 3, "type": "serialized-main-integration", "ownedPath": "crates/tools/src/registry_dispatch.rs", "deps": ["I4-PROTECTED-FILEOPS-MERGE", "V1-SCOPED-CANDIDATES"], "cargoWeight": 1, "candidateRevision": "55acf29"},
        {"id": "I4-POSIX-MERGE", "wave": 3, "type": "serialized-main-integration", "ownedPath": "scripts/install-oc2.sh", "deps": ["I4-BATCH-MERGE", "POSIX-UPGRADE-IMPL", "V1-POSIX-CANDIDATE"], "cargoWeight": 1},
        {"id": "I4-EXACT-REVISION", "wave": 3, "type": "serialized-main-integration", "ownedPath": null, "deps": ["I4-POSIX-MERGE", "I3-RELEASE-WORKFLOW"], "cargoWeight": 1, "contract": "push without force, fetch origin/main, rerun frozen suites on identical 40-hex revision"}
      ]
    },
    {
      "laneId": "V01-LOCAL-VERIFIER",
      "role": "independent-verifier",
      "stages": [
        {"id": "V1-RECOVERY-CANDIDATE", "wave": 1, "type": "independent-verifier", "ownedPath": null, "deps": [], "cargoWeight": 1, "candidateRevision": "3ad58b0554aecb79080064a7fa70b0071b1ba2a1", "tests": ["app012_recovery_child_bound_red 2/2", "app012_restart_resume_red 18/18", "storage --tests"]},
        {"id": "V1-POSIX-CANDIDATE", "wave": 1, "type": "independent-verifier", "ownedPath": null, "deps": ["V1-RECOVERY-CANDIDATE"], "cargoWeight": 1, "candidateRevision": "2f87242d6371cc5059facb75ff6756b0ece6ed68", "tests": ["packaged_revision_binding 5/5", "disposable rollback and no-survivor probes"]},
        {"id": "V1-SCOPED-CANDIDATES", "wave": 1, "type": "independent-verifier", "ownedPath": null, "deps": ["V1-POSIX-CANDIDATE"], "cargoWeight": 1, "candidateRevisions": ["c269715", "55acf29", "0fb0707"], "tests": ["protected path frozen 7/7 plus security and journey", "batch immediate frozen 6/6", "Unix cancellation 9/9 plus broker and cwd; classify as existing non-RED behavior"]},
        {"id": "V1-FREEZE-RED", "wave": 1, "type": "freeze-authority", "ownedPath": null, "deps": ["APR-DISPATCH-RED", "OWN-LOCK-RED", "PATH-OPEN-RED", "MAC-INSTALLED-RED", "PKG-PRODUCER-RED", "POSIX-UPGRADE-RED", "SBOM-RED", "NATIVE-MATRIX-RED", "AUTHWEB-RED", "TURN-RED", "SETUP-RED", "MULTICLIENT-RED", "E2E-MAC-RED"], "cargoWeight": 0},
        {"id": "V1-APPROVAL-API-FREEZE", "wave": 2, "type": "freeze-authority", "ownedPath": null, "deps": ["APR-ROUTE-RED"], "cargoWeight": 0, "externalGate": "G-APPROVAL-PREWIRE"},
        {"id": "V1-MACOS-INTEGRATED", "wave": 3, "type": "independent-verifier", "ownedPath": null, "deps": ["I4-EXACT-REVISION", "E2E-MAC-RED"], "cargoWeight": 1, "runner": "macos-arm64"}
      ]
    },
    {
      "laneId": "V02-PLATFORM-VERIFIER",
      "role": "independent-verifier",
      "stages": [
        {"id": "V2-STATIC-INTEGRITY", "wave": 1, "type": "independent-verifier", "ownedPath": null, "deps": ["V1-FREEZE-RED"], "cargoWeight": 0, "contract": "audit dependency closure, hashes, single-file ownership, unsigned claims and frozen-test immutability"},
        {"id": "V2-LINUX-FREEZE-RED", "wave": 1, "type": "freeze-authority", "ownedPath": null, "deps": ["LANDLOCK-RED", "RESOURCE-RED", "E2E-LINUX-RED"], "cargoWeight": 1, "runner": "linux-x64", "externalGate": "G-LINUX-RUNNER"},
        {"id": "V2-WGNU-FREEZE-RED", "wave": 1, "type": "freeze-authority", "ownedPath": null, "deps": ["WGNU-CONPTY-RED", "E2E-WGNU-RED"], "cargoWeight": 1, "runner": "windows-x86_64-pc-windows-gnu", "externalGate": "G-WINDOWS-RUNNER"},
        {"id": "V2-LINUX-INTEGRATED", "wave": 3, "type": "independent-verifier", "ownedPath": null, "deps": ["I4-EXACT-REVISION", "E2E-LINUX-RED", "LANDLOCK-IMPL", "RESOURCE-IMPL"], "cargoWeight": 1, "runner": "linux-x64+linux-arm64", "externalGate": "G-LINUX-RUNNER"},
        {"id": "V2-WGNU-INTEGRATED", "wave": 3, "type": "independent-verifier", "ownedPath": null, "deps": ["I4-EXACT-REVISION", "E2E-WGNU-RED", "WGNU-INSTALL-IMPL", "WGNU-CONPTY-IMPL"], "cargoWeight": 1, "runner": "windows-x86_64-pc-windows-gnu", "externalGate": "G-WINDOWS-RUNNER"}
      ]
    }
  ],
  "parentCloseRule": "APP-012 and Phase 1 remain open until the installed exact-revision journey passes on every mandatory platform and no note admits a repair child, unwired caller, missing runner, partial proof, or authority blocker. G-SIGNING does not block an explicitly unsigned candidate; G-KEYRING-SEAM blocks credential-durability claims."
}
```

## Critical path and scheduling interpretation

1. `I1-AUTHORITY-RECONCILE` is the integration gate, not permission for a
   worker to edit controller state. While its owner reviews reconciliation,
   Wave 1 test authors may produce compiling REDs on branch worktrees and V01
   may independently verify `3ad58b0` and `2f87242`; nothing merges to main.
2. `V1-FREEZE-RED` is the Wave 1 fan-in. Ordinary Wave 2 implementation
   depends on the relevant RED plus this independent freeze. Approval API work
   remains blocked by `G-APPROVAL-PREWIRE`; after an authority-approved
   implementation-independent `APR-ROUTE-RED`, `V1-APPROVAL-API-FREEZE` is its
   separate freeze before digest/auth/schema/coordinator implementation.
3. Wave 2 uses at most 14 breadth slots, but the Cargo semaphore serializes
   every Rust build/test. Text-only producer, installer, SBOM, and workflow work
   may proceed concurrently when paths do not collide.
4. `I4-WAVE2-MERGE -> I4-RECOVERY-MERGE -> I4-POSIX-MERGE ->
   I4-EXACT-REVISION` is the integration spine. No feature branch is treated as
   release truth before that sequence and its exact-revision reruns.
5. Wave 3 fans out to macOS, Linux, and Windows-GNU verification. Linux and
   Windows remain blocked until real runners return receipts. Only after all
   mandatory receipts may `I3-UNSIGNED-RECEIPT` describe a candidate, explicitly
   as unsigned.

No calendar ETA is honest while `G-AUTHORITY`, `G-LINUX-RUNNER`, and
`G-WINDOWS-RUNNER` are blocked. Once gates are supplied, completion is measured
by stage receipts, not elapsed-time promises.

## Validation receipt and remaining gaps

- Proposal parser: one JSON block, 20 lanes, 83 stages, 162 dependency edges.
- Capacity: 14 breadth, 4 integration-spine, 2 independent verifier lanes.
- Graph: all stage IDs unique; no unknown dependency; no dependency on a later
  wave; acyclic; no unordered cross-lane write to the same path in one wave.
- `git diff --check`: PASS.
- `python3 tools/validate_repository.py`: FAIL at the inherited 51-error
  backlog-exhaustion reconciliation gate.
- `python3 tools/convergence_gate.py`: BLOCKED with the inherited baseline total
  of 58 findings.
- No Cargo, product test, runner, browser, database, network, credential, or
  user-data operation was used to validate this planning artifact.

The proposal does not resolve authority reconciliation, keyring durability,
actual Landlock attachment, Linux arm64 native bytes, Windows-GNU DLL/ConPTY
execution, or signing identities. It makes each an explicit gate or stage and
therefore cannot be used to claim APP-012 or Phase 1 complete today.
