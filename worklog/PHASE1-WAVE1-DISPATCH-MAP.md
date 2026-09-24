# PHASE1-WAVE1-DISPATCH-MAP

Compact Wave 1 dispatch table. Derived exclusively from landed evidence. No new research.
Corrected per verifier `0bba574` (ACCEPT WITH CORRECTIONS).

## 1. Immutable Evidence References

| Report | Revision | Branch | Path |
|---|---|---|---|
| Source evidence matrix | 29ec0b3 | plan/phase1-wave1-dispatch | worklog/PHASE1-WAVE1-SOURCE-EVIDENCE.md |
| Shell RED verifier | 8c514e0 | verify/TOOL-RED-SHELL | (shell verifier report) |
| Security verifier | 632a71e | plan/security-acceptance | (security verifier report) |
| Manifest verifier | 0bba574 | plan/phase1-wave1-dispatch | worklog/PHASE1-WAVE1-DISPATCH-MAP-VERIFY.md |

Base source revision: 5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b.

Shell RED artifact: already authored and verified on `origin/lane/TOOL-RED-SHELL` (`2cd9ca9`, hash `2853960b7ad711d88384b136db5b37eca1dd827e82c4339990c120267a4e11ed`, verdict ACCEPT WITH SPLIT). Not launch-now; do not re-author; shell implementation stays blocked.

## 2. Status Snapshot

| State | Count | Notes |
|---|---|---|
| PROVEN (launchable) | 3 | Verification-only frozen reads; no implementation lanes |
| BLOCKED | 7 | Missing caller, backend, platform, or authorized split |
| COMPLETED | 0 | No parent acceptance achieved |
| QUEUED | 7 | Same rows as BLOCKED; awaiting prerequisite resolution |

Gate status: CONVERGENCE BLOCKED, total=93 (anchored observation at this revision; verifier observed total=92 at 0bba574; counts drift with ledger).

## 3. Launch-Next Table (PROVEN and contract-ready only)

Breadth cap: 4 integration + 2 verifier reserved = 6 slots held. Max 14 breadth lanes.
Shell implementation is NOT launchable per ACCEPT WITH SPLIT verdict.
Shell RED is already authored and verified (see section 1); no RED-authoring lane is dispatched.
No wrapper binary exists on this macOS host; commands below carry no wrapper.

| # | Lane | Type | Owned file | Package/target | Command | Resource | Evidence ref |
|---|---|---|---|---|---|---|---|
| 1 | Installed entrypoint verify | verification | crates/cli/tests/installed_default_entrypoint.rs (read-only) | opencode-rk-cli / installed_default_entrypoint | env OC2_BUILD_REVISION=5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --features native --test installed_default_entrypoint -- --test-threads=1 | High | SOURCE-EVIDENCE row 4 |
| 2 | APP-012 journey verify | verification | crates/server/tests/app012_tool_journey_red.rs (read-only) | opencode-rk-server / app012_tool_journey_red | CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_tool_journey_red -- --test-threads=1 | Medium-High | SOURCE-EVIDENCE row 5 |
| 3 | Native artifact manifest verify | verification | crates/opentui-bridge/tests/native_artifact_manifest.rs (read-only) | opencode-rk-opentui-bridge / native_artifact_manifest | CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-opentui-bridge --test native_artifact_manifest -- --test-threads=1 | Low-Medium | SOURCE-EVIDENCE row 6 |

Breadth used: 3 of 14 max. Integration slots reserved: 4. Verifier slots reserved: 2.

## 4. Queued/Blocker Table

| Lane | Blocker | Prerequisite | Evidence ref |
|---|---|---|---|
| OS isolation RED | SEC-RED-OS blocked per bd8ddc9; engage() always returns Blocked; no real backend; requires authorized Linux runner plus linked syscall backend contract | Approved OS backend owner and dependency | SOURCE-EVIDENCE row 2 |
| Receipt binding / archive owner | No archive producer exists; scripts are extractors only | Integration proposal for release packaging path | SOURCE-EVIDENCE row 3 |
| Keyring persistence | No keyring dependency in Cargo.toml; no safe local authority | Controller authorization + audited mock credential builder | SOURCE-EVIDENCE row 7 |
| Approval/resume | RequireHuman is terminal error; no approval-resume channel wired | Real caller wiring in lib.rs:1388-1438 | SOURCE-EVIDENCE row 8 |
| Restart/resume | Storage baseline proven; daemon restart not wired | APP-012 live wiring | SOURCE-EVIDENCE row 9 |
| Protected-path denial | No frozen RED for .env/outside-root zero-I/O proof | Separate frozen RED per APP-012 worklog | SOURCE-EVIDENCE row 10 |
| Windows-GNU closure | No Windows CI runner; no Rust Windows target installed locally | Authorized Windows-GNU CI + build.rs verification | SOURCE-EVIDENCE row 11 |

## 5. One-File Collision Matrix

No two launchable lanes share an owned file. Verification lanes (rows 1-3) are read-only against existing frozen tests.

| File | Lanes | Collision risk |
|---|---|---|
| crates/cli/tests/installed_default_entrypoint.rs | Lane 1 only (read-only) | None (frozen) |
| crates/server/tests/app012_tool_journey_red.rs | Lane 2 only (read-only) | None (frozen) |
| crates/opentui-bridge/tests/native_artifact_manifest.rs | Lane 3 only (read-only) | None (frozen) |
| tasks/completion/claims.json | All (ledger) | Managed by cc.claim fail-closed |

## 6. TDD/Integration Order

1. Shell broker RED: already authored and frozen on `origin/lane/TOOL-RED-SHELL` (`2cd9ca9`). Do NOT re-author. Do NOT implement. Implementation blocked pending shell split/refreeze authorization.
2. Lanes 1-3 (verification): Run frozen tests to confirm current state. These are read-only verifiers, not implementation.
3. Integration spine: After verification lanes confirm state, orchestrator runs convergence_gate.py to check gate movement from the anchored observation.
4. No parent task may close until APP-012 full journey passes on integrated revision (restart, approval, protected paths all wired).

## 7. One-Heavy-Command Resource Schedule

Run one heavy command at a time. Budget: 8 GiB total, keep 2 GiB available.

| Priority | Command | Est. memory | Bound | Parallelism |
|---|---|---|---|---|
| 1 | Lane 1: cli native test (PTY + daemon) | ~1.5 GiB | 180s serial | CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 |
| 2 | Lane 2: server journey test | ~800 MiB | 180s serial | CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 |
| 3 | Lane 3: bridge manifest test | ~200 MiB | 180s serial | CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 |

Never run lanes 1-2 concurrently. Lane 3 may follow immediately after any single lane completes. Record `vm_stat` before each. Stop if available < 1 GiB.

## 8. Platform Evidence Boundaries

- Lane 1: macOS only (cfg target_os = "macos"); requires PTY /usr/bin/script + aarch64 native bridge.
- Lane 2: Cross-platform (Tokio loopback + disposable SQLite).
- Lane 3: Runs on any host with checked-in files; validates macOS arm64 artifact contract only.
- OS isolation (blocked): Real Landlock Linux only; macOS/Windows backends absent.
- Windows-GNU (blocked): Requires Windows CI; PE pair verifier can run on macOS.
- Windows MSVC is out of scope. GNU pair only; do not relabel GNU artifacts as MSVC.

## 9. Parent-Open Warning

APP-012 parent task remains OPEN. Frozen child verifications (lanes 1-3) do not close it. Missing: restart/resume, approval/resume, protected-path denial, installed packaging, second-client durability. A wave containing only isolated verification modules is not a successful application-building wave. Convergence gate must advance before declaring parent complete.

SEC-RED-OS is blocked per bd8ddc9. OS isolation lane must not be dispatched until an authorized Linux runner plus linked syscall backend contract resolves the backend requirement.
