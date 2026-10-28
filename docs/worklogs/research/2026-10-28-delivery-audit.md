# Dynamic delivery import audit — 2026-10-28

## Finding

**Subsequent resolution:** the user explicitly authorized a one-time bootstrap of
exactly the 19 reviewed package files. Commit
`3fae911a15f34496b81ec7526d7c43d014c8a3fe` now preserves them in Git history.
All committed blobs match the reviewed source hashes; the pre-existing unrelated
tracked diff was byte-identical afterward. Existing hook/capability rules stayed
enabled. The findings below describe the audit-start state.

The complete supplied `source/` package is present in this working tree with
identical bytes: **19 files, 3,177 lines**. It has not been merged into repository
history. All 19 paths were untracked at inspection; the all-ref path-history
query returned no commits. The source directory is not a Git repository.

Canonical revision inspected:
`92b216d04e5e71370247105e7cf119dce0b23f1f`.

Source:
`/Users/mymac/Projects/.oc2-delivery-dynamic-20261028-78571fd6/source`.

Exact paths, SHA-256 values, line counts, tracking status and recent commit
name-status evidence are preserved in
[`2026-10-28-delivery-source-audit.json`](2026-10-28-delivery-source-audit.json).
The supplied older 16-entry `source-manifest-v2.json` describes a different
snapshot; it must not be used to overwrite the actual supplied 19-file package.

## Read coverage and adopted instructions

Main read all supplied documentation, configuration, controller modules, CLI,
allocator, five test modules and deterministic agent fixture. The compact restart
contract is [`../../DELIVERY_OPERATING_CONTRACT.md`](../../DELIVERY_OPERATING_CONTRACT.md).
`AGENTS.md` records the latest user direction and its precedence over historical
manual integration and fixed-lane scheduling instructions.

The dynamic supervisor owns new scheduled delivery: fenced task-stage leases,
independent exact-candidate reviews, trusted focused checks, re-review after SHA
changes, full checks, one guarded fast-forward publication, and fresh exact-SHA
postchecks. ACCEPTED is distinct from installed Phase-1 acceptance.

## Executed verification

```sh
python3 -m unittest discover -s tests/delivery -p 'test_*.py' -v
```

- Exit: **0**.
- Executed: **76 tests; all passed**.
- Unittest duration: **255.164 seconds**; wrapper duration: 255.28544425964355.
- Log SHA-256:
  `25daa336532ded668a81219555557e9ee2c88f14673d007377acebd0963e15f6`.
- Raw log and JSON receipt:
  `/Users/mymac/Projects/opencode-rk/.git/recovery-evidence/delivery-audit-20261028/delivery-tests.log`
  and `delivery-tests.json`.

The suite exercises real subprocesses and Git operations in disposable fixtures,
including the rolling twenty-task fleet, watched task admission, independent
review rejection, dirty-work preservation, lease fencing, single-supervisor locks,
process cleanup and publication crash recovery. Its agents are deterministic test
doubles. These results establish neither live-provider concurrency nor installed
product acceptance. Heavy validation was serialized using the shared lock.

## Outstanding work and preservation

- Sole shared-chat investigator: `ses_e7d5fe66bffeKqPiz4k1qdHadM`, using `cua_repl`.
  Investigation completed, with original and revised artifacts preserved in
  `shared-chat-6ae1f178/`. Both observed turns captured; final assistant list item
  has no accessible text. README records coverage limits and the supporting-log
  manifest mismatch. No unseen conversation content is asserted here.
- Independent package import evaluator: `ses_e7d5e6eb0ffedW7B0viIth0zAF`.
  Completed **SOURCE_APPROVE** for all 19 exact files. Main checked the report
  and scratchpad hashes; report preserved under
  `delivery-import-review-20261028/REPORT.md`. Operational activation remains
  separately blocked. Main independently confirmed the old manifest has 16
  entries, nine differing hashes and three missing package paths.
- The package remains untracked. This report is not a publication receipt.
  Source preservation in Git history still requires a policy-compliant import.
- Canonical contains pre-existing dirty product/auth/web work. It is preserved;
  no cleaning, stashing, resetting, broad committing or hook disabling was used.
- No production delivery controller was initialized or started during this audit.
  The old scheduler was not launched. Private product candidates and negative
  verdicts remain preserved, with product integration paused.
- Token Harbor remains paused after observed errors despite its presence in the
  supplied route configuration. Configured aliases do not prove route health.
  Token usage remains UNKNOWN.

The recent footer, authenticated snapshot and horizontal-tab commits list only
modifications and additions. The two existing deleted web asset paths are separate
dirty work with replacement asset paths present; this audit does not remove them.

## Bootstrap and controller initialization outcome

The user-authorized bootstrap committed only the 19 package paths. Audit notes,
transcripts and other dirty work remain outside that narrow commit. Receipt:
`/Users/mymac/Projects/opencode-rk/.git/recovery-evidence/delivery-audit-20261028/bootstrap-receipt.json`.

`init` then succeeded, with state at
`/Users/mymac/Projects/opencode-rk/.git/delivery-state` and policy pin equal to
`3fae911a15f34496b81ec7526d7c43d014c8a3fe`.

`doctor` exited 2: **OpenCode agent inventory failed or exceeded the output bound**.
`status` exited 0: dirty source, 20 configured workers, no fixed role quotas,
zero tasks/active roles, no verified SHA or publication barrier. Initialization
is not a running supervisor. No tasks were imported or dispatched, and no product
acceptance was recorded. Runtime inventory and source cleanliness remain blockers.
