# Phase 1 vertical product map: unsigned macOS

## Scope and verdict

- Task: `PHASE1-VERTICAL-PRODUCT-MACOS-MAP`
- Candidate revision: `e4bbb11dcac87cd706c3e82f199dec51ec285dd6`
- Platform scope: unsigned macOS arm64 source/package evidence; no signing,
  notarization, hosted runner, or production credential claim.
- Boundary: install -> bare `oc2`/`opencode2` -> daemon -> native TUI or web ->
  setup/auth -> provider turn -> brokered tool -> persistence -> restart ->
  second client -> uninstall.
- Verdict: **RED/BLOCKED.** Installer closure, authenticated daemon routes,
  native renderer path, several state units, and narrow read-tool fixture are
  present. The clean installed no-subcommand journey is not proven end to end.

Evidence classes:

- `main`: current intended path is wired and has direct runtime evidence.
- `verified-branch`: focused branch or package evidence proves a bounded branch,
  not the parent journey.
- `RED`: frozen or independently authored test reproduces the missing behavior.
- `blocked/external`: source-only, absent caller, platform authority, or missing
  external identity. Never acceptance.

## Journey ledger

| Surface | Observable path | Evidence | Class | Gap / failure state |
|---|---|---|---|---|
| Install | `scripts/install-oc2.sh:73-90,194-235,263-325,366-427` detects macOS, verifies checksum, rejects unsafe archive members, stages `oc2` plus `libopentui.dylib`, rolls back atomically | `worklog/TUI-011.md:223-243` records disposable Unicode-path install, matching hashes, `oc2 --version`, native PTY run | verified-branch | Unsigned only. No notarization, signing, or exact integrated release receipt |
| Upgrade | Same script backs up binary/native siblings, installs both, removes backups only after both renames: `scripts/install-oc2.sh:403-425` | Script-level identity/checksum/rollback tests recorded in `worklog/APP-010-REVISION-RECEIPT-INTEGRATION.md` | verified-branch | No installed restart/resume or upgrade-with-live-client proof |
| Uninstall | `scripts/install-oc2.sh:154-175` removes only `oc2` and platform native sibling, preserves user data, rejects symlink destinations | Existing install evidence covers binary/native cleanup and preservation semantics | verified-branch | No packaged macOS acceptance receipt; history export is only a message, not a tested export artifact |
| Bare launch | `crates/cli/src/main.rs:225-272` probes TTY, presence, credentials, plans launch, prepares daemon, calls `tui_entry::run_default`; `app_start::plan_default_launch` routes owner/attacher and setup/main: `crates/cli/src/app_start.rs:243-320` | `worklog/APP-001-REPAIR-RED.md:59-65` records 3/3 repair candidate; `worklog/TUI-011.md:245-262` records packaged bare launch still fails setup/empty-session journey | RED | Frozen `installed_default_entrypoint.rs` remains five `todo!()` bodies; fresh installed HOME renders empty/offline instead of required setup/main path |
| Daemon ownership | `crates/cli/src/chat.rs:99-137` bounded startup lock and one owner; `crates/cli/src/daemon_client.rs:745-805` validates descriptor and credentials; `crates/cli/src/main.rs:700-757` mints/publishes auth and serves authenticated router | Authenticated daemon and descriptor suites recorded in `worklog/APP-002.md` and `worklog/WEB-006-DESCRIPTOR.md` | main | Real bare-launch attach under two simultaneous fresh clients remains unproven |
| Native TUI | `crates/cli/src/tui_entry.rs:1425-1598` resolves bearer, creates first session for `StartupView::Main`, invokes `native_interactive_loop`; renderer setup/raw mode/input loop at `:948-1038`; host caller at `:958-966` | `worklog/TUI-011.md:193-243` records native bridge 73/73, parity 13/13, real PTY 1/1, installed explicit-session daemon survival | verified-branch | `native_interactive_loop` sets `skip_onboarding: !setup_mode` but provider submission still lacks full setup/provider persistence caller; piped stdin intentionally refuses at `:1563-1571` |
| Onboarding | `crates/cli/src/onboarding.rs:48-74,155-215,719-755` defines bounded setup, redaction, cancel, and credential-free state; `native_interactive_loop` enters setup when `setup_mode` is true: `tui_entry.rs:1471-1494` | `worklog/APP-005.md` records setup state tests; `worklog/APP-012.md:104-107` says `account_setup` is pure and caller persistence is absent | RED | `daemon_client::creds_configured` only checks non-empty env vars: `daemon_client.rs:775-790`; no production Keychain-backed caller, no secure persistent setup proof |
| Auth | `crates/server/src/daemon_auth.rs:31-80,150-175` mints/verifies 64-hex bearer and gates `/api/*`; `crates/server/src/lib.rs:271-334` applies middleware while `/health` remains public | Auth middleware, descriptor, and API route tests in current server suite/worklogs | main | Web client has no demonstrated authenticated browser journey; token discovery/renewal after restart lacks installed E2E proof |
| Provider | `crates/server/src/lib.rs:920-968` accepts only `openai`, builds `OpenAiResponsesClient` from env, appends user and assistant messages; `crates/cli/src/daemon_client.rs:775-790` recognizes four env keys | `worklog/E2E-APP.md` and `worklog/APP-012.md:89-100` identify OpenAI-only real fixture and narrow read journey | verified-branch | Other providers/auth flows are not real callers; setup does not persist an authorized account; absent credentials route is not packaged-green |
| Web | `crates/server/src/lib.rs:275-325` registers API routes and embedded SPA fallback; `:326-334` applies auth; `crates/server/src/web_assets.rs:22-71` serves or fails closed on missing bundle | `web_assets` real bundle tests; authenticated `/api/*` route evidence | verified-branch | Capability report explicitly marks approvals/plugins/attachments execution unavailable: `lib.rs:339-407`; no authenticated embedded web client turn/approval proof |
| Prompt/stream | `crates/server/src/lib.rs:1126-1255` creates provider stream, bounded turn permit, history, broker, loop controller; `:1276-1565` emits deltas, tool calls, assistant result; `:1682-...` persists tool outputs and next round | `worklog/APP-012-TOOL-RED.md` and integrated read-tool receipt `945236c4...` prove one loopback provider round and durable rows | verified-branch | Parent stream does not expose approval resume; provider stream error/interrupt/restart lacks installed journey receipt |
| Tools/broker | `crates/server/src/lib.rs:1388-1437,1607-1645` calls `PermissionBroker`; allow/deny/human decisions are handled; `crates/tools/src/executor.rs:136-237` caps `read` at 64 KiB and uses `spawn_blocking`; `file_ops.rs:176-208` authorizes concrete paths | Read tool integrated 1/1; `worklog/APP-012.md:89-100` records exact second-round output and durable tool rows | RED | `RequireHuman` becomes terminal text at `lib.rs:1429-1437,1639-1644`; no approval record/channel/resume caller. Unknown tool still fails in generic executor for unregistered names, as recorded `APP-012.md:108-118` |
| Approval/denial | State machine provides `AwaitingApproval`, `deny_approval`, bounded receipt, no side effect: `crates/server/src/turn_service.rs:65-81,303-357` | Unit state tests cover denial and no-side-effect semantics | RED | Live stream bypasses state machine for human approval, returns error text, does not hold/reconnect a pending turn |
| Restart/recovery | `turn_service::Turn::mark_uncertain` and ambiguous retry are truthful: `turn_service.rs:423-465`; daemon descriptor is atomic and authenticated | State-only tests plus descriptor restart checks | RED | Durable in-flight turn rehydration, approval resume, and installed daemon restart/resume are absent. No automatic replay permitted |
| Persistence | `SessionService` is shared by routes and engine; `main.rs:541-555` opens storage under data dir; TUI snapshot creates first session at `tui_entry.rs:462-481` | `worklog/APP-006-ws.md`, `APP-012.md:93-99`, and read-tool durable `user/tool/assistant` rows | verified-branch | Bare fresh HOME, restart/rebuild, and exact transcript/tool recovery through packaged binary remain unproven |
| Multi-client | `crates/server/src/app_runtime.rs:609-636,682-745` models one daemon-owned engine, shared sessions, bounded permits/events; `chat.rs:99-137` coordinates singleton startup | Engine unit assertions and sequential daemon survival in `TUI-011.md:239-243` | RED | No simultaneous second client observing one live turn and approval without duplicate work; no exact installed two-client receipt |
| Terminal shutdown | `crates/cli/src/shutdown.rs` and app-start guards own restoration; TUI raw mode guard is entered at `tui_entry.rs:969-972` | `worklog/APP-009.md`; packaged PTY restoration in `TUI-011.md:234-238` | verified-branch | macOS suspend/resize/mouse/clipboard/IME coverage and exact bare-app failure cleanup remain platform/E2E gaps |

## Current macOS journey classification

```json
{
  "revision": "e4bbb11dcac87cd706c3e82f199dec51ec285dd6",
  "platform": "macos-arm64",
  "artifact": "unsigned",
  "journey": {
    "install": "verified-branch",
    "upgrade": "verified-branch",
    "uninstall": "verified-branch",
    "bare_no_subcommand": "RED",
    "daemon_auth_singleton": "main",
    "native_tui_explicit_session": "verified-branch",
    "native_tui_fresh_home": "RED",
    "onboarding_state": "verified-branch",
    "onboarding_persistent_secure_caller": "blocked/external",
    "openai_fixture_turn": "verified-branch",
    "other_provider_turns": "blocked",
    "web_bundle_and_api_auth": "verified-branch",
    "web_turn_approval": "blocked",
    "brokered_read_tool": "verified-branch",
    "human_approval_resume": "RED",
    "restart_resume": "RED",
    "multi_client_live_observation": "RED",
    "terminal_restore": "verified-branch",
    "signing_notarization": "blocked/external"
  },
  "acceptance": "not_claimed",
  "parent_boundary": "blocked until installed bare journey passes on exact integrated revision"
}
```

## Wave proposals

Each proposal separates RED author, implementation owner, verifier, and
serialized integration. Product files below are proposed ownership, not edits in
this map. Each RED must compile and fail before implementation, then be frozen.

### Wave 1: bare installed macOS spine

```json
[
  {
    "id": "MACOS-ENTRY-001",
    "wave": 1,
    "outcome": "Fresh disposable HOME plus installed unsigned macOS arm64 oc2, invoked with no subcommand, opens the planned native setup/main view, discovers or starts one authenticated daemon, atomically creates the first session, and leaves no child or descriptor residue on startup failure.",
    "real_caller": "crates/cli/src/main.rs::run no-subcommand arm -> chat::prepare_daemon -> tui_entry::run_default",
    "red_test_owner": "tests/e2e/installed_default_entrypoint.rs (independent test-author; repair frozen todo contract before freeze)",
    "implementation_owner": "crates/cli/src/main.rs (serialized entrypoint integration fragment; no parallel edits)",
    "verifier": "independent verifier reruns frozen macOS PTY test against installed artifact and exact revision",
    "integration_owner": "serialized CLI/integrator lane; rerun app_start, daemon_client, native PTY, installer suites",
    "dependencies_refs": ["APP-001", "APP-002", "TUI-003", "TUI-011", "APP-010", "APP-012", "crates/cli/src/app_start.rs:287-320", "crates/cli/src/tui_entry.rs:1432-1513", "worklog/TUI-011.md:245-262"],
    "security_resource_failures": ["reject redirected stdin for interactive setup", "reject forged/stale descriptor and mismatched bearer", "bound PTY capture and startup timeout", "one startup lock and one daemon owner", "restore terminal and remove temp descriptor/child on failure", "never log credentials"],
    "commands_evidence": ["CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 300 cargo test -p opencode-rk-cli --features native --test installed_default_entrypoint -- --test-threads=1", "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 300 cargo test -p opencode-rk-cli --bin opencode-rk app_start daemon_client", "sh -n scripts/install-oc2.sh", "install archive into disposable HOME, run installed oc2 without Cargo/Zig/Bun/Node"],
    "residuals": ["signing/notarization external", "real OS Keychain persistence remains Wave 2", "frozen parent test cannot be silently edited by implementation lane"]
  },
  {
    "id": "MACOS-SHUTDOWN-002",
    "wave": 1,
    "outcome": "Bare installed native TUI exits, interrupts, and handles daemon/client ownership without corrupting macOS terminal state or killing a daemon used by another client.",
    "real_caller": "crates/cli/src/tui_entry.rs::native_interactive_loop plus crates/cli/src/shutdown.rs guards",
    "red_test_owner": "crates/cli/tests/macos_installed_lifecycle.rs",
    "implementation_owner": "crates/cli/src/tui_entry.rs (single-file lifecycle fragment; serialized with entrypoint)",
    "verifier": "independent macOS PTY verifier checks escape restoration, child reaping, daemon PID survival, and bounded exit",
    "integration_owner": "CLI integration lane",
    "dependencies_refs": ["APP-009", "TUI-011", "crates/cli/src/tui_entry.rs:948-1038", "worklog/TUI-011.md:234-243"],
    "security_resource_failures": ["no detached owned child", "no unbounded PTY output", "SIGINT/SIGTERM and provider interruption reclaim owned work", "second client prevents daemon termination", "resize/mouse/clipboard/IME unsupported cases fail explicitly"],
    "commands_evidence": ["CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 300 cargo test -p opencode-rk-cli --features native --test macos_installed_lifecycle -- --test-threads=1", "run installed oc2 under disposable PTY with bounded transcript and process-tree receipt"],
    "residuals": ["platform-specific suspend/IME behavior needs actual macOS PTY evidence", "no signing claim"]
  }
]
```

### Wave 2: one durable provider/tool turn

```json
[
  {
    "id": "MACOS-SETUP-003",
    "wave": 2,
    "outcome": "Missing credentials open in-app setup; successful fixture credentials persist through an authorized OS credential-store caller; cancel and invalid input leave no account or secret residue; restart reuses the account without env-only detection.",
    "real_caller": "crates/cli/src/tui_entry.rs::run_with_startup setup branch -> onboarding state -> provider account persistence",
    "red_test_owner": "crates/cli/tests/macos_onboarding_persistence.rs",
    "implementation_owner": "crates/providers/src/auth_store.rs (serialized secure-store adapter fragment; real backend capability review required)",
    "verifier": "independent verifier uses only mock credential backend locally, then a disposable macOS Keychain profile for platform proof; checks logs/transcript/export redaction",
    "integration_owner": "provider/CLI serialized lane; no live user keychain mutation during local loop",
    "dependencies_refs": ["APP-005", "APP-012", "crates/cli/src/onboarding.rs:155-215,719-755", "crates/cli/src/daemon_client.rs:775-790", "crates/providers/src/account_setup.rs:137-190", "crates/providers/src/auth_store.rs:62-124", "worklog/APP-012.md:70-87,104-107"],
    "security_resource_failures": ["secret bytes never enter diagnostics, transcript, shell args, or task artifacts", "explicit human consent for import", "owner-scoped keychain service/account", "bounded provider/model/secret sizes", "cancel is no-residue", "offline cache does not imply hosted login"],
    "commands_evidence": ["CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 300 cargo test -p opencode-rk-cli --bin opencode-rk onboarding", "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 300 cargo test -p opencode-rk-providers --test auth_store -- --test-threads=1", "macOS disposable Keychain PTY test under independent verifier"],
    "residuals": ["OAuth/provider credentials and signing identities are external", "do not claim secure persistence from pure state tests"]
  },
  {
    "id": "MACOS-TURN-004",
    "wave": 2,
    "outcome": "The installed TUI submits through the daemon-owned provider engine; a deterministic provider requests one bounded file operation; the real broker authorizes or denies it; result streams back, persists, and is visible to another client.",
    "real_caller": "crates/cli/src/tui_entry.rs native composer -> authenticated `/api/sessions/{id}/turns/stream` -> crates/server/src/lib.rs::create_turn_stream",
    "red_test_owner": "tests/e2e/macos_installed_turn.rs",
    "implementation_owner": "crates/server/src/lib.rs (serialized turn integration fragment; central route/stream owner)",
    "verifier": "independent verifier runs loopback scripted provider, real filesystem fixture, real PermissionBroker, real SessionService, and checks no side effect on denial",
    "integration_owner": "server/CLI spine integrator; serialize lib.rs changes and rerun tool, stream, auth, and persistence suites",
    "dependencies_refs": ["APP-003", "APP-004", "APP-012", "crates/server/src/lib.rs:1126-1255,1276-1565,1572-1689", "crates/tools/src/executor.rs:136-237", "crates/tools/src/file_ops.rs:176-208", "worklog/APP-012.md:89-122"],
    "security_resource_failures": ["allowlist tool IDs, deny by default", "authorize concrete path before I/O", "64 KiB read cap and bounded output", "bounded turn permits and step/call limits", "no shell-string concatenation", "provider partial stream does not replay ambiguous effects", "denial asserts fixture unchanged"],
    "commands_evidence": ["CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 300 cargo test -p opencode-rk-server --test app012_tool_journey_red -- --test-threads=1", "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 300 cargo test -p opencode-rk-tools --lib executor file_ops -- --test-threads=1", "run installed macOS PTY E2E with disposable DB/HOME and provider fixture"],
    "residuals": ["current `RequireHuman` path is terminal error, not resume", "provider support remains OpenAI-only until a real adapter exists", "web capability remains unavailable for approvals"]
  }
]
```

### Wave 3: approval, recovery, multi-client closure

```json
[
  {
    "id": "MACOS-APPROVAL-005",
    "wave": 3,
    "outcome": "A human-gated tool enters durable AwaitingApproval, a second authenticated client observes the same turn, deny leaves the fixture unchanged, approve resumes the same turn exactly once, and disconnect does not duplicate the effect.",
    "real_caller": "crates/server/src/lib.rs::create_turn_stream broker decision -> approval route/event -> turn_service state transition",
    "red_test_owner": "crates/server/tests/macos_approval_resume.rs",
    "implementation_owner": "crates/server/src/turn_service.rs (state/approval fragment), with route registration as a serialized integration fragment owned by integrator",
    "verifier": "independent verifier injects fake bounded grant issuer and two authenticated clients; verifies durable approval receipt, one effect, denial absence, cancellation cleanup",
    "integration_owner": "serialized server route/state integration lane; no direct ToolExecutor bypass",
    "dependencies_refs": ["APP-004", "APP-007", "APP-012", "crates/server/src/turn_service.rs:303-465", "crates/server/src/lib.rs:1390-1437,1607-1645", "crates/server/src/app_runtime.rs:682-745", "docs/SECURITY.md:62-72"],
    "security_resource_failures": ["human grant carries operation/path/identity/expiry/policy version", "wildcard permission cannot bypass human gate", "deny has zero filesystem/process side effects", "approval queue bounded and owned", "ambiguous disconnect never auto-replays", "approval records redact secrets"],
    "commands_evidence": ["CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 300 cargo test -p opencode-rk-server --test macos_approval_resume -- --test-threads=1", "targeted turn_service tests for AwaitingApproval/deny/interrupt/uncertain", "two-client installed PTY run with exact event and process receipts"],
    "residuals": ["real Keychain and production provider authority remain external", "remote web approval is separate scope unless the same daemon route is proven"]
  },
  {
    "id": "MACOS-RECOVERY-006",
    "wave": 3,
    "outcome": "Daemon restart during streaming, approval, and possible tool effect reopens the durable session with truthful settled/cancelled/uncertain state; the user can reconcile or resume only when safe; a second client observes one shared turn ID.",
    "real_caller": "daemon lifecycle in `crates/cli/src/main.rs` plus shared `SessionService`, `EngineHandles`, `turn_service::Turn::mark_uncertain`, and authenticated reconnect",
    "red_test_owner": "tests/e2e/macos_restart_multiclient.rs",
    "implementation_owner": "crates/server/src/app_runtime.rs (serialized daemon-owned lifecycle fragment; storage/schema changes require separate integration review)",
    "verifier": "independent verifier kills/restarts only disposable daemon, reconnects two clients, checks durable rows, turn IDs, child/FD cleanup, and no replay",
    "integration_owner": "daemon/server/CLI integration spine; exact integrated revision rerun required",
    "dependencies_refs": ["APP-002", "APP-003", "APP-007", "APP-009", "APP-012", "crates/server/src/app_runtime.rs:609-797", "crates/server/src/turn_service.rs:423-465", "crates/cli/src/daemon_client.rs:159-213,745-805", "docs/CONVERGENCE.md:17-24"],
    "security_resource_failures": ["never mark an in-flight possible effect Settled", "no blind replay after timeout/crash", "bounded event replay and history windows", "reclaim owned tasks/processes/permits", "descriptor/token rotation follows explicit policy", "foreign port/PID is never killed"],
    "commands_evidence": ["CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 300 cargo test -p opencode-rk-server --lib app_runtime turn_service -- --test-threads=1", "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 300 cargo test -p opencode-rk-cli --bin opencode-rk daemon_client shutdown", "run installed two-client restart/resume PTY with disposable storage and process-tree receipt"],
    "residuals": ["full convergence remains blocked until installed APP-012 passes", "hosted cross-platform and signed release evidence outside macOS map"]
  }
]
```

## Verification receipts and blockers

- `python3 tools/convergence_gate.py`: **blocked**, pre-existing repository-wide
  ledger/accounting findings. This is not product acceptance and no controller
  state was changed.
- `worklog/TUI-011.md:223-243`: strongest macOS receipt: unsigned arm64
  archive/install, native PTY, daemon survival. `:245-265` explicitly keeps the
  bare installed parent RED.
- `worklog/APP-012.md:89-135`: narrow authenticated read tool is integrated;
  approval/resume, persistent provider caller, Windows installer, and full
  installed proof remain open.
- `crates/server/src/lib.rs:373-407`: capability response is honest and marks
  web approvals/plugins/attachments execution unavailable.
- `crates/server/src/lib.rs:1429-1437,1639-1644`: live `RequireHuman` behavior
  is a terminal error, not an approval continuation.
- `crates/providers/src/account_setup.rs:1-8`: pure setup boundary explicitly
  delegates persistence/transport to caller; no secure-store caller is present.
- `scripts/install-oc2.sh:194-235,263-325,366-427`: unsigned package safety and
  native closure are bounded, but source/script evidence is not notarization.
- External blockers: macOS signing/notarization identities, real provider/OAuth
  authority, independent frozen-test repair approval, and exact integrated
  installed APP-012 rerun.

## Residual contract

Do not call the application parent complete from this map. Completion requires
the exact `docs/CONVERGENCE.md:10-24` journey: clean installed no-subcommand
launch, setup, real stream/tool/broker, persistence, restart/resume, second
client, denial/interruption, terminal restoration, and frozen rerun on the
integrated revision. Static modules, mocked provider success, explicit-session
PTY proof, or unsigned package hashes cannot substitute.
