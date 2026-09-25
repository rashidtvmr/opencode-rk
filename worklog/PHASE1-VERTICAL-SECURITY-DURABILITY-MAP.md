{
  "schema": "phase1.vertical_security_durability_map.v1",
  "task": "PHASE1-VERTICAL-SECURITY-DURABILITY-MAP",
  "artifact_kind": "machine_readable_convergence_plan",
  "candidate_revision": "8a91a7b49a5a1c948218ad8f176d44e015530dcb",
  "parent": {
    "id": "APP-012",
    "title": "Installed local application golden journey",
    "source": "tasks/completion/local.json:15",
    "status": "open",
    "frozen_installed_journey": {
      "path": "crates/server/tests/app012_tool_journey_red.rs",
      "sha256": "945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50",
      "candidate_checkout_state": "absent; historical frozen hash retained from independent receipts"
    },
    "close_rule": "Do not close while any path, recovery child, generation, owner lock, approval resume, keyring, credential authority, daemon/live-turn wiring, or installed-verifier row remains open, blocked, partial, unwired, or unproven."
  },
  "authority": {
    "policy": [
      "AGENTS.md:Authority and ownership",
      "AGENTS.md:Convergence and parent-completion boundary",
      "AGENTS.md:Non-negotiable engineering rules",
      "PLAN.md:61-120,162-179,227-239",
      "docs/TDD.md:21-83",
      "docs/SECURITY.md:9-82",
      "docs/CONVERGENCE.md:8-68,87-109"
    ],
    "method": "Source paths and symbols below are current-code evidence. Prior commits and worklogs are evidence of RED, implementation, or verification only; they do not authorize scope expansion or parent acceptance.",
    "classification": [
      "scoped_accept means only the named behavior was independently verified",
      "open means a required behavior or edge is not proven",
      "blocked means the required RED or implementation seam cannot be created honestly under current authority",
      "proposed means a future discovery or integration child, not a completed task",
      "green_prerequisite means useful landed behavior that still needs caller wiring or integrated proof"
    ]
  },
  "invariants": {
    "security": [
      "Authorization remains outside model output. Every effect uses the trusted broker immediately before the concrete effect.",
      "Permission * cannot lift mandatory system protection or human-only authority.",
      "Denied operations have no effect, no process spawn, no file mutation, no approval consumption, and no secret or path leakage.",
      "A prompt, regex, project config, hook, persisted approval row, or daemon bearer cannot create human authority or sandbox isolation.",
      "No direct secret-file access, unrestricted inherited environment, plaintext credential fallback, secret logging, or automatic replay of an ambiguous side effect."
    ],
    "durability": [
      "Startup ownership is serialized by a real OS-held owner lock before writable open or recovery scan.",
      "owner_generation advances exactly once per successful owner startup under synchronous durability; stale writer commands fail closed.",
      "Recovery uses one bounded transaction and deterministic indexed batches; terminal rows remain unchanged.",
      "Running, dispatched, or externally ambiguous work becomes uncertain and is never blindly replayed.",
      "Approval evidence is not authority. Resume revalidates digest, scope, requester, principal, expiry, origin, policy generation, owner generation, and broker decision.",
      "One durable decision and one dispatch claim are guaranteed; exactly-once completion of an unobservable non-idempotent external effect is not claimed."
    ],
    "resources": [
      "Use bounded byte and row limits, fixed semaphore capacity, owned JoinHandles, cancellation and drain; no detached task or unbounded retained output.",
      "Keep at least 2 GiB host headroom. Run one resource-heavy Cargo validation at a time with CARGO_BUILD_JOBS=1 or 2 and RUST_TEST_THREADS=1 or 2.",
      "Use disposable databases, workspaces, provider fixtures, homes, keyring namespaces, and process fixtures only."
    ]
  },
  "waves": [
    {
      "id": "W1",
      "name": "Protected effects and restart fencing",
      "goal": "Close the concrete protected-read identity gaps, make bounded recovery complete, and establish OS ownership plus generation enforcement before live execution is wired.",
      "entry_gates": [
        "Keep protected-path scoped acceptance c269715/8916868 intact; do not broaden the policy or edit frozen tests.",
        "Carry the frozen recovery RED 2d0a6fe unchanged.",
        "Treat facade recovery as a repair child, not as parent acceptance."
      ],
      "chains": [
        "J1 protected path identity and descriptor-bound read",
        "J2 startup recovery, child drainage, generation fencing, and owner lock"
      ],
      "exit_gates": [
        "Hardlink aliases cannot bypass protected identity policy; authorization and opened descriptor refer to the same checked object.",
        "Unreadable or missing roots have an explicit availability contract and bounded, non-leaking failure; no accidental deny-all or fail-open ambiguity remains.",
        "A root with 501 open children drains all children over bounded successive opens without stranding rows; terminal children remain unchanged.",
        "Stale owner_generation is rejected by the real writer caller, not merely stored or passed through APIs.",
        "Concurrent startup proves one OS owner; occupied or reused PID never kills an unrelated process.",
        "Independent RED, implementation, and verifier receipts exist for each behavior."
      ],
      "integration_owner": "storage/server integration spine",
      "parent_status_after_wave": "open_until_W2_live_wiring"
    },
    {
      "id": "W2",
      "name": "Durable approval, credential authority, and live-turn wiring",
      "goal": "Replace terminal in-memory human gates and environment-only credential detection with one daemon-owned, durable, brokered execution path.",
      "entry_gates": [
        "W1 owner lock and generation boundary is verified before accepting writable execution.",
        "Approval contract corrections from 6d2f97f are reviewed by an independent verifier; no RED is frozen against absent symbols.",
        "Keyring remains blocked until a trusted controller records the APP012-KEYRING-BOOTSTRAP authority decision described by 4dc8811/4814357."
      ],
      "chains": [
        "J3 durable approval request, authenticated human decision, conditional resume, and restart reconciliation",
        "J4 keyring-or-deny provider credential lifecycle and onboarding authority",
        "J5 daemon/live turn integration through storage, broker, concrete tools, and second client"
      ],
      "exit_gates": [
        "Approval request, decision, resume claim, result or uncertain state, and bounded outbox records commit through the durable owner.",
        "Expiry uses one inclusive boundary: now >= expires_at is unusable; approve-versus-expiry has one CAS winner.",
        "Bearer authentication is transport authentication only; human principal, client identity, local origin, and revocation are independently verified.",
        "Canonical operation digest and encoding are versioned and identical across security, tools, storage, and wire IDs; stale bindings fail before effect.",
        "Consumed or dispatched work never auto-replays after restart; ambiguous outcomes remain visible for explicit reconciliation.",
        "Keyring uses exact keyring = \"=3.6.3\" target features, bounded owned blocking operations, no File0600/config/SQLite/env fallback, and separate native A/B/C receipts.",
        "Live create_turn_stream no longer converts RequireHuman to a terminal error; it reaches the durable approval/resume owner and concrete brokered executor.",
        "A second client observes one durable execution and cannot duplicate work."
      ],
      "integration_owner": "daemon/runtime integration spine",
      "parent_status_after_wave": "open_until_W3_installed_verifier"
    },
    {
      "id": "W3",
      "name": "Shell lifecycle and installed APP012 proof",
      "goal": "Integrate repaired shell dispatch and cancellation into the same brokered live path, then independently verify the complete installed journey and resource evidence.",
      "entry_gates": [
        "Preserve batch-immediate RED hash aaad6ab33406a3d5cecf8ca8d5ce0ae6ca16ba7aa22ce264d1abf5240efdc0af and broker hash ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8.",
        "Treat cancellation candidate hash 44c228804107b65451c04fbbad9e93d14f5494605c2bcab40ad740c8aafc6827 as Unix verification evidence, not a RED or cross-platform acceptance.",
        "Use only the release artifact, disposable HOME, real terminal, provider fixture, real broker, real storage, and one authenticated daemon."
      ],
      "chains": [
        "J6 shell batch immediate-result parity",
        "J7 Unix process cancellation, timeout, output and child cleanup",
        "J8 installed APP012 golden journey and independent release verification"
      ],
      "exit_gates": [
        "Batch Immediate results retain exact errors or denial records in request order, with no spawn, store write, or permit leak.",
        "Unix cancellation and timeout reap owned process groups and descendants, drain bounded output, and leave no marker or child; unsupported targets fail honestly.",
        "Shell approval and denial use the same concrete broker path as file and provider tools; no legacy allowlist-only bypass remains on the live path.",
        "Fresh install opens native UI, setup, fixture turn, real approval/tool operation, second client observation, restart/resume, denial, interruption, daemon restart, and terminal restoration.",
        "Independent verifier reruns the frozen installed test on the exact integrated revision and archives terminal, process-tree, resource, artifact, and hash evidence."
      ],
      "integration_owner": "release integration and independent verifier",
      "parent_status_after_wave": "only_verifier_may_decide"
    }
  ],
  "journeys": [
    {
      "id": "J1",
      "name": "Protected-path read identity",
      "status": "open_repair_child",
      "current_behavior": [
        "c2697156e3078b4dff50cb8093bd502840c12a4c changes read-class denial to fixed path-free file read denied; 8916868dc31ba98f98bf26d33dbe9d43b1758d10 accepts containment, mandatory secret/system ordering, redaction, and no-side-effect denial only.",
        "crates/security/src/lib.rs:256-365::PermissionBroker::authorize_file/is_within_readable_root canonicalizes the target and roots; canonicalization cannot identify a hardlink alias and root canonicalization failure can deny all reads.",
        "crates/tools/src/file_ops.rs:173-217::execute_authorized authorizes before execution; crates/tools/src/file_ops.rs:220-266::read_file later calls raw fs::File::open(path), leaving descriptor-bound authorization/open TOCTOU unresolved."
      ],
      "target_contract": {
        "identity": "Authorize an object identity, then open the same object through a descriptor-bound or equivalent checked operation; reject hardlink alias escape according to the approved identity policy.",
        "availability": "Distinguish missing/unreadable target or root from protected denial with fixed bounded codes; define whether an unavailable configured root is unavailable, not silently an empty readable root.",
        "side_effect": "Read denial returns success=false, empty content, fixed path-free error, one broker audit, and zero file bytes exposed.",
        "failure_states": ["secret_or_system_denied", "outside_or_traversal_denied", "hardlink_identity_mismatch", "descriptor_swap_detected", "root_unavailable", "read_io_failure"],
        "persistence": "No protected read approval or secret bytes are persisted; audit is bounded and redacted.",
        "resource_bounds": "Bound path bytes, read output at existing 64 KiB limit, descriptor metadata, and one authorization/open attempt; no repeated unbounded canonicalization."
      },
      "ownership": {
        "red": "Independent security/tools RED author; new repair child for hardlink, descriptor swap, and unavailable-root cases. Existing protected RED remains frozen at ed524ec1a239e8403c76d133e6929683a5a751f758f662b91b0a8d5a2b687d77.",
        "implementation": "Security owner for object identity/root availability contract; tools file-operations owner for descriptor-bound open. Shared broker registration and integration wiring belong to the integrator.",
        "verifier": "Independent security verifier on actual macOS target, then integrated APP012 verifier.",
        "not_owned": ["permission wildcard policy expansion", "frozen protected tests", "release acceptance"]
      },
      "dependencies": ["W1", "real platform filesystem identity API", "bounded read API", "broker audit contract"],
      "commands": {
        "existing_green": [
          "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_protected_path_red -- --test-threads=1",
          "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-security --lib",
          "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_tool_journey_red -- --test-threads=1"
        ],
        "future_red": "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test app012_protected_identity_red -- --test-threads=1",
        "future_verifier": "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test app012_protected_identity_red -- --test-threads=1"
      },
      "evidence": ["c269715", "8916868", "worklog/APP012-PROTECTED-PATH-VERIFY.md", "crates/security/src/lib.rs:256-365", "crates/tools/src/file_ops.rs:173-266"],
      "blockers": ["No accepted hardlink identity contract or descriptor-bound implementation exists.", "Unreadable-root availability is not frozen by the existing RED."]
    },
    {
      "id": "J2",
      "name": "Startup recovery, bounded children, generation, and owner lock",
      "status": "partial_repair_and_wiring_open",
      "current_behavior": [
        "f83637df72c0410cfde84bdee225d0b060248c2c and verifier 48d0351040efb83ffd8fd2cc65bcdb89ee217620 implement and scope-accept one IMMEDIATE startup transaction, FULL policy, generation increment, clean marker reset, and LIMIT 500 recovery in crates/storage/src/facade.rs:148-222.",
        "2d0a6fead69210ed985cc46e969fbdcb64282aba freezes a valid RED showing that a root with 501 open children strands one child after the root changes to uncertain; frozen RED hash fe78660842335e79e33787cb95bb675a80c65e541b3bd52fa9e92d1efc9138b0.",
        "crates/storage/src/execution_v2.rs:20-50 accepts caller-supplied owner_generation but does not compare it to workspace_state; current server live turn has no ExecV2/ApprovalsV2 caller.",
        "crates/server/src/daemon.rs:49-103::PidLock provides an OS file lock, but storage startup does not acquire an owner lock before writable open/recovery. docs/storage/CRASH_CONSISTENCY.md:25-31,152-177 requires this protocol."
      ],
      "target_contract": {
        "startup": "Acquire one OS-held owner lock before writable open, validate DB identity, advance owner_generation exactly once under FULL, set clean_shutdown=0, recover bounded unsettled rows, then admit work.",
        "recovery": "Select at most 500 roots and their children from one deterministic transaction; child rows for selected roots cannot be stranded across later opens. Terminal states and timestamps remain unchanged.",
        "generation": "Every production writer transition and new execution compares caller generation against current workspace generation; stale generations fail without effect.",
        "lock": "Second owner fails or waits within the bounded policy; no stale PID or occupied port causes an unrelated process signal.",
        "failure_states": ["schema_or_checksum_failure", "owner_lock_busy", "generation_overflow", "stale_generation", "recovery_transaction_rollback", "uncertain_external_effect"],
        "persistence": "Recovery transitions running/dispatched work to uncertain, keeps single-owner fencing, never fabricates terminal failure, and never replays an ambiguous effect.",
        "resource_bounds": "One IMMEDIATE transaction, LIMIT 500 per bounded batch, indexed scans, no retained child list beyond bounded batch, busy timeout 5000 ms."
      },
      "ownership": {
        "red": "Existing child-bound RED owner remains frozen; new generation and OS-lock RED authors must be independent and separate from implementation.",
        "implementation": "Storage facade owner repairs selected-root child drainage; storage execution owner enforces generation predicates; daemon/runtime integration owner acquires and owns PidLock before storage startup.",
        "verifier": "Independent storage verifier for recovery and generation; independent daemon verifier for concurrent owner lock and no-kill behavior; integrated verifier for the real startup path.",
        "not_owned": ["approval semantics", "keyring persistence", "frozen restart RED", "parent acceptance"]
      },
      "dependencies": ["J1 is independent but both feed the same security verifier", "W1 owner-lock API", "workspace_state schema", "live execution caller"],
      "commands": {
        "existing_red": "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-storage --test app012_recovery_child_bound_red -- --test-threads=1",
        "existing_green": "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test --no-fail-fast -p opencode-rk-storage --test app012_restart_resume_red -- --test-threads=1",
        "future_generation_red": "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-storage --test app012_generation_fence_red -- --test-threads=1",
        "future_lock_verifier": "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_owner_lock_red -- --test-threads=1"
      },
      "evidence": ["f83637d", "48d0351", "2d0a6fe", "worklog/APP012-RESTART-RECOVERY-VERIFY.md", "crates/storage/src/facade.rs:27-43,148-222", "crates/storage/src/execution_v2.rs:20-81", "crates/server/src/daemon.rs:49-103", "docs/storage/CRASH_CONSISTENCY.md:25-31,152-185"],
      "blockers": ["Child-bound repair is not implemented.", "Generation is stored but not enforced by a production caller.", "OS owner-lock acquisition is not wired into storage startup."]
    },
    {
      "id": "J3",
      "name": "Durable approval and conditional resume",
      "status": "contract_corrected_no_runtime_red",
      "current_behavior": [
        "3f7405eed88dc568ad601249c8f2f74e44653465 defines the initial approval/resume contract; independent verifier 1f53b01176248c1b9dc364c8add06039d4d800a1 requires corrections before RED.",
        "6d2f97f90478abf1dd484e3865d8aa0fd72630bc records corrected expiry, authenticated principal, local-only, canonical digest, claim, retention, and concrete broker requirements.",
        "crates/storage/schema/v2/workspace.sql:222-249 and crates/storage/src/approvals_v2.rs:82-145 persist approval evidence and pending CAS, but lack complete requester/workspace/principal/decision-kind/claim/reconciliation fields and permit pending to consumed through the generic resolve API.",
        "crates/server/src/lib.rs:1126-1167,1360-1437 and :1639-1644 authorize a generic tool then converts RequireHuman into a terminal error; no durable decision route or resume channel exists.",
        "crates/server/src/daemon_auth.rs:150-176 authenticates daemon bearer transport, not human authority; crates/server/src/event_bus.rs transient PermissionRequested is not a durable request."
      ],
      "target_contract": {
        "request": "Persist exact canonical operation binding, principal/requester/workspace/session, policy and owner generations, expiry, decision kind, dispatch/reconciliation identity, and bounded outbox event atomically.",
        "decision": "Authenticated human principal and local origin are required; pending CAS has one winner; exact expiry boundary is now >= expires_at; human decision cannot directly consume approval.",
        "resume": "Atomically claim allowed-once approval plus execution/tool dispatch identity, then rerun concrete broker authorization immediately before effect.",
        "restart": "Pending expires boundedly; approved-not-started may resume only after all revalidation; consumed/dispatched/unknown never auto-replays and requires reconciliation.",
        "failure_states": ["unauthenticated", "principal_mismatch", "remote_local_only", "digest_mismatch", "scope_mismatch", "stale_policy", "expired", "replay", "claim_conflict", "ambiguous_effect", "capacity_exceeded"],
        "persistence": "Approval 0 pending, 1 allowed-once, 2 denied, 3 expired, 4 consumed by claim only; execution/tool uncertain states retain ownership and evidence.",
        "resource_bounds": "At most 500 approval/retention sweep rows, 1024 daemon pending approvals, 16 per session, bounded wait registrations and 4096-byte outbox payloads."
      },
      "ownership": {
        "red": "Independent server RED author after schema/API seam is admitted; no compile-failing test against absent RecoveryV2 or route symbols.",
        "implementation": "Storage/schema owner for durable fields and atomic claim; security owner for canonical digest and broker bridge; server/runtime owner for principal, route, continuation, restart recovery; tools owner for concrete reauthorization and result persistence.",
        "verifier": "Independent server verifier plus integrated installed-journey verifier; verifier rejects generic Tool authorization, terminal RequireHuman handling, missing caller, and ambiguous replay.",
        "not_owned": ["keyring native receipts", "shell platform matrix", "frozen APP012 journey edits"]
      },
      "dependencies": ["J2 generation and owner lock", "schema/API authority", "canonical digest decision", "concrete brokered executor", "daemon singleton runtime"],
      "commands": {
        "future_red": "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_approval_resume_red -- --test-threads=1",
        "future_storage_regression": "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-storage --tests -- --test-threads=1",
        "future_integrated": "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_tool_journey_red -- --test-threads=1"
      },
      "evidence": ["3f7405e", "1f53b01", "6d2f97f", "worklog/APP012-APPROVAL-RESUME-CONTRACT-VERIFY.md", "crates/storage/schema/v2/workspace.sql:222-249", "crates/storage/src/approvals_v2.rs:82-145", "crates/server/src/lib.rs:1126-1167,1360-1437,1639-1644"],
      "blockers": ["No complete durable approval claim/reconciliation schema or API is admitted.", "No authenticated human principal route or continuation owner exists.", "Current live RequireHuman path terminates instead of resuming.", "Current digest implementations differ across security, tools, and storage."]
    },
    {
      "id": "J4",
      "name": "Keyring-or-deny credential authority",
      "status": "blocked_bootstrap",
      "current_behavior": [
        "41f37f0 records the corrected exact keyring 3.6.3 contract, target feature matrix, namespace, UTF-8 password representation, no-fallback rule, bounded blocking lifetime, and native A/B/C boundary.",
        "4dc8811 independently rejects bootstrap authorization before a compiling behavioral RED; 4814357 records the honest blocker that no admitted public persistence seam exists.",
        "crates/providers/src/auth_store.rs:1-7,62-124,154-249,347-431 is planning-only and includes a File0600 fallback planner; it performs no persistence and must not be used as APP012 runtime authority.",
        "crates/providers/src/lib.rs:4-59 has no keyring persistence module or keyring dependency; crates/cli/src/daemon_client.rs:775-791 is environment-only; crates/cli/src/onboarding.rs:326-405,424-586 is in-memory."
      ],
      "target_contract": {
        "backend": "Use exact keyring = \"=3.6.3\", target-specific native features: macOS apple-native, Linux linux-native-sync-persistent plus crypto-rust, Windows windows-native; unsupported targets fail explicitly.",
        "authority": "Broker authorizes bounded save/load/delete; no env/config/SQLite/File0600 fallback. Missing or unavailable keyring opens setup or returns fixed unavailable code.",
        "lifetime": "Owned secret input and loaded secret remain within a bounded owner; synchronous native calls use bounded spawn_blocking admission and owned JoinHandles; timeout returns only after started work is retained for drain.",
        "failure_states": ["missing", "keyring_unavailable", "ambiguous", "corrupt_encoding", "invalid_identity", "oversize", "timeout", "cancelled", "unsupported_target"],
        "persistence": "A/B/C independent processes prove save, reconstruct/load, overwrite, delete/missing in disposable native namespaces; mock EntryOnly proves mapping only, not restart durability.",
        "resource_bounds": "Provider ID 128 bytes, secret 64 KiB, bounded derived labels, two native operations in flight, bounded pending queue and shutdown grace; no raw keyring error or secret in logs."
      },
      "ownership": {
        "red": "Independent keyring RED owner only after controller-approved bootstrap seam; K01-K09 use an application-owned durable fake, K10a/K10b separately cover native mock and adapter error mapping.",
        "implementation": "Provider persistence owner for orchestration; dependency owner for exact manifest and adapter; CLI/onboarding owner for save-before-commit; daemon owner for startup presence; provider client owner for scoped load.",
        "verifier": "Independent platform verifier runs macOS, Linux, and Windows GNU A/B/C receipts where supported; unavailable backend is blocked evidence, never a fake pass.",
        "not_owned": ["PROV-022 File0600 planner", "environment compatibility behavior as APP012 proof", "live user keychain mutation during local RED"]
      },
      "dependencies": ["controller bootstrap authority", "J3 broker credential capability", "onboarding caller", "daemon startup", "native platform environments"],
      "commands": {
        "future_red": "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-providers --test keyring_persistence_red -- --nocapture --test-threads=1",
        "future_mock": "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-providers --test keyring_mock_api -- --test-threads=1",
        "native_receipt": "Controller-selected per-target process A/B/C command; exact target triple, lock hash, feature resolution, backend availability, cleanup, and redaction must be recorded."
      },
      "evidence": ["41f37f0", "4dc8811", "4814357", "worklog/APP012-KEYRING-API-SEAM-CORRECTION-VERIFY.md", "crates/providers/src/auth_store.rs:1-7,62-124,347-431", "crates/providers/src/lib.rs:4-59", "crates/cli/src/daemon_client.rs:775-791", "crates/cli/src/onboarding.rs:326-405,424-586"],
      "blockers": ["No controller checkpoint authorizes a dependency-free seam or implementation-before-RED exception.", "No compiling behavioral K01-K09 RED hash exists.", "No keyring dependency, native adapter, daemon-owned service, or provider scoped-load caller exists."]
    },
    {
      "id": "J5",
      "name": "Daemon and live-turn integration spine",
      "status": "unwired_open",
      "current_behavior": [
        "crates/server/src/lib.rs:1126-1167 drives create_turn_stream through legacy SessionService message persistence; :1143-1200 dispatches a generic OperationIntent::Tool and a ToolExecutor path rather than durable ExecV2/ApprovalsV2 ownership.",
        "crates/server/src/lib.rs:1639-1644 turns RequireHuman into a terminal error string; no approval continuation or authenticated human decision route exists.",
        "SchemaV2::open_existing has production callers in crates/storage/src/facade.rs:27-43 and crates/sessions/src/branch_v2.rs:84-103, but neither caller drives live execution or approval recovery.",
        "worklog/E2E-APP.md:14-20,63-81 records bare CLI/daemon/chat wiring and an honest offline degradation, not full brokered tool approval, keyring persistence, or restart acceptance."
      ],
      "target_contract": {
        "composition": "One daemon-owned runtime composes provider, session, storage, security, tools, events, approvals, and credential service; CLI/TUI/web/headless attach to it.",
        "turn": "Provider fixture requests a concrete file/process/tool operation; real broker authorizes or creates durable approval; tool result is persisted and fed back to provider.",
        "clients": "Second client sees the same session, turn, approval, and execution state; no duplicate private executor or work.",
        "failure_states": ["missing_credentials_setup", "auth_failure", "provider_failure", "approval_pending", "denied_no_side_effect", "interrupted_uncertain", "daemon_restart_recovered", "duplicate_owner_rejected"],
        "persistence": "Session, turn, approval, execution, tool result, uncertain state, and durable outbox survive client exit and daemon restart.",
        "resource_bounds": "One Tokio runtime, bounded event replay, bounded tool output, one owner per session, bounded client subscriptions, owned shutdown joins."
      },
      "ownership": {
        "red": "Independent installed-journey test author; existing APP012 journey is immutable.",
        "implementation": "Daemon/runtime integration owner wires startup, storage facade, generation, approval coordinator, broker, concrete executor, provider, event bus, and client routes.",
        "verifier": "Independent installed E2E verifier checks real binary and process tree; no module-only or mocked success accepted.",
        "not_owned": ["source audit artifact itself", "provider vendor production account", "release acceptance authority"]
      },
      "dependencies": ["J2", "J3", "J4", "existing APP-001 through APP-011 integrated revisions", "native TUI path"],
      "commands": {
        "existing_cli_regression": "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli -- --test-threads=1",
        "existing_installed_journey": "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_tool_journey_red -- --test-threads=1",
        "future_release": "Release-verifier-owned installed command from a clean disposable HOME and PTY; archive exact artifact and revision hashes."
      },
      "evidence": ["crates/server/src/lib.rs:1126-1200,1639-1644", "crates/storage/src/facade.rs:27-43", "crates/sessions/src/branch_v2.rs:84-103", "worklog/E2E-APP.md:14-20,63-81", "docs/CONVERGENCE.md:8-24,54-68"],
      "blockers": ["No live durable approval/resume caller.", "No live v2 execution owner.", "No integrated second-client/restart proof through installed native UI."]
    },
    {
      "id": "J6",
      "name": "Shell batch immediate-result parity",
      "status": "green_prerequisite_integration_open",
      "current_behavior": [
        "6e1695dbd01981d22c95291b7fe576aeb56e5d20 freezes six compiling behavioral RED tests at SHA aaad6ab33406a3d5cecf8ca8d5ce0ae6ca16ba7aa22ce264d1abf5240efdc0af.",
        "55acf2942d950fea0b4c90c336ec10713f0fa81d implements extraction of Ready::Spawn only, preserving Ready::Immediate errors/records/order and preventing immediate store writes.",
        "30de11af23a4db7ef701d5a753933a18897954d independently accepts the repair; stale broad-lib shell expectations and /bin/false host fixture failure are unrelated."
      ],
      "target_contract": {
        "observable": "Unknown, disabled, oversized, policy-denied, empty, and shell-denied batch entries equal single-dispatch outcomes in request order.",
        "failure_states": ["unknown", "disabled", "input_too_large", "denied", "empty_name", "shell_broker_required", "join_failure"],
        "persistence": "Immediate fail-closed entries produce no output-store write; only spawned valid entries record bounded results.",
        "resource_bounds": "Semaphore permits are reclaimed; input/output byte caps and existing batch cardinality remain bounded.",
        "wiring": "The repaired dispatcher must be the live daemon/tool path, not only a directly tested module."
      },
      "ownership": {
        "red": "Completed independent RED owner, frozen hash above.",
        "implementation": "Completed tools dispatcher owner, source crates/tools/src/registry_dispatch.rs.",
        "verifier": "Completed independent verifier 30de11a; integration verifier must still exercise live caller.",
        "not_owned": ["stale broad unit tests", "shell cancellation RED", "parent APP012 acceptance"]
      },
      "dependencies": ["J3 concrete broker dispatch", "J5 live dispatcher caller"],
      "commands": {
        "frozen": "CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-tools --test registry_batch_immediate_red -- --test-threads=2",
        "broker": "CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-tools --test phase1_shell_broker -- --test-threads=2",
        "verifier": "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test registry_batch_immediate_red -- --test-threads=1"
      },
      "evidence": ["6e1695d", "55acf29", "30de11a", "worklog/TOOL-SHELL-BATCH-IMMEDIATE-RED.md", "worklog/TOOL-SHELL-BATCH-IMMEDIATE-GREEN.md", "crates/tools/src/registry_dispatch.rs:218-347"],
      "blockers": ["No live caller proof; broad legacy tests remain stale and must not be weakened."]
    },
    {
      "id": "J7",
      "name": "Shell cancellation and process lifetime",
      "status": "unix_scoped_acceptance_only",
      "current_behavior": [
        "56f8e2b3d9bd1f50481d7c8b0ee9fc0a13b662d7 records a nine-case candidate suite that ran 9/9 green against existing seam; it is explicitly NOT RED and NOT FROZEN.",
        "0fb07076d4608f3c7b08eeeb2342cf11b85177d0 independently accepts Unix cancellation behavior only: process groups, descendants, readers, timeout, output cap, readiness close, terminal precedence, and broker denial.",
        "crates/tools/src/shell_tool.rs:154-273 retains a legacy ShellTool with allowlist-only behavior when authz is None; crates/tools/src/executor.rs authorized process seam is the stronger path and must be the live owner."
      ],
      "target_contract": {
        "observable": "Pre-start denial/cancel performs no spawn; post-ready cancellation and timeout reap owned group/child/readers; normal exit wins when already terminal; output remains bounded.",
        "failure_states": ["cancelled_before_start", "cancelled", "timed_out", "readiness_receiver_closed", "spawn_failed", "unsupported_platform", "broker_denied", "human_required"],
        "persistence": "Cancellation or uncertain external outcome is recorded by the durable execution owner; no detached child or replay is allowed.",
        "resource_bounds": "Owned process group, bounded stdout/stderr cap, bounded readiness, cancellation token, JoinHandle ownership, bounded cleanup timeout.",
        "platform": "Unix receipt is scoped; Windows support requires separate real platform implementation and verification."
      },
      "ownership": {
        "red": "No valid cancellation RED currently exists because seam behavior already passes; preserve candidate as verifier evidence, do not fabricate missing behavior.",
        "implementation": "Tools process-seam owner for any remaining live wiring or platform repair; runtime owner for cancellation propagation.",
        "verifier": "Independent Unix process verifier accepted current seam; future integrated verifier checks live daemon cancellation and no child survivors.",
        "not_owned": ["candidate RED hash as frozen test", "legacy allowlist compatibility tests", "Windows acceptance without a real backend"]
      },
      "dependencies": ["J3 durable uncertain state", "J5 live process owner", "J6 brokered batch path"],
      "commands": {
        "candidate": "CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test phase1_shell_cancellation -- --test-threads=1",
        "broker": "CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-tools --test phase1_shell_broker -- --test-threads=2",
        "verifier": "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo check -p opencode-rk-tools"
      },
      "evidence": ["56f8e2b", "0fb0707", "worklog/TOOL-SHELL-CANCELLATION-RED.md", "worklog/TOOL-SHELL-CANCELLATION-VERIFY.md", "crates/tools/src/shell_tool.rs:154-273", "crates/tools/src/executor.rs:731-1098"],
      "blockers": ["Candidate is not a RED and cannot authorize implementation.", "Legacy ShellTool has an optional broker and must not remain the live bypass.", "No Windows process-lifecycle proof."]
    },
    {
      "id": "J8",
      "name": "Installed APP012 golden journey",
      "status": "parent_open_no_acceptance",
      "current_behavior": [
        "APP012 requires install, bare opencode2, setup, coding turn, approval/tool edit, concurrent client, restart/resume, failure injection, and archived resource/artifact evidence as defined by tasks/completion/local.json:15.",
        "worklog/E2E-APP.md:14-20 proves useful bare CLI/daemon/chat wiring and honest offline degradation, but explicitly leaves only the OpenAI server provider wired and does not prove the complete security/durability chain.",
        "The frozen journey hash remains 945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50; no artifact in this map changes it."
      ],
      "target_contract": {
        "journey": "Fresh disposable HOME and release artifact open native TUI, invoke one authenticated daemon, configure or honestly report credential state, submit provider fixture turn, execute real brokered tool, persist, attach second client, restart, resume/reconcile, deny, interrupt, and restore terminal.",
        "failure_states": ["setup_required", "provider_unavailable", "approval_pending", "denied", "interrupted", "uncertain_after_crash", "owner_conflict", "resume_rejected", "unsupported_platform"],
        "persistence": "History, tool result, approval decision, uncertain state, owner generation, and no-replay evidence are durable and reconstructed after restart.",
        "resource_bounds": "Record complete process tree, RSS/CPU, FD/child counts, output bytes, replay buffers, queue counts, and cleanup; do not report daemon-only resources.",
        "acceptance": "Only an independent verifier can accept the exact integrated revision; a module GREEN, static convergence gate, or ledger completed row is insufficient."
      },
      "ownership": {
        "red": "Independent installed E2E/test-author lane owns any new APP012 extension; existing frozen journey is immutable.",
        "implementation": "Integration spine owns all caller wiring and release artifact composition; leaf owners retain their file boundaries.",
        "verifier": "Independent release verifier owns PTY, clean-machine, provider fixture, second-client, restart, failure-injection, resource and artifact receipts.",
        "not_owned": ["fake live credentials", "user database", "production provider account", "acceptance declaration by implementation lane"]
      },
      "dependencies": ["J1", "J2", "J3", "J4", "J5", "J6", "J7", "APP-008", "APP-010", "APP-011", "TUI-010"],
      "commands": {
        "frozen_hash": "shasum -a 256 crates/server/tests/app012_tool_journey_red.rs",
        "focused": "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_tool_journey_red -- --test-threads=1",
        "release": "Verifier-owned clean-machine installed command with PTY, exact release artifact hash, terminal recording, process-tree receipt, and no Cargo/Node/Bun/Zig runtime."
      },
      "evidence": ["tasks/completion/local.json:15", "worklog/E2E-APP.md:14-20,57-81", "docs/CONVERGENCE.md:8-24", "crates/server/tests/app012_tool_journey_red.rs"],
      "blockers": ["Every unresolved chain in this map blocks parent closure.", "No exact integrated revision has passed the installed golden journey and independent verifier."]
    }
  ],
  "ownership_boundaries": {
    "independent_red": [
      "Authors compiling behavior-failing tests only after the required public seam exists.",
      "Freezes source hash and command manifest; never edits tests after freeze.",
      "Uses disposable fixtures and asserts no side effects for denials, rejection, expiry, and cancellation."
    ],
    "implementation": [
      "Changes only leased product files and real callers; no stubs, todo, unimplemented, mock-only production success, or hidden runtime.",
      "Preserves protocol/state/persistence semantics and reports deviations instead of weakening tests.",
      "Does not edit controller state, frozen tests, verifier configuration, dependency acceptance, security policy, release criteria, or another lane's shared contract."
    ],
    "verifier": [
      "Runs frozen RED/GREEN tests against exact candidate and integrated revisions; independently audits side effects, resource cleanup, redaction, and caller reachability.",
      "Rejects state-only, module-only, generic authorization, mocked success, missing caller, ambiguous replay, and unsupported platform claims.",
      "Owns acceptance decision; completed claims and static gates are coordination evidence only."
    ],
    "integration_spine": [
      "Owns additive shared registration, schema/API convergence, daemon/runtime composition, live turn caller, second-client observation, release artifact, and exact integrated reruns.",
      "Must keep at least four integration-spine roles and two independent test/verifier roles while docs/CONVERGENCE.md:30-43 remains unsatisfied."
    ]
  },
  "global_blockers": [
    {
      "id": "B1",
      "severity": "release_blocking",
      "statement": "APP012 parent remains open: hardlink identity, descriptor-bound TOCTOU, unreadable-root availability, recovery child repair, generation enforcement, OS owner lock, approval-resume channel, and daemon/live-turn wiring are unresolved.",
      "evidence": ["8916868", "48d0351", "2d0a6fe", "worklog/APP012-RESTART-RECOVERY-VERIFY.md", "worklog/APP012-APPROVAL-RESUME-CONTRACT-VERIFY.md", "docs/CONVERGENCE.md:26-28"]
    },
    {
      "id": "B2",
      "severity": "keyring_blocking",
      "statement": "No admitted public, compilable credential persistence seam exists. K01-K10 RED cannot be authored honestly without invented imports, test-local fake success, or an explicit controller policy checkpoint.",
      "evidence": ["4dc8811", "4814357", "crates/providers/src/lib.rs:4-59", "crates/providers/src/auth_store.rs:1-7,347-431"]
    },
    {
      "id": "B3",
      "severity": "repository_guard_blocking",
      "statement": "python3 tools/convergence_gate.py is pre-existing RED with off-plan/completion findings, including AUD-017/AUD-020 unresolved acceptance notes. This artifact does not edit ledger/controller state outside its own row or claim the gate green.",
      "evidence": ["docs/CONVERGENCE.md:101-109", "observed command result: convergence_gate reported 58 findings"]
    },
    {
      "id": "B4",
      "severity": "platform_and_acceptance_blocking",
      "statement": "Cancellation evidence is Unix-scoped and not a RED; keyring native A/B/C and Windows GNU support require real target receipts; /bin/false and stale broad-lib failures are environment or old-contract evidence, not grounds to weaken frozen tests.",
      "evidence": ["0fb0707", "56f8e2b", "41f37f0", "30de11a"]
    }
  ],
  "validation": {
    "performed": [
      "Read AGENTS.md, PLAN.md, docs/TDD.md, docs/SECURITY.md, docs/CONVERGENCE.md, .agents/WORKER.md before artifact authoring.",
      "Audited requested refs c269715, 8916868, f83637d, 48d0351, 2d0a6fe, 4dc8811, 4814357, 6d2f97f, 55acf29, 6e1695d, 56f8e2b, and 0fb0707.",
      "Verified current source evidence for security broker, file open path, storage facade/execution/schema, daemon lock, approval API, live turn, onboarding, credential planner, and shell lifecycle.",
      "Preserved the installed APP012 frozen hash and all cited RED hashes; no Cargo/source/test/controller/plan edits were made."
    ],
    "read_only_commands": [
      "git rev-parse HEAD",
      "git status --short --branch",
      "git diff --check",
      "shasum -a 256 crates/server/tests/app012_tool_journey_red.rs",
      "python3 tools/convergence_gate.py",
      "python3 tools/validate_repository.py"
    ],
    "observed_limitations": [
      "crates/server/tests/app012_tool_journey_red.rs is absent from candidate revision 8a91a7b49a5a1c948218ad8f176d44e015530dcb; local shasum and focused cargo test cannot be rerun here, so the historical frozen hash is not recomputed"
    ],
    "expected_guard_results": {
      "convergence_gate": "FAIL on pre-existing repository ledger findings; not repaired in this planning lane",
      "validate_repository": "FAIL on pre-existing backlog/ownership findings; no protected path changed",
      "scope": "Only this artifact and this task's ledger row may differ from the base"
    },
    "resource_profile": "Planning and read-only validation only; no Cargo, browser, daemon, database mutation, native keyring access, credential access, or user database access.",
    "acceptance_statement": "This artifact is a convergence plan and evidence map, not an APP012 acceptance certificate."
  }
}
