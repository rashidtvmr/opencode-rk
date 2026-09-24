# PHASE1-WAVE1-DISPATCH-MAP

Compact Wave 1 dispatch table. Derived exclusively from landed evidence. No new research.

## 1. Immutable Evidence References

| Report | Revision | Branch | Path |
|---|---|---|---|
| Source evidence matrix | 29ec0b3 | plan/phase1-wave1-dispatch | worklog/PHASE1-WAVE1-SOURCE-EVIDENCE.md |
| Shell RED verifier | 8c514e0 | verify/TOOL-RED-SHELL | (shell verifier report) |
| Security verifier | 632a71e | plan/security-acceptance | (security verifier report) |

Base source revision: 5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b.

## 2. Status Snapshot

| State | Count | Notes |
|---|---|---|
| PROVEN (launchable) | 4 | Verification-only or focused RED; no implementation lanes |
| BLOCKED | 7 | Missing caller, backend, platform, or authorized split |
| COMPLETED | 0 | No parent acceptance achieved |
| QUEUED | 7 | Same rows as BLOCKED; awaiting prerequisite resolution |

Gate status: CONVERGENCE BLOCKED, total=90. Current gate blocked at 91.

## 3. Launch-Next Table (PROVEN and contract-ready only)

Breadth cap: 4 integration + 2 verifier reserved = 6 slots held. Max 14 breadth lanes.
Shell implementation is NOT launchable per ACCEPT WITH SPLIT verdict.

| # | Lane | Type | Owned file | Package/target | Command | Resource | Evidence ref |
|---|---|---|---|---|---|---|---|
| 1 | Tools shell broker RED | RED authoring | crates/tools/tests/phase1_shell_broker.rs (new) | opencode-rk-tools / phase1_shell_broker | CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 cargo test -p opencode-rk-tools --test phase1_shell_broker -- --test-threads=1 | Medium | SOURCE-EVIDENCE row 1 |
| 2 | Installed entrypoint verify | verification | crates/cli/tests/installed_default_entrypoint.rs (read-only) | opencode-rk-cli / installed_default_entrypoint | env OC2_BUILD_REVISION=5d66683... CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 cargo test -p opencode-rk-cli --features native --test installed_default_entrypoint -- --test-threads=1 | High | SOURCE-EVIDENCE row 4 |
| 3 | APP-012 journey verify | verification | crates/server/tests/app012_tool_journey_red.rs (read-only) | opencode-rk-server / app012_tool_journey_red | CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 cargo test -p opencode-rk-server --test app012_tool_journey_red -- --test-threads=1 | Medium-High | SOURCE-EVIDENCE row 5 |
| 4 | Native artifact manifest verify | verification | crates/opentui-bridge/tests/native_artifact_manifest.rs (read-only) | opencode-rk-opentui-bridge / native_artifact_manifest | CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 cargo test -p opencode-rk-opentui-bridge --test native_artifact_manifest -- --test-threads=1 | Low-Medium | SOURCE-EVIDENCE row 6 |

Breadth used: 4 of 14 max. Integration slots reserved: 4. Verifier slots reserved: 2.

## 4. Queued/Blocker Table

| Lane | Blocker | Prerequisite | Evidence ref |
|---|---|---|---|
| OS isolation RED | SEC-RED-OS active; engage() always returns Blocked; no real backend | Approved OS backend owner and dependency | SOURCE-EVIDENCE row 2 |
| Receipt binding / archive owner | No archive producer exists; scripts are extractors only | Integration proposal for release packaging path | SOURCE-EVIDENCE row 3 |
| Keyring persistence | No keyring dependency in Cargo.toml; no safe local authority | Controller authorization + audited mock credential builder | SOURCE-EVIDENCE row 7 |
| Approval/resume | RequireHuman is terminal error; no approval-resume channel wired | Real caller wiring in lib.rs:1388-1438 | SOURCE-EVIDENCE row 8 |
| Restart/resume | Storage baseline proven; daemon restart not wired | APP-012 live wiring | SOURCE-EVIDENCE row 9 |
| Protected-path denial | No frozen RED for .env/outside-root zero-I/O proof | Separate frozen RED per APP-012 worklog | SOURCE-EVIDENCE row 10 |
| Windows-GNU closure | No Windows CI runner; no Rust Windows target installed locally | Authorized Windows-GNU CI + build.rs verification | SOURCE-EVIDENCE row 11 |

## 5. One-File Collision Matrix

No two launchable lanes share an owned file. Verification lanes (rows 2-4) are read-only against existing frozen tests. Row 1 creates a new file in crates/tools/tests/ which does not collide with existing shell_tool_process_tree.rs.

| File | Lanes | Collision risk |
|---|---|---|
| crates/tools/tests/phase1_shell_broker.rs | Lane 1 only | None (new file) |
| crates/cli/tests/installed_default_entrypoint.rs | Lane 2 only (read-only) | None (frozen) |
| crates/server/tests/app012_tool_journey_red.rs | Lane 3 only (read-only) | None (frozen) |
| crates/opentui-bridge/tests/native_artifact_manifest.rs | Lane 4 only (read-only) | None (frozen) |
| tasks/completion/claims.json | All (ledger) | Managed by cc.claim fail-closed |

## 6. TDD/Integration Order

1. Lane 1 (shell broker RED): Author compiling RED first. Freeze hash. Do NOT implement. Implementation blocked pending shell split/refreeze authorization.
2. Lanes 2-4 (verification): Run frozen tests to confirm current state. These are read-only verifiers, not implementation.
3. Integration spine: After lane 1 RED is frozen, orchestrator runs convergence_gate.py to check if gate advances from 91.
4. No parent task may close until APP-012 full journey passes on integrated revision (restart, approval, protected paths all wired).

## 7. One-Heavy-Command Resource Schedule

Run one heavy command at a time. Budget: 8 GiB total, keep 2 GiB free.

| Priority | Command | Est. memory | Timeout | Parallelism |
|---|---|---|---|---|
| 1 | Lane 2: cli native test (PTY + daemon) | ~1.5 GiB | 180s | CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 |
| 2 | Lane 3: server journey test | ~800 MiB | 180s | CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 |
| 3 | Lane 1: tools RED compile | ~600 MiB | 180s | CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 |
| 4 | Lane 4: bridge manifest test | ~200 MiB | 180s | CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 |

Never run lanes 1-3 concurrently. Lane 4 may follow immediately after any single lane completes. Record `rtk free -h` before each. Stop if available < 1 GiB.

## 8. Platform Evidence Boundaries

- Lane 1: Unix process fixtures only (/bin); ShellTool API cross-platform.
- Lane 2: macOS only (cfg target_os = "macos"); requires PTY /usr/bin/script + aarch64 native bridge.
- Lane 3: Cross-platform (Tokio loopback + disposable SQLite).
- Lane 4: Runs on any host with checked-in files; validates macOS arm64 artifact contract only.
- OS isolation (blocked): Real Landlock Linux only; macOS/Windows backends absent.
- Windows-GNU (blocked): Requires Windows CI; PE pair verifier can run on macOS.

## 9. Parent-Open Warning

APP-012 parent task remains OPEN. Frozen child verifications (lanes 2-4) do not close it. Missing: restart/resume, approval/resume, protected-path denial, installed packaging, second-client durability. A wave containing only isolated verification modules is not a successful application-building wave. Convergence gate must advance before declaring parent complete.

SEC-RED-OS is active. OS isolation lane must not be dispatched until security acceptance branch resolves the backend contract.
