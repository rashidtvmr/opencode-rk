# CONFIRM-CK verify-only — rev 248f519

## Scope
VERIFY-ONLY. No source edits. No stub edits. Owned file only: this report.
Serial runs, JOBS=1 THREADS=1, `timeout 120`, `free -h` checked before each run.

## Results (all GREEN, 0 failed)
- providers PROV-015..024: 12 suites, 62 tests GREEN. Log /tmp/opencode/cK-prov1524.log
  (auth_profile 5, codex_oauth 5, bounds 6, bounds2 6, prov_017..024 5 each).
- server acp/wsx-proxy/sdk/sync: 7 suites, 48 GREEN. Log /tmp/opencode/cK-srvnew.log
  (acp_bridge 5, acp_files 5, remote_sync/wsx-sync 5, sdk_client 12, sdk_spawns 11,
  sync_log 5, workspace_proxy/wsx-proxy 5).
- cli: 5 suites + 1 unit bin, 23 GREEN. Log /tmp/opencode/cK-clinew.log
  (doctor 5, run_headless 8, session_export 8, web_entrypoint 1, singleton_runtime 1).
- tools TOOL/SYNC/UI: 8 suites, 40 GREEN. Log /tmp/opencode/cK-tool.log
  (bulk/catalog/lifecycle/payload/policy/rtk_core/rtk_pass/rtk_testfilter 5 each).
- WEB-001..017 full sweep: 21 suites, 100 GREEN. Log /tmp/opencode/cK-web.log
  (server WEB-001..007,009..012,016,017 = 17 suites 80; sessions
  web_008_lane/WEB-008, chat_nav_lane/WEB-014, project_context/WEB-015,
  web_013/WEB-013 = 4 suites 20).

## Totals
- suites: 12+7+6+8+21 = 54. tests passed: 62+48+23+40+100 = 273. failed: 0.

## Guards
- No writes outside worklog/CONFIRM-CK.md. Frozen tests, lib.rs, ralph.json,
  mic untouched by this lane (0 paths matching frozen/ralph.json in diff names).
- Stub scan over owned product src: no `todo!()`/`unimplemented!()`.
- Worktree has pre-existing unrelated modifications (469 paths); lane added none.
