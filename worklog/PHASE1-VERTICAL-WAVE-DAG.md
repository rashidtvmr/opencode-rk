# PHASE1-VERTICAL-WAVE-DAG

## Status and boundary

**PROPOSED, NOT ACCEPTED.** This artifact proposes a machine-actionable three-wave
local vertical DAG. It does not alter `ralph.json`, `ralph.completion.json`,
`PLAN.md`, tests, Cargo files, controller state, verifier configuration, or
release policy. Only this worklog and this task's claim row are owned here.

Task: `PHASE1-VERTICAL-WAVE-DAG`
Session: `ses_f29646e0cffeonm9du18zQP7j3`
Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/plan-phase1-vertical-wave-dag`
Branch: `plan/PHASE1-VERTICAL-WAVE-DAG`

Observed before claim: `HEAD=8a91a7b49a5a1c948218ad8f176d44e015530dcb`,
`origin/main=8a91a7b49a5a1c948218ad8f176d44e015530dcb`, Darwin arm64, stable
toolchain. The only intentional working-tree change is this task's claim row;
the worklog is the only new artifact. No product implementation or test was
run or changed by this task.

## Source evidence and current truth

| Evidence | Current behavior or rule |
|---|---|
| `PLAN.md:113-120` | Tests, verifier configuration, limits, pins and acceptance have separate trusted ownership. Worker evidence is not acceptance. |
| `PLAN.md:149-155` | Shared schemas, migrations, Cargo manifests, lockfiles and central route registries are serialized integration ownership. |
| `PLAN.md:162-180` | Required lifecycle is inspect, define, compiling RED, freeze hash, minimum implementation, GREEN, independent regression, integrated rerun. A compile failure is invalid RED. |
| `AGENTS.md:39-43` | Twenty-worker waves reserve at least four integration-spine lanes and two independent test/verifier lanes; breadth is at most fourteen. |
| `AGENTS.md:164-170` | One owned file per lane; shared `lib.rs` and similar files are prewired by the integrator; `lane_gate.py` is independent of worker claims. |
| `AGENTS.md:174-207` | Claim before files, legal status transitions, commit and push every completed lane, verify exact integrated revision on `origin/main`. |
| `docs/TDD.md:32-42,43-75` | Independent author must produce executable RED before implementation; frozen tests are immutable; verifier decides, not implementer. |
| `docs/CONVERGENCE.md:10-24` | Hard boundary is installed `opencode2`, authenticated daemon, native UI, real provider/tool/broker turn, persistence, restart/resume and second client. |
| `docs/CONVERGENCE.md:30-43,70-105` | No breadth majority before local boundary; four spine plus two verifier roles; ledger completion is not acceptance. |
| `config/completion-controller.json:4-31` | Config target is 20, max 20, one heavy validation, 8 GiB budget, 2 GiB reserve, two Cargo jobs, two test threads, four spine, two verifier, max fourteen breadth. |
| `config/controller.settings.json:4-29` | Effective current controller cap is six; its worker/verifier pools do not include `9router-xk-gpt56-luna`. Proposed 20 is not currently authorized. |
| `ralph.completion.json:7,28-35` | Full scope is the union of legacy, requirements, completion includes and discovered children. External credentials, signing, devices and isolation are blockers, not passes. |
| `tasks/completion/delivery.json:4-11` | COORD-001..008 already define native delegation, rolling scheduling, one-file ownership, frozen evidence, serialized integration, leases and budgets. This DAG is a phase proposal, not a replacement. |
| `sources/release-assurance-gap.json:1-17,140-174` | REL-001..003 have no native product owner or bound task card. Release assurance remains unresolved and cannot be made green by this worklog. |
| `REVIEW_ITERATION_1.md:26-40` | Current review reports unauthenticated serve path, no-subcommand bypass, fake native path and client auth gaps. These are spine selection evidence, not acceptance. |
| `REVIEW_ITERATION_1.md:104-120` | Current review reports direct tool execution bypassing the security broker, single-provider gating, and unsafe 15-way/heavy validation pressure. These are breadth and verifier priorities. |
| `576cda6` (`worklog/PHASE1-INTEGRATION-GUARD-UNBLOCK.md` in that commit) | Prior guard proposal records baseline `validate_repository` failure with 51 backlog errors and `convergence_gate` failure with 58 findings. It is not authority to edit protected state. |

Read-only commands and truthful results:

```text
python3 tools/convergence_gate.py
CONVERGENCE BLOCKED; total=58

python3 tools/validate_repository.py
render_ruleset_import: OK
validate_protection_policy: OK
validate_backlog_exhaustion: 51 error(s)
validate_repository: FAIL backlog exhaustion exit=1
```

The 51 and 58 findings remain blockers. They are not hidden by this proposal.
The current claims ledger has 162 rows (`123 completed`, `21 in-progress`,
`18 blocked`), including off-plan rows; convergence explicitly rejects such
rows. No row in this artifact may be treated as a release acceptance record.

Current pushed references inspected:

```text
origin/main                                      8a91a7b49a5a1c948218ad8f176d44e015530dcb
origin/plan/PHASE1-INTEGRATION-GUARD-UNBLOCK     576cda69c01ac9375d95dc37936ee564e53dbd59
origin/lane/PHASE1-product-spine-20260923       5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b
origin/lane/PHASE1-convergence                   e57d1d37b6050f217609c2c994bb9f9948841388
origin/lane/PHASE1-convergence-review            00e2c26d2c4e1a95480afbc051ba08e7b2a231e6
origin/plan/release-evidence                     c0c5d9b48020413e57fdcabee708c83459b39268
```

`origin/plan/PHASE1-VERTICAL-WAVE-DAG` did not exist before this lane. It is
created only by the required push below. A release fixture such as
`fixtures/release-accounting/complete/release-ledger.json` is a validator input,
not platform or product evidence.

## Observable vertical contract

The proposed parent is `PH1-LOCAL-APP`. It remains open until one exact
integrated revision demonstrates, in a disposable HOME and workspace:

1. Installed `opencode2` with no subcommand discovers or starts exactly one
   authenticated per-user daemon.
2. Missing credentials enters setup; no manual `serve`, browser or database
   setup is needed.
3. Native OpenTUI has a real caller and renders the app, not line fallback.
4. A fixture provider streams a turn, requests a tool, and the security broker
   authorizes or denies it. Denial leaves no file/process side effect.
5. Transcript, tool result, cursor/event state and session history persist.
6. Exit, restart and resume recover the same durable session; uncertain work is
   not fabricated as completed.
7. A second client sees the same session and does not duplicate work.
8. Cancellation reclaims owned tasks, child processes, permits and bounded
   buffers. Output and queues have byte/count/retention limits.

Failure states are explicit: unauthorized, stale descriptor, version mismatch,
provider failure, partial stream, denied operation, interrupted turn, uncertain
restart, unavailable platform, unavailable credentials, and resource admission
blocked. Ownership is daemon/session/turn scoped; no detached task or automatic
replay of ambiguous effects is allowed.

## Replacement for the impossible one-agent acceptance story

The old shape, "one subagent owns a parent test through implementation to
acceptance," is invalid. It lets an implementer author or mutate its own oracle,
confuses source presence with wiring, and cannot prove independent verification.

This DAG uses a single-owner vertical **chain**, not a single-owner acceptance:

```text
test-author RED (one test file)
  -> freeze command and test hashes (test becomes read-only)
  -> implementer GREEN (one product file)
  -> independent verifier GREEN (one independent test file, read-only frozen test)
  -> serialized integrator rerun (one product file, current mainline)
  -> trusted controller acceptance decision
```

Each stage has a distinct task ID, claim row, session, receipt and status. A
stage may hand off one file only after its dependency completes. The integrator
prewires shared `lib.rs`, `Cargo.toml`, workflow, schema, migration, route and
controller changes before admitting file lanes. No child edits a frozen test,
shared contract or another lane's file.

## Machine-actionable proposal: scheduler and receipts

The following JSON is embedded here only. It is not a live plan file.

```json
{
  "schemaVersion": 1,
  "dagId": "PHASE1-VERTICAL-WAVE-DAG",
  "parent": "PH1-LOCAL-APP",
  "baseRevision": "8a91a7b49a5a1c948218ad8f176d44e015530dcb",
  "originMainRequired": true,
  "runner": {
    "adapter": "native-harness-only",
    "platformObserved": "Darwin arm64",
    "toolchain": "stable",
    "providerRoute": "9router-xk-gpt56-luna",
    "retiredRoutes": ["prior-provider-failed-before-claim-and-files"],
    "reAdmitRetiredRoute": "user-only",
    "textConcurrency": {
      "requestedLogicalWorkers": 20,
      "effectiveCurrentCap": 6,
      "admission": "min(20, controller, provider, memory, lease caps)",
      "neverFabricateOccupancy": true
    },
    "cargo": {
      "globalSemaphore": 1,
      "maxHeavyValidations": 1,
      "cargoBuildJobs": 2,
      "rustTestThreads": 1,
      "availableMemoryFloorMiB": 1024,
      "hostBudgetMiB": 8192,
      "reservedMemoryMiB": 2048
    },
    "roleReservation": {
      "workers": 20,
      "integrationSpine": 4,
      "independentVerifier": 2,
      "breadthMaximum": 14
    },
    "retry": {
      "repairableMaximum": 3,
      "authSecuritySigningBudgetFailures": "block",
      "ambiguousSideEffect": "stop-and-preserve"
    }
  },
  "statusVocabulary": ["not-started", "in-progress", "completed", "blocked"],
  "completionMeaning": "coordination-only; independent verification, serialized integration, exact origin/main rerun and controller acceptance still required",
  "integratorLocks": [
    "lib.rs", "Cargo.toml", "Cargo.lock", "workflow", "schemas", "migrations",
    "central route registries", "ralph.json", "ralph.completion.json",
    "FEATURES.md", "PLAN.md", "claims acceptance state", "verifier configuration"
  ]
}
```

Receipt fields are mandatory for every stage. A null hash is honest before the
stage runs, not a pass:

```json
{
  "taskId": "PH1-ENTRY-RED",
  "status": "not-started",
  "type": "test-author",
  "wave": 1,
  "deps": [],
  "owned_paths": ["crates/cli/tests/phase1_entrypoint.rs"],
  "scratchpad": "worklog/PH1-ENTRY.md",
  "claim_row": "tasks/completion/claims.json#/claims/PH1-ENTRY-RED",
  "cargo_weight": 1,
  "command": "CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 timeout 180 cargo test -p opencode-rk-cli --test phase1_entrypoint -- --test-threads=1",
  "command_sha256": null,
  "frozen_test_sha256": null,
  "manifest_sha256": null,
  "candidate_revision": null,
  "integrated_revision": null,
  "origin_main_proof": null,
  "red_expected": "compile succeeds; named missing behavior fails; no ignored or zero-discovery tests",
  "acceptance": "none"
}
```

`owned_paths` has exactly one test or product file. The worklog and claim row
are coordination records, not additional product ownership. Integration tasks
may reacquire the same product path only after their implementation dependency
has released it; the global integration mutex makes this serial.

## Proposed task graph

The 18 vertical lanes are four integration-spine lanes plus fourteen breadth
lanes. Each has RED/freeze, implementation, and serialized integration stages.
The two verifier lanes have independent RED/freeze and GREEN verification
stages. Thus the graph has 58 proposed stage IDs, while its rolling worker
reservation remains 4 + 14 + 2 = 20.

### Stage rules used by every lane

```json
{
  "redFreeze": {
    "type": "test-author",
    "status": "not-started",
    "cargo_weight": 1,
    "must": ["compile", "fail-for-missing-behavior", "freeze-test-hash", "freeze-command-manifest"],
    "after": "claim",
    "handoff": "frozen test is read-only"
  },
  "implementation": {
    "type": "implementation",
    "status": "not-started",
    "cargo_weight": 1,
    "must": ["edit-one-product-file", "do-not-edit-frozen-test", "GREEN", "focused-regression"],
    "after": "redFreeze"
  },
  "independentVerification": {
    "type": "independent-verifier",
    "status": "not-started",
    "cargo_weight": 1,
    "must": ["read-only-frozen-test", "run-real-candidate", "assert-side-effect-absence", "assert-resource-reclaim", "emit-redacted-receipt"],
    "after": "implementation"
  },
  "integration": {
    "type": "serialized-integration",
    "status": "not-started",
    "cargo_weight": 1,
    "must": ["fetch-origin-main", "rebase-or-merge-without-force", "run-affected-frozen-test", "record-integrated-revision", "push-origin-main"],
    "after": ["implementation", "independentVerification"],
    "mutex": "integration-and-global-cargo"
  }
}
```

### Four integration-spine lanes

```json
[
  {
    "laneId": "PH1-ENTRY",
    "role": "integration-spine",
    "journey": "no-subcommand -> authenticated daemon -> native app",
    "red_path": "crates/cli/tests/phase1_entrypoint.rs",
    "product_path": "crates/cli/src/main.rs",
    "stages": [
      {"id":"PH1-ENTRY-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/cli/tests/phase1_entrypoint.rs"],"cargo_weight":1,"scratchpad":"worklog/PH1-ENTRY.md"},
      {"id":"PH1-ENTRY-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-ENTRY-RED"],"owned_paths":["crates/cli/src/main.rs"],"cargo_weight":1,"scratchpad":"worklog/PH1-ENTRY.md"},
      {"id":"PH1-ENTRY-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-ENTRY-IMPL","PH1-V-LOCAL-GREEN"],"owned_paths":["crates/cli/src/main.rs"],"cargo_weight":1,"scratchpad":"worklog/PH1-ENTRY.md"}
    ]
  },
  {
    "laneId": "PH1-AUTH",
    "role": "integration-spine",
    "journey": "singleton descriptor -> bearer auth -> two clients",
    "red_path": "crates/server/tests/phase1_daemon_auth.rs",
    "product_path": "crates/server/src/daemon.rs",
    "stages": [
      {"id":"PH1-AUTH-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/server/tests/phase1_daemon_auth.rs"],"cargo_weight":1,"scratchpad":"worklog/PH1-AUTH.md"},
      {"id":"PH1-AUTH-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-AUTH-RED"],"owned_paths":["crates/server/src/daemon.rs"],"cargo_weight":1,"scratchpad":"worklog/PH1-AUTH.md"},
      {"id":"PH1-AUTH-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-AUTH-IMPL","PH1-V-LOCAL-GREEN"],"owned_paths":["crates/server/src/daemon.rs"],"cargo_weight":1,"scratchpad":"worklog/PH1-AUTH.md"}
    ]
  },
  {
    "laneId": "PH1-TURN",
    "role": "integration-spine",
    "journey": "prompt -> provider stream -> broker -> tool -> durable turn",
    "red_path": "crates/server/tests/phase1_turn_broker.rs",
    "product_path": "crates/server/src/turn_service.rs",
    "stages": [
      {"id":"PH1-TURN-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/server/tests/phase1_turn_broker.rs"],"cargo_weight":1,"scratchpad":"worklog/PH1-TURN.md"},
      {"id":"PH1-TURN-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-TURN-RED"],"owned_paths":["crates/server/src/turn_service.rs"],"cargo_weight":1,"scratchpad":"worklog/PH1-TURN.md"},
      {"id":"PH1-TURN-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-TURN-IMPL","PH1-V-SEC-GREEN"],"owned_paths":["crates/server/src/turn_service.rs"],"cargo_weight":1,"scratchpad":"worklog/PH1-TURN.md"}
    ]
  },
  {
    "laneId": "PH1-TUI",
    "role": "integration-spine",
    "journey": "real native renderer caller -> input -> transcript view",
    "red_path": "crates/cli/tests/phase1_native_tui.rs",
    "product_path": "crates/cli/src/tui_entry.rs",
    "stages": [
      {"id":"PH1-TUI-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/cli/tests/phase1_native_tui.rs"],"cargo_weight":1,"scratchpad":"worklog/PH1-TUI.md"},
      {"id":"PH1-TUI-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-TUI-RED"],"owned_paths":["crates/cli/src/tui_entry.rs"],"cargo_weight":1,"scratchpad":"worklog/PH1-TUI.md"},
      {"id":"PH1-TUI-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-TUI-IMPL","PH1-V-LOCAL-GREEN"],"owned_paths":["crates/cli/src/tui_entry.rs"],"cargo_weight":1,"scratchpad":"worklog/PH1-TUI.md"}
    ]
  }
]
```

### Fourteen breadth lanes

These are safe only while four spine and two verifier reservations remain. They
do not authorize web, remote, mobile, release signing, or live-provider breadth
before the local boundary is green.

```json
[
  {"laneId":"PH1-CLIENT","role":"breadth","product_path":"crates/cli/src/daemon_client.rs","red_path":"crates/cli/tests/phase1_client_auth.rs","stages":[{"id":"PH1-CLIENT-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/cli/tests/phase1_client_auth.rs"],"cargo_weight":1},{"id":"PH1-CLIENT-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-CLIENT-RED"],"owned_paths":["crates/cli/src/daemon_client.rs"],"cargo_weight":1},{"id":"PH1-CLIENT-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-CLIENT-IMPL","PH1-V-LOCAL-GREEN"],"owned_paths":["crates/cli/src/daemon_client.rs"],"cargo_weight":1}]},
  {"laneId":"PH1-SESSION","role":"breadth","product_path":"crates/sessions/src/store.rs","red_path":"crates/sessions/tests/phase1_session_restart.rs","stages":[{"id":"PH1-SESSION-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/sessions/tests/phase1_session_restart.rs"],"cargo_weight":1},{"id":"PH1-SESSION-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-SESSION-RED"],"owned_paths":["crates/sessions/src/store.rs"],"cargo_weight":1},{"id":"PH1-SESSION-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-SESSION-IMPL","PH1-V-LOCAL-GREEN"],"owned_paths":["crates/sessions/src/store.rs"],"cargo_weight":1}]},
  {"laneId":"PH1-EVENT","role":"breadth","product_path":"crates/server/src/event_stream.rs","red_path":"crates/server/tests/phase1_event_replay.rs","stages":[{"id":"PH1-EVENT-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/server/tests/phase1_event_replay.rs"],"cargo_weight":1},{"id":"PH1-EVENT-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-EVENT-RED"],"owned_paths":["crates/server/src/event_stream.rs"],"cargo_weight":1},{"id":"PH1-EVENT-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-EVENT-IMPL","PH1-V-LOCAL-GREEN"],"owned_paths":["crates/server/src/event_stream.rs"],"cargo_weight":1}]},
  {"laneId":"PH1-PROVIDER","role":"breadth","product_path":"crates/providers/src/router.rs","red_path":"crates/providers/tests/phase1_provider_fixture.rs","stages":[{"id":"PH1-PROVIDER-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/providers/tests/phase1_provider_fixture.rs"],"cargo_weight":1},{"id":"PH1-PROVIDER-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-PROVIDER-RED"],"owned_paths":["crates/providers/src/router.rs"],"cargo_weight":1},{"id":"PH1-PROVIDER-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-PROVIDER-IMPL","PH1-V-LOCAL-GREEN"],"owned_paths":["crates/providers/src/router.rs"],"cargo_weight":1}]},
  {"laneId":"PH1-BROKER","role":"breadth","product_path":"crates/security/src/tool_authorize.rs","red_path":"crates/security/tests/phase1_broker_denial.rs","stages":[{"id":"PH1-BROKER-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/security/tests/phase1_broker_denial.rs"],"cargo_weight":1},{"id":"PH1-BROKER-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-BROKER-RED"],"owned_paths":["crates/security/src/tool_authorize.rs"],"cargo_weight":1},{"id":"PH1-BROKER-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-BROKER-IMPL","PH1-V-SEC-GREEN"],"owned_paths":["crates/security/src/tool_authorize.rs"],"cargo_weight":1}]},
  {"laneId":"PH1-SANDBOX","role":"breadth","product_path":"crates/security/src/sandbox.rs","red_path":"crates/security/tests/phase1_sandbox.rs","stages":[{"id":"PH1-SANDBOX-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/security/tests/phase1_sandbox.rs"],"cargo_weight":1},{"id":"PH1-SANDBOX-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-SANDBOX-RED"],"owned_paths":["crates/security/src/sandbox.rs"],"cargo_weight":1},{"id":"PH1-SANDBOX-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-SANDBOX-IMPL","PH1-V-SEC-GREEN"],"owned_paths":["crates/security/src/sandbox.rs"],"cargo_weight":1}]},
  {"laneId":"PH1-TOOLS","role":"breadth","product_path":"crates/tools/src/registry_dispatch.rs","red_path":"crates/tools/tests/phase1_tool_dispatch.rs","stages":[{"id":"PH1-TOOLS-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/tools/tests/phase1_tool_dispatch.rs"],"cargo_weight":1},{"id":"PH1-TOOLS-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-TOOLS-RED"],"owned_paths":["crates/tools/src/registry_dispatch.rs"],"cargo_weight":1},{"id":"PH1-TOOLS-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-TOOLS-IMPL","PH1-V-SEC-GREEN"],"owned_paths":["crates/tools/src/registry_dispatch.rs"],"cargo_weight":1}]},
  {"laneId":"PH1-AGENTS","role":"breadth","product_path":"crates/agents/src/executor.rs","red_path":"crates/agents/tests/phase1_agent_lifecycle.rs","stages":[{"id":"PH1-AGENTS-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/agents/tests/phase1_agent_lifecycle.rs"],"cargo_weight":1},{"id":"PH1-AGENTS-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-AGENTS-RED"],"owned_paths":["crates/agents/src/executor.rs"],"cargo_weight":1},{"id":"PH1-AGENTS-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-AGENTS-IMPL","PH1-V-SEC-GREEN"],"owned_paths":["crates/agents/src/executor.rs"],"cargo_weight":1}]},
  {"laneId":"PH1-PERSIST","role":"breadth","product_path":"crates/sessions/src/persist.rs","red_path":"crates/sessions/tests/phase1_history_persist.rs","stages":[{"id":"PH1-PERSIST-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/sessions/tests/phase1_history_persist.rs"],"cargo_weight":1},{"id":"PH1-PERSIST-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-PERSIST-RED"],"owned_paths":["crates/sessions/src/persist.rs"],"cargo_weight":1},{"id":"PH1-PERSIST-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-PERSIST-IMPL","PH1-V-LOCAL-GREEN"],"owned_paths":["crates/sessions/src/persist.rs"],"cargo_weight":1}]},
  {"laneId":"PH1-ONBOARD","role":"breadth","product_path":"crates/providers/src/account_setup.rs","red_path":"crates/providers/tests/phase1_setup_fixture.rs","stages":[{"id":"PH1-ONBOARD-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/providers/tests/phase1_setup_fixture.rs"],"cargo_weight":1},{"id":"PH1-ONBOARD-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-ONBOARD-RED"],"owned_paths":["crates/providers/src/account_setup.rs"],"cargo_weight":1},{"id":"PH1-ONBOARD-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-ONBOARD-IMPL","PH1-V-LOCAL-GREEN"],"owned_paths":["crates/providers/src/account_setup.rs"],"cargo_weight":1}]},
  {"laneId":"PH1-HEADLESS","role":"breadth","product_path":"crates/cli/src/chat.rs","red_path":"crates/cli/tests/phase1_client_journey.rs","stages":[{"id":"PH1-HEADLESS-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/cli/tests/phase1_client_journey.rs"],"cargo_weight":1},{"id":"PH1-HEADLESS-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-HEADLESS-RED"],"owned_paths":["crates/cli/src/chat.rs"],"cargo_weight":1},{"id":"PH1-HEADLESS-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-HEADLESS-IMPL","PH1-V-LOCAL-GREEN"],"owned_paths":["crates/cli/src/chat.rs"],"cargo_weight":1}]}
  ,{"laneId":"PH1-SHUTDOWN","role":"breadth","product_path":"crates/cli/src/shutdown.rs","red_path":"crates/cli/tests/phase1_shutdown.rs","stages":[{"id":"PH1-SHUTDOWN-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/cli/tests/phase1_shutdown.rs"],"cargo_weight":1},{"id":"PH1-SHUTDOWN-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-SHUTDOWN-RED"],"owned_paths":["crates/cli/src/shutdown.rs"],"cargo_weight":1},{"id":"PH1-SHUTDOWN-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-SHUTDOWN-IMPL","PH1-V-LOCAL-GREEN"],"owned_paths":["crates/cli/src/shutdown.rs"],"cargo_weight":1}]},
  {"laneId":"PH1-RUNTIME","role":"breadth","product_path":"crates/server/src/app_runtime.rs","red_path":"crates/server/tests/phase1_runtime_composition.rs","stages":[{"id":"PH1-RUNTIME-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/server/tests/phase1_runtime_composition.rs"],"cargo_weight":1},{"id":"PH1-RUNTIME-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-RUNTIME-RED"],"owned_paths":["crates/server/src/app_runtime.rs"],"cargo_weight":1},{"id":"PH1-RUNTIME-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-RUNTIME-IMPL","PH1-V-LOCAL","PH1-V-SEC-GREEN"],"owned_paths":["crates/server/src/app_runtime.rs"],"cargo_weight":1}]
  },
  {"laneId":"PH1-MCP","role":"breadth","product_path":"crates/tools/src/mcp_spawn.rs","red_path":"crates/tools/tests/phase1_mcp_lifecycle.rs","stages":[{"id":"PH1-MCP-RED","type":"test-author","status":"not-started","wave":1,"deps":[],"owned_paths":["crates/tools/tests/phase1_mcp_lifecycle.rs"],"cargo_weight":1},{"id":"PH1-MCP-IMPL","type":"implementation","status":"not-started","wave":2,"deps":["PH1-MCP-RED"],"owned_paths":["crates/tools/src/mcp_spawn.rs"],"cargo_weight":1},{"id":"PH1-MCP-INTEGRATE","type":"serialized-integration","status":"not-started","wave":3,"deps":["PH1-MCP-IMPL","PH1-V-SEC-GREEN"],"owned_paths":["crates/tools/src/mcp_spawn.rs"],"cargo_weight":1}]}
]
```

### Independent verifier lanes

Verifier lanes are not implementers. Their frozen files are independently
authored and then read-only. Their stage IDs are dependencies of integration
stages, never acceptance records.

```json
[
  {
    "laneId":"PH1-V-LOCAL",
    "role":"independent-verifier",
    "scope":"installed local journey, auth, UI, provider fixture, persistence, restart, second client",
    "owned_test_path":"tests/release/local_install/phase1_local.rs",
    "stages":[
      {"id":"PH1-V-LOCAL-RED","type":"independent-test-author","status":"not-started","wave":1,"deps":["PH1-ENTRY-RED","PH1-AUTH-RED","PH1-TUI-RED"],"owned_paths":["tests/release/local_install/phase1_local.rs"],"cargo_weight":1},
      {"id":"PH1-V-LOCAL-GREEN","type":"independent-verifier","status":"not-started","wave":3,"deps":["PH1-ENTRY-IMPL","PH1-AUTH-IMPL","PH1-TUI-IMPL","PH1-CLIENT-IMPL","PH1-SESSION-IMPL","PH1-EVENT-IMPL","PH1-PROVIDER-IMPL","PH1-CATALOG-IMPL","PH1-PERSIST-IMPL","PH1-ONBOARD-IMPL","PH1-HEADLESS-IMPL"],"owned_paths":["tests/release/local_install/phase1_local.rs"],"cargo_weight":1}
    ]
  },
  {
    "laneId":"PH1-V-SEC",
    "role":"independent-verifier",
    "scope":"broker denial, no side effects, sandbox capability, tool bounds, cancellation and resource reclamation",
    "owned_test_path":"crates/security/tests/phase1_release_security.rs",
    "stages":[
      {"id":"PH1-V-SEC-RED","type":"independent-test-author","status":"not-started","wave":1,"deps":["PH1-TURN-RED","PH1-BROKER-RED","PH1-SANDBOX-RED"],"owned_paths":["crates/security/tests/phase1_release_security.rs"],"cargo_weight":1},
      {"id":"PH1-V-SEC-GREEN","type":"independent-verifier","status":"not-started","wave":3,"deps":["PH1-TURN-IMPL","PH1-BROKER-IMPL","PH1-SANDBOX-IMPL","PH1-TOOLS-IMPL","PH1-BOUNDS-IMPL","PH1-AGENTS-IMPL"],"owned_paths":["crates/security/tests/phase1_release_security.rs"],"cargo_weight":1}
    ]
  }
]
```

The verifier stage IDs `PH1-V-LOCAL` and `PH1-V-SEC` are intentionally distinct
from their RED authors. They may read the frozen files but cannot change them.
The proposal's integration dependency names those verifier receipts, so a
candidate cannot unlock its parent through an implementer self-report.

## Wave protocol and exit criteria

### Wave 1: RED and freeze

Admission: 18 lane RED authors plus two independent verifier RED authors. Text
occupancy is bounded by the effective controller/provider cap; cargo commands
are always one at a time.

Exit only when every stage has:

- successful claim before file creation;
- executable test discovery and compilation;
- failure caused by the missing behavior, not an import or compile error;
- test-source SHA-256 and command-manifest SHA-256;
- disposable fixture declaration and no real HOME, database, secret or network;
- reviewer confirmation that the frozen test is immutable;
- row status `completed` only for the test-author's RED obligation, never for
  product acceptance.

If a RED test is already green, it is not silently reclassified. Stop, record
`NOT RED`, and request an independent contract decision.

### Wave 2: minimum implementation GREEN

Admission: 18 implementation lanes, with two verifier slots reserved rather
than falsely filled. Each implementer gets exactly one product file. Shared
files are prewired by the serialized integrator. Each implementation runs the
same frozen command and records candidate revision, result counts, stderr tail,
resource observation, and zero test edits.

Exit only when all candidate frozen tests and affected focused regressions are
green on the candidate branch. This is candidate GREEN, not acceptance. A
provider, signing, OS backend or memory failure remains `blocked` with exact
reproduction; it cannot be repaired by weakening a test.

### Wave 3: independent verification and serialized integration

The two verifier lanes run first or as soon as their implementation dependencies
are green. The 18 integration tasks are admitted through one integration mutex;
only one Cargo-heavy validation runs globally. Each task fetches current
`origin/main`, rebases or merges without force, checks ownership, reruns its
frozen command and affected regression, commits only its one product file plus
allowed receipt/row, pushes its lane branch, then is integrated by the
serialized authority. A failed merge or post-integration test leaves the task
unaccepted and dependent parents blocked.

Wave exit requires:

- both verifier receipts on exact candidate revisions;
- every integrated commit reachable from exact `origin/main`;
- `git merge-base --is-ancestor <integrated-commit> origin/main` success;
- frozen hashes unchanged from RED;
- affected tests rerun after integration, not only on the candidate branch;
- `python3 tools/convergence_gate.py` rerun;
- `python3 tools/validate_repository.py` rerun;
- installed local journey rerun on the same `origin/main` revision.

Current static gates are expected to remain blocked by the observed 51/58
findings until the controller reconciles them. The wave must report that fact,
not edit guards or call the local parent complete.

## Commands, hashes and gates

All shell commands use the repository RTK convention. One resource-heavy Cargo
command at a time:

```sh
rtk git status --short --branch
rtk git rev-parse HEAD
rtk git rev-parse origin/main
rtk git merge-base --is-ancestor origin/main HEAD
rtk git diff --check

rtk python3 tools/validate_repository.py
rtk python3 tools/convergence_gate.py
rtk python3 tools/completion_plan.py --check
rtk python3 tools/lane_gate.py --json

rtk CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 timeout 180 cargo test -p <crate> --test <target> -- --test-threads=1
rtk CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 timeout 300 cargo test -p <crate> --lib -- --test-threads=1

rtk shasum -a 256 <frozen-test-file>
rtk shasum -a 256 <command-manifest>
rtk git diff --exit-code <frozen-test-file>
```

On Linux runners use `sha256sum` where `shasum` is unavailable. The command,
test and manifest hashes are receipt fields, never prose-only claims. Cargo
weight is one permit for every test, build, check or integration rerun. Text
workers can be logically concurrent only within the reported provider and
controller cap; they must not launch concurrent Cargo jobs.

### Claim, reclaim and retirement protocol

```text
claim: cc.claim(root, TASK_ID, session, worklog/TASK_ID.md)
update: cc.update(root, TASK_ID, session, completed|blocked, exact_note)
report: cc.scratchpad_report(document, session)
reclaim: cc.reclaim(root, TASK_ID, orchestrator_session, evidence)
```

No worker edits `claims.json` directly. `completed` requires frozen tests green
on the integrated revision; otherwise use `blocked`. A foreign live claim is
untouchable. Reclaim requires proof of provider/process exit or expired
heartbeat, preserves the old worklog, sets `not-started`, and uses a fresh
session. It does not erase evidence or replay ambiguous side effects.

The prior provider attempt failed before claim and before files, so it is retired
with no reclaim and no fabricated attempt count. Current remaining reliable
route is exactly `9router-xk-gpt56-luna`. A route that later completes
successfully may be re-admitted only by explicit user decision. Provider auth,
rate, budget, signing, or security failures stop admission; no route substitution
or budget increase is implicit.

### Branch and landing protocol

For each proposed stage, use a persistent branch:

```text
lane/PH1-<TASK-ID>-20260924
```

The planning artifact itself uses `plan/PHASE1-VERTICAL-WAVE-DAG`. Required
landing sequence:

```sh
rtk git fetch origin main
rtk git rebase origin/main
rtk git status --short --branch
rtk git diff --check
rtk git add <owned-file> worklog/<TASK-ID>.md tasks/completion/claims.json
rtk git commit -m "PH1 <TASK-ID>: <stage>"
rtk git push -u origin lane/PH1-<TASK-ID>-20260924
```

The integrator then rebases or merges serially, reruns the frozen command, and
pushes `main`. Never force-push, delete a remote lane branch, edit frozen tests,
or treat a pushed branch as integrated. Proof is:

```sh
rtk git rev-parse HEAD
rtk git rev-parse origin/main
rtk git merge-base --is-ancestor <commit> origin/main
rtk git status --porcelain=v1
rtk git ls-remote origin refs/heads/main refs/heads/lane/PH1-<TASK-ID>-20260924
```

Acceptance needs `HEAD == origin/main` at the verified integrated revision and
clean status. This proposal's final clean check is performed after its own
commit/push; before that, the own claim/worklog are expected changes.

## Parent, authority and blocker rules

Parent `PH1-LOCAL-APP` remains open whenever any note says repair child,
follow-up, unwired, unproven, state-only, partial, missing, or no acceptance.
The worker cannot accept the parent. The trusted controller and independent
verifier decide acceptance after exact integrated reruns.

Current blockers and owners:

| Blocker | Evidence | Owner or disposition |
|---|---|---|
| 51 canonical backlog errors | `python3 tools/validate_repository.py`; baseline `8a91a7b` and `576cda6` guard worklog | Controller/integration authority; no lane-side edits. |
| 58 convergence findings | `python3 tools/convergence_gate.py`; off-plan claims and no-acceptance notes | Controller reconciles claims and parent state. |
| Current lane cap six, route absent from configured pools | `config/controller.settings.json:4-29` | Operator/user must authorize route and concurrency change. |
| One Cargo permit required under 8 GiB | `config/completion-controller.json:13-17`; host rule | Scheduler/integrator; no parallel Cargo. |
| Native OS sandbox platform coverage | `docs/SECURITY.md:36-60` | Actual platform verifier; Darwin/Linux capability result required. |
| Provider credentials or live canary | `ralph.completion.json:35`; `SHIP-003` contract | User-authorized provider only; fixture cannot prove live canary. |
| macOS notarization, Windows signing, release artifacts | `tasks/completion/delivery.json:12-18` | Signing authority; absent identity is blocked. |
| Cloudflare gateway credentials and real devices | `ralph.completion.json:12-15,35`; `SHIP-005` | Administrator/device owner; no fabricated remote or mobile evidence. |
| Canonical `lane_gate.py` scope | `tools/lane_gate.py:11-13` and protection inventory | Integrator must extend or select an authorized verifier; this proposal does not edit it. |
| Release assurance ownership | `sources/release-assurance-gap.json:1-17,140-174` | Release/controller authority; no REL acceptance inferred. |

No signing identity, Cloudflare credential, physical iOS/Android device, or live
provider credential is assumed. A blocked external dependency does not become a
green fixture result. Source-controlled protection also does not prove hosting
platform activation, per `docs/REPOSITORY_PROTECTION.md:150-167`.

## Critical path and ETA

Critical path:

```text
PH1-* RED/freeze
  -> all 18 implementation candidates
  -> PH1-V-LOCAL and PH1-V-SEC independent GREEN
  -> one-at-a-time 18 integrations
  -> installed local journey on exact origin/main
  -> canonical guard and convergence reconciliation
  -> trusted parent/release decision
```

Estimated elapsed time after authority, route and runner admission: RED/freeze
0.5-1 day, implementation 1-2 days with at most six currently authorized text
workers, verification 0.5-1 day, serial integration and reruns 1-2 days.
Planning estimate: **3-6 working days**, not a promise. Current ETA is
**blocked/no release date** because 51 guard errors, 58 convergence findings,
the six-lane controller cap, missing route authorization, and external release
authority remain unresolved. Web, remote, mobile, live-provider and signed
release DAGs are explicitly downstream of the local boundary and are not counted
as complete here.

## Verification receipt for this artifact

Read-only checks completed:

```text
rtk git status --short --branch
  plan/PHASE1-VERTICAL-WAVE-DAG...origin/main
  M tasks/completion/claims.json   # own claim only before worklog/commit

rtk git rev-parse HEAD
  8a91a7b49a5a1c948218ad8f176d44e015530dcb

rtk git rev-parse origin/main
  8a91a7b49a5a1c948218ad8f176d44e015530dcb

rtk git diff --check
  clean

rtk python3 tools/convergence_gate.py
  blocked, total=58

rtk python3 tools/validate_repository.py
  failed, validate_backlog_exhaustion: 51 error(s)
```

The worklog JSON is a proposal and has no frozen test hash, candidate revision,
integrated revision or acceptance receipt. Those fields remain null until the
trusted controller admits and runs the proposed stages.
