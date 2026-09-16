# SHARE aA wave (SHARE-003/004/005): temp-stub-restore + 6-suite GREEN

## State-on-arrival (rev 248f519, workdir dirty by others)
- Owned ONLY: `crates/sessions/src/share_enterprise.rs`, `share_store.rs`,
  `share_policy_lane.rs`, `share_policy2_lane.rs`. Untouched throughout:
  merge/queue/`lib.rs`, frozen `tests/`, `ralph.json`.
- Pre-existing workdir diffs (not mine, rustfmt-only): `share_store.rs` (+5/-5),
  `share_policy_lane.rs` / `share_policy2_lane.rs` (+1/-3 each), `lib.rs` (+3:
  `part_events`, `runner`, `tui_info_panel`). `share_enterprise.rs` clean.
- Baseline hashes (pre-task, /tmp/opencode/aA-baseline.sha256):
  enterprise `7f7e7ac4`, store `418be003`, policy_lane `10bbb9fb`, policy2 `0345fd4f`.
- Frozen prestart hashes (/tmp/opencode/aA-frozen-prestart.sha256): 6 suites OK
  before and after (`share_enterprise`, `share_store`, `share_policy_lane`,
  `share_policy2_lane`, `share_enterprise_lane`, `share_store_lane`).
- `ralph.json` clean (`git status --short -- ralph.json` empty). `lib.rs` diff
  is other-lane (+3 mods), not mine.

## Temp-stub-restore RED (impls restored byte-identical, sha256sum -c 4/4 OK)
- SHARE-003 wrong-partition (`partition(ShareHttp) => "WRONG-PARTITION-share-http"`):
  RED 2 pass / 3 fail (t01, t02, t04), log /tmp/opencode/aA-SHARE-003-red.log (81 lines).
- SHARE-004 duplicate-overwrite (drop `AlreadyShared` guard): RED 3 pass / 2 fail
  (t02, t04), log /tmp/opencode/aA-SHARE-004-red.log (65 lines).
- SHARE-005 classify-flip (429/5xx Retry => Abort `classify-flip-{status}`), both
  lanes: RED 3 pass / 2 fail each (t02, t04), logs /tmp/opencode/aA-SHARE-005-red.log
  (86 lines, policy_lane) + /tmp/opencode/aA-SHARE-005b-red.log (87 lines, policy2).
- Post-restore: enterprise `7f7e7ac448f61b62fb6022a9a7fed5f0acd95924139c9caa54cd5332a7ff86b4`,
  store `418be003787f5267e47f78856fda514d741f942c463e0ddc5917b80d6a686f41`,
  policy_lane `10bbb9fb8529eece8eb52241d55356e5b9cdfd5219db4f88c18263fecec544be`,
  policy2 `0345fd4fcfd31748b19a6a27116d294770cf5578bd24cf0bdc421c96cf535eb1`.
- No WRONG-PARTITION/classify-flip/TEMP-STUB strings remain in owned src.

## GREEN (serial JOBS=1 THREADS=1 timeout 120, free -h each step avail ~2.5Gi)
| suite | result | log |
|---|---|---|
| share_enterprise | 5/5 EXIT=0 | /tmp/opencode/aA-share_enterprise-green.log |
| share_store | 5/5 EXIT=0 | /tmp/opencode/aA-share_store-green.log |
| share_policy_lane | 5/5 EXIT=0 | /tmp/opencode/aA-share_policy_lane-green.log |
| share_policy2_lane | 5/5 EXIT=0 | /tmp/opencode/aA-share_policy2_lane-green.log |
| share_enterprise_lane (mirror) | 5/5 EXIT=0 | /tmp/opencode/aA-share_enterprise_lane-green.log |
| share_store_lane (mirror) | 5/5 EXIT=0 | /tmp/opencode/aA-share_store_lane-green.log |

## Non-touch confirmation
- NEVER touched: frozen `tests/`, `ralph.json`, `lib.rs`, `share_merge*`,
  `share_queue*`. Own-file net diff this wave: 0 lines (4 temp stubs reverted,
  backups in /tmp/opencode/aA-backup/).

## Confirm-2 cH VERIFY-ONLY (rev 248f519, no stubs, no edits)
- Impl hashes (verify-only, unchanged): enterprise
  `7f7e7ac448f61b62fb6022a9a7fed5f0acd95924139c9caa54cd5332a7ff86b4`,
  store `418be003787f5267e47f78856fda514d741f942c463e0ddc5917b80d6a686f41`,
  policy_lane `10bbb9fb8529eece8eb52241d55356e5b9cdfd5219db4f88c18263fecec544be`,
  policy2 `0345fd4fcfd31748b19a6a27116d294770cf5578bd24cf0bdc421c96cf535eb1`;
  lane mirrors: enterprise_lane `b1bef9e0`, store_lane `7c32b321`.
- Frozen hashes: `share_enterprise f8e6df55`, `share_enterprise_lane e2d06e7a`,
  `share_store dd21adf7`, `share_store_lane 5a95a015`,
  `share_policy_lane 804bca15`, `share_policy2_lane 47ae926f`.
- Stub scan: no `todo!()/unimplemented!()/TEMP-STUB/WRONG-PARTITION/classify-flip`
  in owned 4 + 2 lane mirrors.
- `lib.rs` verdict: complete-as-designed `#[path]` wiring; NOT edited (diff +3
  `part_events/runner/tui_info_panel` owned by wiring lane). `tests/`,
  `ralph.json`, `share_merge*`, `share_queue*` untouched (status clean).
- GREEN serial JOBS=1 THREADS=1 `--test-threads=1` timeout 120, avail ~2.6-2.8Gi,
  probe log /tmp/opencode/cH-share345.log: 6x `5 passed` + EXIT=0 =
  share_enterprise 5/5, share_enterprise_lane 5/5, share_store 5/5,
  share_store_lane 5/5, share_policy_lane 5/5, share_policy2_lane 5/5 = 30/30.
