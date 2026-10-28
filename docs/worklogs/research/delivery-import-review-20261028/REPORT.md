# Combined import evaluation report — 2026-10-28

Role: independent delivery-package preservation/import evaluator. Not an author of
the package. Session `ses_e7d5e6eb0ffedW7B0viIth0zAF`, River route.
Canonical: `/Users/mymac/Projects/opencode-rk-recovery-20261005`, branch `main-v2`,
HEAD verified `92b216d04e5e71370247105e7cf119dce0b23f1f`.
Source: `/Users/mymac/Projects/.oc2-delivery-dynamic-20261028-78571fd6/source`.

## Verdict

**SOURCE_APPROVE** for exact preservation of all 19 supplied files.

**Operational activation: separately BLOCKED** by dirty canonical worktree. This
is a user decision, not a defect in the package. Details below.

## Preservation evidence (independently reproduced)

All 19 files exist in canonical and are byte-identical to the supplied source:

| Path | SHA-256 |
| --- | --- |
| config/delivery-tasks.example.json | `81d028a0a3d9063a50d752a45dd7b13262728bb11e687402b5f16877ddebb590` |
| config/delivery-workflow.json | `6f71836d93528c34a395e5001e0451e1050d983e91db5e1f679b2a0aba6fc1f6` |
| docs/CONTINUOUS_DELIVERY.md | `b5d3c1237c827ec52f7693c961f88a02fff1484502ba9397d8e5f3ec0199289b` |
| tests/delivery/fixtures/agent.py | `852938c41f6e7125bd6759aff118a99369dab5af6e8e94bf1157208abb3fd67c` |
| tests/delivery/test_engine.py | `117f39e8ca7ba8f592c2fc8872833ed3b2f064a52e880380e07eb7b0a18b3037` |
| tests/delivery/test_fleet.py | `86396083c1041d237d6a7bbcc58e3aa8f36435cf2263c15e3fe14437a9e5b6c0` |
| tests/delivery/test_hardening.py | `ff554b8d97db674c1d5a353a8b317bb6e9ef1f1b5c04376879a524204efbd29e` |
| tests/delivery/test_invariants.py | `43635eaf90f0ff3ed674277625d2720cc352a684cca4119485e60e938a9ad2af` |
| tests/delivery/test_recovery.py | `d2a0af4cf07ec3485fc86a811192dbc2a934506a47a6e2264be20db35114516f` |
| tools/continuous_delivery.py | `c8744da04ec2e29f17c4da7ca20abe6d1941d5a5f33bf428809e47b1f4857788` |
| tools/delivery/__init__.py | `45c87bb2be71b4fe585831a98f0d7bc355805847bc110645668ad586c0a654e7` |
| tools/delivery/cleanup.py | `3b87537fc57b038eb3013b07152e3ac669cdb2b9cc6cabaab3bac6dfa8717f53` |
| tools/delivery/engine.py | `b455c9b7136353a5e1d6cea07fd4fb38330db91828063b3a29f07fc993772813` |
| tools/delivery/jobs.py | `61a56a894505405a5527fcd3acdf1858cafdbacd572ceff44668778bbb1ff5c9` |
| tools/delivery/model.py | `10a4d1d5ac04c64ffa67ef61e06036f372f70ceb03fd0ce5f93a9cf27670f8cc` |
| tools/delivery/process_guard.py | `a77376ccd6cf5e3ea2c715c91ed57601a0ed7cf3cf86c7c482c151718ddda387` |
| tools/delivery/runtime.py | `ea68fce73e7ff15ab3f234df26844b2576876f7881b380c80ab8c26a367d532a` |
| tools/delivery/store.py | `c91936aca71c8b9a06aea043e0533bd782e6caba27bca3cb96f482c52fa0edd7` |
| tools/dynamic_agent_allocator.py | `12f07fe1c397f9c0f8e5b349ff189c8762ca7597c363a67047e12840018aed15` |

Line totals independently recomputed: 3,177 lines across exactly the 19 paths
(2,924 from the 16 code/config files + 27 fixture + 29 allocator + 197 doc).
Full machine manifest: `HASH-MANIFEST.sha256` in this directory.

## Tracking and history status (independently verified)

- `git ls-files` on all 19 paths: **empty**; nothing tracked.
- `git log --all -- <all 19 paths>`: **zero commits**. Copied, never merged.
- Source directory is not a Git repository (no `.git` under `source/`).
- `tools/` has pre-existing unrelated tracked files; the delivery files were not
  among them. `config/` currently tracks six unrelated JSON files.

## Package review findings (read completed; independent, non-author)

Read every file: CONTINUOUS_DELIVERY.md (197 lines), both configs, all eight
`tools/delivery` modules, entry CLI, allocator, five test modules, fixture.
Key properties confirmed against code, not claims:

- Fenced leases, single-route-no-fallback (`model.config` requires explicit
  routes; `choose_route` returns None rather than substituting), author cannot
  review own candidate (`jobs.stage` review branch), tool output can never
  supply a verdict (`runtime.assistant_text` ignores tool parts),
  `structured_response` strict JSON contracts, scope/symlink/credential checks
  in `check_scope`, ff-only publication with journal + reconciliation in
  `jobs.publish`/`reconcile_publication`, non-stashing dirty-source refusal.
- `source_gate` refuses publication while `git status --porcelain=v1
  --untracked-files=normal` shows anything: tracked modifications AND untracked
  files both block. Deliberate; no bypass exists in code.
- 76 delivery tests PASS (255.164 s) in main's run at
  `/Users/mymac/Projects/opencode-rk/.git/recovery-evidence/delivery-audit-20261028/delivery-tests.log`
  (log SHA-256 `25daa336532ded668a81219555557e9ee2c88f14673d007377acebd0963e15f6`).
  I did not rerun tests per instruction; the receipt is consistent with the
  fixture code reviewed. Agent fixture is a deterministic double, not a model.

Manifest staleness confirmed: `source-manifest-v2.json` has 16 entries; 9
hashes differ from the actual supplied bytes (older snapshot), 3 package files
missing entirely. The actual `source/` bytes were preserved; the stale manifest
was correctly NOT used to restore older bytes.

## Source preservation vs operational activation (separate)

**File preservation: COMPLETE.** All 19 files, exact bytes, zero drift.

**Operational activation BLOCKED by canonical dirty state.** Two independent
gate inputs block publication:

1. 27 tracked files modified (user web/auth/docs work, e.g. `web/src/App.tsx`,
   `crates/server/src/daemon_auth.rs`, `AGENTS.md`, scratchpad).
2. Untracked user work inside the repo (`.private-tmp/`, `.scratch.RArhl2/`,
   `T/`, `concurrency-repair-wave/`, and other untracked product artifacts)
   which `--untracked-files=normal` also reports.

No stash/reset/clean/broad-commit is permitted or performed. Resolving this is
the user's decision. Agents must not manufacture cleanliness.

## Bootstrap/import recommendation (policy-compliant, no invention)

Strictly ordered owner actions; none performed by this evaluator:

1. **Scoped bootstrap commit of exactly the 19 package paths** on `main-v2`
   (pathspec-limited `git add` of the 19 files + `git commit`). This makes the
   package tracked and preserves it in history without touching the 27 dirty
   modified files or any other untracked work. Requires Recovery-Mode write
   capability per existing hook if active.
2. **Do NOT initialize the controller before step 1** if the intent is to
   include the package in `policy_base_sha`: `_setup` pins
   `policy_base_sha = branch HEAD`, and `source_gate` later flags any
   protected-path change since that pin. Package files include
   `docs/CONTINUOUS_DELIVERY.md` and `tools/delivery/`, both protected. Commit
   first, then `init`.
3. **`doctor` then user decision on remaining dirty work.** The gate will still
   block on the user's dirty/untracked product work. Only the user can choose
   to land, set aside, or explicitly authorize carrying that work. Agents: no
   stash, no reset, no clean, no broad commit, no hook disable.
4. **Then** `run` without `--publish` for dry validation, import a real
   owner-authored task manifest (the example JSON is explicitly not a backlog),
   and only with explicit publication authorization enable `--publish --watch`.
5. Token Harbor route: present in config but user-observed paused/errored. The
   controller marks routes unavailable from its own observed runtime failures
   with bounded backoff; it has no out-of-band health source. Until then,
   dispatch attempts to that route will burn attempt cycles before backoff.
   Recording the pause out-of-band is an owner/AGENTS-level responsibility, not
   something to silently patch into the config here. Usage UNKNOWN.

Chat investigation artifacts exist at
`chat-investigation-vyce/` with a later gap-closure pass and the reconciled
`docs/worklogs/research/shared-chat-6ae1f178/README.md`; acknowledged as
covered, empty final list item honestly recorded there. Not re-verified here.

## Errors encountered during this evaluation

- Repeated shell-tool `command` argument drops (non-deterministic); retried.
- One wrong path attempt for `tests/delivery/agent.py` (actual path is
  `tests/delivery/fixtures/agent.py`); corrected. No data impact.
- First `source-manifest-v2.json` comparison used wrong parent path; corrected
  to the actual location; comparison completed.

## Limits

This report does not claim controller acceptance, publication, or product Phase-1
acceptance. No canonical/source writes, no Git mutations, no builds/tests, no
browser work, no nested agents. Writing was confined to this directory.
