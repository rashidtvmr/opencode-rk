# COMPLETION orchestrator scratchpad (compact-recovery)

## Goal
Execute commit 9cc0a34 scope: 90 slices + 450 scenarios (AUD/APP/TUI/PAR/NET/MOB/COORD/SHIP). Union legacy ralph.json (258 accepted, not evidence) + ralph.completion.json. No acceptance claimed until integrated-revision proof.

## Base
- Base commit: 9cc0a34 (18 files, +1532). HEAD at fan-out: 5af7884 (AUD-001..020 reports).
- Pins: opencode 95daf906, dev-observed 5a833585, opentui fork c01292fd, audit 165b076. Upstream lock vendored:false, fullCheckout:false.
- Gates: `python3 tools/completion_plan.py --check` = SPEC OK 90/258/450. `tests/completion` = 38 OK. `validate_repository.py` = FAIL backlog exhaustion (expected).

## Wave1: 18 lanes (one owned file each)
| lane | session | owned file | state |
|---|---|---|---|
| COORD-001 doc | ses_f4cacf8a9ffeW6IkAf4bW0GxxW | docs/ADAPTER_PROTOCOL.md | DONE 244+/126- |
| COORD-001 adapter | ses_f4cacf8a7ffeQA300WMBp9UcHi | tools/harness_adapter.py | DONE 320L, 38 tests OK |
| COORD-003 | ses_f4cacf8a6ffeMbVi2X3Nm1IBAr | tools/completion_ownership.py | DONE 178L self-check OK |
| COORD-004 | ses_f4cacf8a4ffeIlxa7I01cOMe6B | tools/completion_verification.py | DONE 207L, edge-probe OK |
| COORD-005 | ses_f4cacf8a3ffeqtjcpy5nGP9J7V | tools/completion_integration.py | EMPTY/MISSING -> retry |
| COORD-006 | ses_f4cacf8a2ffeB78QxLpsCV1LP3 | tools/completion_leases.py | EMPTY/MISSING -> retry |
| COORD-007 | ses_f4cacf8a0ffeZMkvIULTyLP6Lt | tools/completion_budgets.py | DONE 516L import OK |
| PAR-001 evidence | ses_f4cacf89fffeROw7K1dB7zXun2 | sources/completion/legacy-evidence.json | DONE 258 tasks |
| PAR-001 discovered | ses_f4cacf89dffe8GkMxc5zqpZz2B | tasks/completion/discovered.json | DONE 19 DISC-101..119, not in includes yet |
| APP-002 | ses_f4cacf89bffeBBxrespx5Kzu3u | crates/cli/src/daemon_client.rs | MISSING -> retry |
| APP-001 | ses_f4cacf86cffexR6GEK11Pbepl6 | crates/cli/src/app_start.rs | DONE 352L, 8 tests pass |
| APP-003 | ses_f4cacf869ffeKJ8MWthAWmpwGH | crates/server/src/app_runtime.rs | DONE 788L, cargo check+10 tests pass |
| APP-004 | ses_f4cacf865ffeSxusR34vkQ4xTb | crates/server/src/turn_service.rs | MISSING -> retry |
| SEC app-policy | ses_f4cacf861ffeSyhWIT2yj7yjKd | crates/security/src/app_policy.rs | DONE ~580L, 132 lib tests pass (mod line restored-out, rewiring needed) |
| TOOLS services | ses_f4cacf823ffeXrRlfMEI2VMwX3 | crates/tools/src/app_services.rs | DONE ~450L, 11 tests pass |
| SYNC protocols | ses_f4cacf821ffe7MO1MPYLbjaPFX | crates/server/src/app_protocols.rs | DONE 794L, 16 tests pass |
| TUI host | ses_f4cacf81fffeuJfM0wCWWmFchM | crates/cli/src/terminal_host.rs | DONE 300L, 6 tests pass |
| PAR-010 | ses_f4cacf81dffezDYp97DAuvycMo | sources/completion/surface-evidence.json | DONE 32 surfaces, certified false |

## Wave1 retries: 4 (running)
- COORD-005 retry ses_f4ca466e1ffejDPmV4bPdjdBz4 -> tools/completion_integration.py DONE 614L self-check OK
- COORD-006 retry ses_f4ca466ddffeU8OlvJIZ9eddYv -> tools/completion_leases.py DONE 575L self-check OK
- APP-002 retry ses_f4ca466daffe4Sx61FsydEaxIB -> crates/cli/src/daemon_client.rs DONE 18KB, 13 tests pass
- APP-004 retry ses_f4ca466cdffeI52JqHUbGWTCgb -> crates/server/src/turn_service.rs DONE 25KB, 7 tests pass

## Wave2: 16 (running, one file each)
- APP-005 onboarding ses_f4ca3a4d8ffeeEAV24xEC8H6mE -> crates/cli/src/onboarding.rs DONE 872L, 8 tests pass
- APP-006 membership ses_f4ca3a4d5ffeeRHUyQxF4QvJQc -> crates/sessions/src/session_membership.rs DONE 526L, 7 tests pass
- APP-007 cursor ses_f4ca3a4d2ffeIjY5FK2NIVPTqJ -> crates/server/src/event_cursor.rs DONE 315L, 5 tests pass
- APP-009 svc ses_f4ca3a4d0ffeBJ4mDtDY1Hibbv -> crates/cli/src/service_commands.rs DONE 526L, 9 tests pass
- APP-009 shutdown ses_f4ca3a4ceffeSMa4ru5IiYSZJa -> crates/cli/src/shutdown.rs DONE 282L, 6 tests pass
- APP-011 diag ses_f4ca3a4cdfferD0yf8IohSYuzN -> crates/cli/src/diagnostics.rs DONE 361L, 6 tests pass
- TUI-003 ses_f4ca3a4caffeS8Dvi27YbPVAHE -> crates/cli/src/native_app.rs DONE 374L, 6 tests pass
- TUI-004 ses_f4ca3a4c8ffe7pjQ25o5SD89AD -> crates/cli/src/native_composer.rs DONE 903L, 14 tests pass
- TUI-005 ses_f4ca3a4c6ffe1Jyb8xloPlAJbr -> crates/cli/src/native_transcript.rs DONE 466L, 7 tests pass
- PAR-002 ses_f4ca3a4c4ffe0mWyPQoo6Y694h -> crates/providers/src/app_routing.rs DONE 553L, 7 tests pass
- PAR-003 ses_f4ca3a492ffeb5tP6TME5gKZ5R -> crates/sessions/src/app_history.rs DONE 430L, 5 tests pass
- PAR-004 ses_f4ca3a48effeLOuMgUrLsOuRNL -> crates/agents/src/app_delegation.rs DONE 368L, 4 tests pass
- NET-002 ses_f4ca3a48cffeot6ht6W2092lVG -> crates/cli/src/pair.rs DONE 355L, 8 tests pass
- PROV acct ses_f4ca3a48affehRc4fyOIs3T0o6 -> crates/providers/src/account_setup.rs DONE 617L, 10 tests pass
- COORD-008 ses_f4ca3a45dffehYlyGqcbBWXVRx -> tools/completion_reconcile.py DONE 235L self-check OK
- SHIP-007 ses_f4ca3a45bffeKSo322NyVouW7N -> docs/USER_GUIDE.md DONE 7.7KB, check OK

## Verified so far
- SPEC OK 90/258/450; 38 completion tests OK; py compile OK (adapter/ownership/verification/budgets).
- Rust standalone: app_start 8/8, terminal_host 6/6. app_services 11/11, app_protocols 16/16, security lib 132 pass (temp mod wiring).
- No stubs in landed files (only doc-mention matches + one ponytail digest marker in app_services).
- Landed files forbid(unsafe_code). No workspace cargo build run (6GiB host, lanes active).

## Integrator TODO (after all return)
1. Add `mod` lines (main.rs/lib.rs per crate) for new Rust modules; rerun focused cargo check + tests.
2. Add tasks/completion/discovered.json to ralph.completion.json includes (after schema check).
3. Run full gates: validate_repository.py, bootstrap tests, completion tests, --check, --audit-legacy.
4. Single commit; no bulk accept flips; no test edits.

## Wave3: 20 (running, one file each)
- TUI layout ses_f4c94a601ffevxus1taDwR2now -> crates/cli/src/native_layout.rs DONE 200L, 3 tests pass
- TUI-006 palette ses_f4c94a5ffffeh4OEeO4MpKwbYY -> crates/cli/src/native_palette.rs DONE 787L, 9 tests pass
- TUI-007 nav ses_f4c94a5fdffeOz9fwO5f6luod5 -> crates/cli/src/native_navigation.rs DONE 547L, 8 tests pass
- TUI-008 appr ses_f4c94a57cffeQyjy4TgpwJiGq3 -> crates/cli/src/native_approvals.rs DONE 577L, 10 tests pass
- NET-005 conn ses_f4c94a578ffe4WLainpsaT83MO -> crates/server/src/remote_connector.rs DONE 821L, 7 tests pass (retry confirms, no diff)
- TUI-009 status ses_f4c94a57affew73NwlhQQ6mgvS -> crates/cli/src/native_status.rs DONE 13KB, 6 tests pass
- APP-006 ws ses_f4c94a579ffebtCX7LrfksJ272 -> crates/server/src/workspace_sessions.rs DONE 862L, 6 tests pass (retry confirms)
- NET-005 conn ses_f4c94a578ffe4WLainpsaT83MO -> crates/server/src/remote_connector.rs
- NET-007 rsess ses_f4c94a577ffehX8rAQNMKKk82u -> crates/server/src/remote_sessions.rs DONE 462L, 5 tests pass (retry ses_f4c93ddd8ffeYRCvEG0no25PSU)
- NET-008 rturn ses_f4c94a575ffezCalP4SW6S1PvH -> crates/server/src/remote_turns.rs DONE 617L, 9 tests pass
- NET-009 rappr ses_f4c94a574ffejE5QDMba8SH0yi -> crates/server/src/remote_approvals.rs DONE 553L, 5 tests pass
- NET-010 rfile ses_f4c94a573ffe5q1pmoVCKAmxdl -> crates/server/src/remote_files.rs DONE 408L, 9 tests pass
- NET-011 rpty ses_f4c94a571ffeBE1HN6f1oJTI4V -> crates/server/src/remote_pty.rs DONE 904L, 7 tests pass (retry ses_f4c91de16ffehoLKUY0wmpOqJh)
- NET-012 rrec ses_f4c94a56fffeQ7KrY1U66dDdxH -> crates/server/src/remote_recovery.rs DONE 626L, 7 tests pass
- PAR-008 wadapter ses_f4c94a56effeMvetHJqXuJa8o9 -> crates/server/src/web_turn_adapter.rs DONE 516L, 5 tests pass
- TUI-005 ses_f4ca3a4c6ffe1Jyb8xloPlAJbr -> crates/cli/src/native_transcript.rs DONE 466L, 7 tests pass
- PAR-007 ext ses_f4c94a4baffe5dWLu14HrFDLIn -> crates/tools/src/app_extensions.rs DONE 445L, 8 tests pass (235L present at check, lane-shipped 445L)
- DB backup ses_f4c94a4b9ffevcMRELfnfwU504 -> crates/storage/src/backup_v2.rs DONE 13KB, 7 tests pass
- AGENT exec ses_f4c94a4b7ffeV2k7SkRzxeUji3 -> crates/agents/src/agent_executor.rs DONE 316L, 5 tests pass
- NET-013 revoke ses_f4c94a4b5ffeBwAlHhgH4VYeiQ -> crates/server/src/remote_revocation.rs DONE 495L, 6 tests pass
- APP-008 headless ses_f4c94a4b4ffeLlHTBoK6v6Z0k3 -> crates/cli/src/headless_engine.rs DONE 276L, 5 tests pass
- SEC matrix ses_f4c94a4b2ffeLafC2eWjk23Eb6 -> crates/security/src/platform_matrix.rs DONE 397L, 5 tests pass

## Blockers (honest, not passed)
Native daemon auth/TUI renderer/OS sandbox/packaging/signing/mobile builds/devices/provider budget — COORD/SHIP lanes mark BLOCKED.
