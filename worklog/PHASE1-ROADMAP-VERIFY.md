# PHASE1-ROADMAP-VERIFY

## Claim

- Task: `PHASE1-ROADMAP-VERIFY`, session `ses_f2e26c28bfferM86JJzsHLeEEK`.
- Owned files ONLY: `worklog/PHASE1-ROADMAP-VERIFY.md` + own ledger row. No edits to
  the roadmap artifact, product, tests, plans, controller files, or verifier code.
- Prior owner of the artifact: `ses_f2e2b0c72ffeT2x01toLrwBRBC` (row left
  `in-progress`; not touched by this verifier, orchestrator-only release).
- Verdict: ROADMAP NOT CONSUMABLE AS-IS. Status `blocked` with exact deficiencies.

## Artifact under verification

- Path: `worklog/PHASE1-ROADMAP-20260923.md` (161 lines).
- Landed: commit `bb027c1a02723efe107a6d3f196d95722758b289`
  "plan(phase1-roadmap): evidence-grounded dependency graph to unsigned release".
- Ref: `refs/heads/plan/phase1-roadmap` remote == local `bb027c1...`
  (`git ls-remote origin refs/heads/plan/phase1-roadmap` returned `bb027c1...`).
- Commit contents: `worklog/PHASE1-ROADMAP-20260923.md` (+161) and
  `tasks/completion/claims.json` (+5 roadmap row). Nothing else. Clean tree.

## Structural requirements check (all pass)

- Substantive: yes, 161 lines across Claim/Evidence/Mandatory parents/Remaining
  lanes/Conflicts/Parallelism/Effort/Critical path/Wave design/Platform ledger.
- Evidence-cited: mostly yes, path:line refs; several verified below.
- Dependency graph: present as node/dep lists (APP-012, COORD, NET, MOB, SHIP).
  Not a formal node+edge rendering, but deps are enumerated for the critical chain.
- Critical path: section "Critical path to unsigned Phase 1 release" (6 steps), ends
  in explicit SHIP-008 BLOCKED decision. No fabricated completion.
- Wave design: `## Wave design (20-worker harness: 4 integration-spine + 2 verifier
  reserved, <=14 breadth)` matches AGENTS.md exactly (4 spine, 2 verifier, <=14
  breadth, "no wave of leaves-only").
- Platform distinctions: macOS / Linux / Windows-GNU / MSVC-out-of-scope, with
  source-GREEN != packaged != clean-machine separation. Matches docs and worklogs.
- External blockers: signing/notarization, live canaries, devices, Windows runner,
  Linux PTY runner. No fabricated receipts.
- No false acceptance: artifact explicitly says "no acceptance claimed"; it does not
  mark any parent complete. This part is honest.

## Quantitative claims re-verified (PASS)

Commands run (cwd = repo root):

- `python3 tools/convergence_gate.py` -> `total=86` (roadmap says total=86). MATCH.
- `python3 tools/validate_repository.py` ->
  `validate_backlog_exhaustion: 51 error(s)` + `FAIL backlog exhaustion exit=1`
  (roadmap says `validate_backlog_exhaustion: 51 error(s)`). MATCH.
- Plan stories: `12+16+19+10+21+11 = 89` across
  `tasks/completion/{local,delivery,discovered,parity,remote,tui}.json`
  (roadmap says 89). MATCH.
- Plan-task ledger join (local tree): completed 47, no-row 40, blocked 1,
  in-progress 1 (roadmap says 47/40/1/1). MATCH.
- Ledger at `bb027c1`: 206 rows, `completed 149 / blocked 33 / in-progress 24`
  (roadmap says local 149/33/24). MATCH.
- Ledger at `origin/main 8a91a7b`: 161 rows, `123 / 18 / 20`
  (roadmap says origin 123/18/20). MATCH.
- Ledger-only IDs at `bb027c1`: 206 - 49 plan-joined = 157 (roadmap says 157). MATCH.
- Frozen hashes cited: `APP-012` tool RED sha `945236c4...` confirmed by
  `git show bb027c1:crates/server/tests/app012_tool_journey_red.rs | shasum -a 256`.
  `INSTALLED-DEFAULT-CONTRACT` sha `fec2fdb9...` and
  `installed_default_entrypoint.rs` sha `fec2fdb9...` confirmed by shasum at
  `bb027c1`. `PROV-024` sha `750e3a32...` confirmed in
  `worklog/PROV-024-FIXTURE-RED.md:48`. `TUI-011` refrozen `cb7d4cde...` confirmed
  by shasum of `crates/opentui-bridge/tests/native_artifact_manifest.rs` at
  `bb027c1` and in `worklog/TUI-011-FROZEN-CONTRACT-REVIEW.md:22`. MATCH.
- `docs/CONVERGENCE.md:10-24` hard boundary: verified same content.
- `crates/tools/src/executor.rs` generic `execute` path still only dispatches
  `bash`/`shell`/`echo` else `Unknown tool` at `bb027c1`; brokered `read` is a
  separate `execute_authorized` path added by `40d56d5`. Roadmap statement
  "executor only runs bash/shell/echo" is TRUE for the generic path. MATCH.
- `scripts/install-oc2.ps1` has no native closure / bounds (copy-only install);
  roadmap statement TRUE. MATCH.

## Exact deficiencies (BLOCKING; not repaired)

1. INVERTED BRANCH RELATION. Roadmap `## Claim` says base `1f4a9e6` "vs
   `origin/main 8a91a7b`" and `## Decisions` says "Branch is behind origin/main
   (missing W5 scratchpads + `8a91a7b` wave commit); rebase plan/phase1-roadmap
   before integration use."
   FALSE. Verified: `git merge-base --is-ancestor origin/main bb027c1` ->
   origin/main IS an ancestor; `git rev-list --count origin/main..bb027c1` = 174,
   reverse = 0. The branch already contains `8a91a7b` plus 174 commits. It is
   AHEAD, not behind. The instruction to rebase before integration is wrong.
   The `1f4a9e6` base is likewise ahead of `origin/main` by 173 commits.

2. WRONG CAUSE FOR MISSING W5 SCRATCHPADS. Roadmap says `worklog/W5-*.md` are
   "absent locally = this branch behind origin". Verified: origin/main tree
   contains NO `worklog/W5-*.md` either (only `worklog/LANE-RULES-INJECT.md` and
   `worklog/LANE-STREAM-FIX.md`). The W5 scratchpad files are absent from BOTH
   trees, so absence is not evidence of being behind. The 20 `LANE-*` wave-5 rows
   are `in-progress` in the origin ledger but their declared scratchpads
   (`worklog/W5-LANE-*.md`) were never committed on any inspected ref.

3. STALE FROZEN-CONTRACT BLOCKERS. Roadmap (lines 30-33, 81-82) lists as OPEN:
   - "frozen `installed_default_entrypoint.rs` has five immutable `todo!()`
     bodies" (APP-012 blocker), and
   - "frozen `native_artifact_manifest` conflicts (SHA-digit predicate :95,
     64KiB dylib bound :122)" (TUI-011 remaining), plus
   - "frozen-contract-review corrections (installed_default_entrypoint 5x todo!,
     native_artifact_manifest 2 assertions; controller authority only)".
   Verified at the roadmap's OWN stated base `1f4a9e6` and at `bb027c1`:
   - `git show bb027c1:crates/cli/tests/installed_default_entrypoint.rs | grep -c
     "todo!"` = 0; the file is the refrozen `fec2fdb9...` revision landed by
     ancestor `15381e3` ("test(cli): replace installed default entrypoint todo
     journeys").
   - `crates/opentui-bridge/tests/native_artifact_manifest.rs` at `bb027c1`:
     sha predicate now `c.is_ascii_digit() || ('a'..='f').contains(&c)` (digits
     accepted), `MAX_ARTIFACT_BYTES = 128 * 1024 * 1024` used for the dylib at
     :122; refrozen `cb7d4cde...` by ancestor `0153bc1` ("Correct and refreeze
     native artifact contract") and recorded in
     `worklog/TUI-011-FROZEN-CONTRACT-REVIEW.md`. The stated corrections are
     already landed, not remaining controller work.
   These two items should not appear as current blockers/critical-path steps.

4. LINE CITATION DRIFT (minor, non-blocking). Roadmap cites
   `tools/src/executor.rs:105-119` for the bash/shell/echo limitation; at
   `bb027c1` the limitation lives at `crates/tools/src/executor.rs:118-125`
   (generic `execute` dispatch). Substance is correct, coordinates are stale.

## What is correct and reusable

- The mandatory-parent list, external-blocker list, platform ledger, wave
  reserve numbers, and no-acceptance posture are accurate and useful.
- All ledger/gate counts and frozen hashes cited verified exactly.
- The critical path shape (controller rulings -> APP-001/005/012 -> TUI-011
  packaging -> SHIP-001 unsigned artifacts -> SHIP-002 matrix -> COORD/SHIP-004/
  006/007 -> SHIP-008 stays blocked) is a defensible dependency ordering once
  items 1-3 are corrected.

## Consumer guidance

Roadmap may be consumed ONLY after correcting items 1-3 (branch relation, W5
absence cause, stale frozen-contract blockers). Until then it must not be used as
the authoritative dependency/critical-path source, because it would send the
orchestrator to re-rebase an already-ahead branch and to re-do frozen corrections
that are already landed. No repair performed by this verifier.

## Commands run

- `git status -sb`, `git log --oneline -15`, `git branch -a`
- `git ls-remote origin refs/heads/plan/phase1-roadmap`
- `git rev-list --left-right --count origin/main...bb027c1`
- `git merge-base --is-ancestor origin/main bb027c1`
- `git show --stat --format=fuller bb027c1`
- `python3 tools/convergence_gate.py` -> total=86
- `python3 tools/validate_repository.py` -> 51 errors, exit 1
- `git show bb027c1:<frozen test>` + `shasum -a 256` for all cited hashes
- `git show bb027c1:crates/cli/tests/installed_default_entrypoint.rs | grep -c todo!` = 0
- `git ls-tree -r --name-only origin/main bb027c1 | grep W5-` = none