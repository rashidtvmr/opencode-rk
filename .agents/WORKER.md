# Worker protocol: claim → own → update → hand back (MANDATORY)

Every delegated subagent reads this file FIRST, before touching any task or
product file. The orchestrator (`prompts/COMPLETE_APP.md` "Task-claim ledger and
scratchpads") holds the orchestrator-side mirror of this protocol.

## 1. Claim before you touch anything

Your task is YOURS only after the ledger says so. The ledger is
`tasks/completion/claims.json`, managed exclusively through
`tools/completion_claims.py` (stdlib, write-through, fail-closed on collision).

Before creating or editing ANY file:

1. Confirm your task is pickable (deps completed, no session holds it).
2. Claim it, supplying the session id your orchestrator gave you and your
   scratchpad path:

   ```python
   import sys, pathlib
   sys.path.insert(0, 'tools')
   import completion_claims as cc
   cc.claim(pathlib.Path('.'), '<TASK-ID>', '<your-session-id>', 'worklog/<TASK-ID>.md')
   ```

`claim` fails closed if another session already holds the task in
`in-progress`/`blocked`. If it raises `ClaimError`, STOP: report the collision
back to the orchestrator and pick nothing. Two agents can never both own a task.
Only after a successful claim may you create `worklog/<TASK-ID>.md`.

## 2. Maintain your scratchpad while you work

Your scratchpad `worklog/<TASK-ID>.md` is your session persistence: keep claim,
source evidence (exact path/line/symbol), observed scenario, target boundary,
tests written, decisions, and remaining unknowns in it as you go. Update it as
you work — it is the record your orchestrator reads when you are done or if you
die mid-task. It is the ONLY file you may write besides your one owned task
file. Do not store credentials or entire transcripts in it.

## 3. Keep your status honest

Status lives only in the ledger, never in `ralph.json`/`ralph.completion.json`
(the plan loader forces story statuses to `not-started`; ledger rows are the
progress record). Legal transitions:

- `not-started -> in-progress` (implicit in `claim`)
- `in-progress -> completed` — ONLY when every frozen test for your owned file
  runs green with ZERO test-code modifications ("complete" = feature file
  written, all test code written, tests green without touching the tests)
- `in-progress -> blocked` / `not-started -> blocked` — with a bounded note
  carrying the exact blocker
- `blocked -> in-progress` — only after the blocker is actually removed

```python
cc.update(pathlib.Path('.'), '<TASK-ID>', '<your-session-id>', 'completed')
```

Never set `completed` to dodge verification. The independent verifier re-runs
your frozen tests; a false `completed` is a failed lane and you will be
re-delegated.

## 4. Hand back to the orchestrator

Your completion message to the orchestrator MUST state:

1. Task id and final ledger status you set.
2. Your scratchpad path (e.g. `worklog/TASK-ID.md`) — the orchestrator collects
   these via `cc.scratchpad_report(document, session)` and consults them before
   re-delegating or integrating your lane.
3. Exact test commands you ran and their results.

Do not `release()` your own claim; releasing is the orchestrator's job when it
integrates or abandons your lane. If you must stop early (blocked, budget,
context), set the honest status first, then report the same three items plus the
blocker.

## 5. Commit and push every completed feature (MANDATORY)

A feature is not complete until its work is landed:

1. Set the honest ledger status first (`cc.update(..., 'completed', note)` with
   the exact test evidence — see section 3).
2. Commit ONLY the files of your lane (your owned file, your scratchpad,
   `tasks/completion/claims.json`, and any tests you authored in the RED phase).
   Never sweep unrelated files into your commit.
3. Push to the remote immediately after the commit succeeds:

   ```sh
   rtk git add <your-lane-files>
   rtk git commit -m "<lane>: <what> — <evidence>"
   rtk git pull --rebase && rtk git push
   ```

   If the push is rejected because `main` advanced, rebase on the fresh `main`,
   re-run your frozen tests on the integrated tree, and push again. Never force-push.

Your completion message must include the commit hash and the pushed branch/ref
so the orchestrator can verify the landing.

## 6. Big tasks: worktree + named branch (keep the branch, delete the worktree)

If your task is large, touches a file other lanes may race, or would benefit
from an isolated tree, work in a dedicated git worktree on a named branch:

1. Create it (branch name convention: `lane/<TASK-ID>`, e.g. `lane/APP-006`):

   ```sh
   rtk git worktree add ../opencode-rk-<TASK-ID> -b lane/<TASK-ID>
   cd ../opencode-rk-<TASK-ID>
   ```

2. Work there exactly as above (claim → scratchpad → RED → implement → green).
   Your scratchpad and ledger row still live in the SAME repository — update the
   ledger from the worktree (same repo object store, same claims.json path).
3. When green, push the BRANCH first — it must exist in the remote as a
   permanent reference, even after merge:

   ```sh
   rtk git push -u origin lane/<TASK-ID>
   ```

4. Merge into `main` (fast-forward or a normal merge; rebase onto fresh `main`
   first if it advanced), re-run the frozen tests on the merged tree, and push:

   ```sh
   rtk git checkout main && rtk git pull --rebase
   rtk git merge --no-ff lane/<TASK-ID>
   # re-run your frozen tests here on the merged tree
   rtk git push
   ```

5. After the merge is confirmed on the remote, delete the WORKTREE only —
   NEVER the branch. The branch stays on the remote as the lane's history:

   ```sh
   cd <original-repo-root>
   rtk git worktree remove ../opencode-rk-<TASK-ID>
   rtk git fetch --prune
   ```

   Do not run `git branch -d` / `git push origin --delete` for your lane branch;
   that is reserved for the orchestrator/human and is not part of lane completion.

## 7. Hard boundaries

- One owned file: never edit anything outside your owned task file, your
  scratchpad, and the ledger row for your own task id. (Worktree files for your
  lane are the same files, isolated; this rule does not grant extra paths.)
- No test edits: frozen tests are frozen. Fix implementation, never tests.
- No controller/state file edits: `ralph.json`, `ralph.completion.json`,
  `FEATURES.md`, verifier/config/policy files are out of your authority.
- No silent takeover: a task held by another session is untouchable until the
  orchestrator proves the prior owner stopped and re-claims it.
- No force-push, no history rewrite, no branch deletion on the remote. If a
  push is rejected, rebase and re-run tests; never `--force`.
