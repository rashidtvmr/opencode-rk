# Diagnosis: Why "OCM: Main v2" Can't Complete in 10+ Hours

## Executive Summary

**The merge session merged only 16 branches out of 347 divergent branches (4.6%).** The remaining 269 eligible branches were quarantined, not because of git conflicts, but because the manifest's eligibility filter was too strict. Four out of nine L0 buckets produced **zero** merges (100% quarantined). The merge-sort tree worked correctly for the branches it accepted — the problem is upstream in the analysis/manifest phase.

---

## Evidence (Measured)

### What the merge run actually did

| Metric | Value |
|---|---|
| Run ID | `merge-run-20261001-20261001T213133-25441` |
| Base SHA | `8d5866e3` (original main-v2) |
| Total refs in manifest | 419 (991 in inventory including remote aliases) |
| Manifest status | `CANDIDATE_GRAPH_ONLY_NOT_MERGED_NOT_ACCEPTED` |
| Buckets created | 9 (with only ~221 eligible branches) |
| **Branches successfully merged** | **16** (across buckets 04-08) |
| Branches quarantined at manifest level | **198** |
| Additional quarantined during merge | ~205 more (bucket-level rejections) |
| L0 buckets with 0 merges | 4 (buckets 00, 01, 02, 03) |
| Merge refs created | 152 (mostly selftest probes) |
| Selftest refs | 132 (debugging attempts) |
| main-v2 HEAD after run | `180c55ee` (only 3 commits ahead of retry result) |
| **Still divergent from current main-v2** | **342 branches** |

### Bucket Results

```
bucket-00: accepted=0  quarantined=25   ← 100% rejected
bucket-01: accepted=0  quarantined=25   ← 100% rejected
bucket-02: accepted=0  quarantined=25   ← 100% rejected
bucket-03: accepted=0  quarantined=25   ← 100% rejected
bucket-04: accepted=5  quarantined=20
bucket-05: accepted=1  quarantined=24
bucket-06: accepted=2  quarantined=23
bucket-07: accepted=4  quarantined=21
bucket-08: accepted=4  quarantined=17
TOTAL:     accepted=16 quarantined=205
```

### Merge Levels Completed

```
L0: 9 buckets → 5 non-empty results (4 were empty = base SHA)
L1: 4 pairs → 4 results (pair-00, pair-01, pair-02, pair-03)
L2: 2 pairs → pair-00, pair-01, plus "pair-01-accounting-v3" (retry variant)
L3: 1 pair → "pair-00-accounting-v3" + normalized variant (retry)
L4: 1 pair → "pair-00-final-accounting-v3-run2"
Retry: "final-pass-v1" → d8489d63 (landed on main-v2)
```

The tree reduction worked correctly for the 16 branches that entered it.

---

## Root Causes (Ranked by Impact)

### 1. QUARANTINE REASON: `independent_verification_required_before_salvage` (88 branches)

**This is the #1 blocker.** The manifest flagged 88 branches with `verificationFlags: ["frozen_assertion_deletion_review"]` — meaning they contain diffs that delete or modify frozen test assertions. Per `AGENTS.md` and `docs/CONVERGENCE.md`, frozen tests are immutable to implementers. The merge engine correctly refused to merge them without independent review.

**These are not git conflicts. They are policy quarantines.** The branches contain real code changes (some are 500+ line diffs replacing `todo!()` stubs with full implementations), but they also modify frozen test assertions — which triggers the verification gate.

**Fix:** Each of these 88 branches needs a human or independent verifier to confirm the assertion changes are legitimate (replacing RED stubs with GREEN implementations, not weakening tests). This is inherently sequential review work, not parallelizable merging.

### 2. QUARANTINE REASON: `tip_is_ancestor_of_frozen_main_v2` (67 branches)

These 67 branches were already integrated into main-v2 at manifest-generation time but were still listed. They were correctly quarantined as "nothing to merge." Combined with the 71 identified as integrated at our check, this accounts for ~138 branches that are already done.

**Fix:** Remove from the work list entirely. These are not blocking — they're already merged.

### 3. QUARANTINE REASON: `changes_are_documentation_or_bookkeeping_only` (40 branches)

40 branches contain only worklog updates, claims.json changes, or documentation. The manifest filtered them because they don't represent product changes worth salvaging.

**Fix:** These can be safely heritage-merged (ancestry-only) or ignored. Per salvage policy: "mark duplicates/superseded/process-only branches explicitly."

### 4. MASSIVE CROSS-BUCKET ANCESTRY DEPENDENCIES (82 same-family, many more cross-family)

The manifest detected extensive cross-bucket ancestry:
- Buckets 00-03 contain ancestors of branches in buckets 01, 02, 05
- `lane/DB-022` in bucket-01 depends on ancestors in buckets 00, 02, 03, 07
- `lane/APP-001-FILE0600-SETUP-W1` depends on ancestors in buckets 00, 02, 05, 07

**The buckets are not independent.** The partition placed ancestors and descendants in different buckets. When bucket-00 fails (100% quarantine), its descendants in other buckets can't merge cleanly either.

**Fix:** Re-partition with ancestry-aware bucket assignment. Ancestors and descendants MUST be in the same bucket, or buckets must be ordered (merge bucket-07 before bucket-00, bucket-00 before bucket-01, etc.).

### 5. The `frozen_assertion_deletion_review` Flag Is Doing Too Much

Looking at the actual diffs (the audit log contains full diffs), many of these "assertion deletions" are legitimate RED→GREEN transitions:
- `todo!("RED: ...")` → real implementation
- Old stub assertions → full PTY-based E2E tests
- `fresh_home()`/`installed_binary()` helpers → proper `Root::new()`/`Pty::spawn()` with cleanup

The verification flag is correctly identifying assertion changes, but treating all of them as requiring review creates a bottleneck that prevents the merge tree from making progress.

### 6. 132 Selftest Refs = Debugging Thrash

The session created 132 selftest refs — probing whether individual merges would succeed. This suggests the agent spent most of its 10+ hours:
1. Discovering that most branches were quarantined
2. Creating selftest probes trying to find branches that would merge
3. Retrying with various "accounting" and "normalized" variants
4. Eventually landing only 16 branches through multiple retry passes

---

## Why the Merge Strategy Document Didn't Help

The `docs/MERGE_STRATEGY.md` we synthesized addresses the **git mechanics** (merge-tree, hub filtering, parallel buckets) but not the **policy bottleneck**:

1. **The strategy assumes branches are merge-eligible.** It focuses on conflict detection and parallel merging. But 88 branches were rejected before any git operation.

2. **The strategy doesn't address frozen-test verification.** The repo's `AGENTS.md` policy requires independent verification before merging branches that modify frozen assertions. No amount of parallelism helps if the gate is "human reviews 88 branches."

3. **The strategy's partition algorithm doesn't respect cross-bucket ancestry.** It groups by file overlap (correct for conflicts) but not by commit ancestry (critical for this repo's lane families).

---

## Recommended Actions

### Immediate (unblock the merge)

1. **Triage the 88 `frozen_assertion_deletion_review` branches:**
   - Classify: how many are RED→GREEN transitions (legitimate) vs actual test weakening?
   - Legitimate transitions: approve and add to eligible set
   - Spawn 20+ reviewer subagents to parallelize the classification

2. **Re-partition with ancestry-first bucketing:**
   ```
   For each branch B:
     Find all ancestors of B that are also in the merge set
     Place B and all its ancestors in the same bucket
     If bucket > 25, split by crate, keeping ancestor chains intact
   ```

3. **Order buckets by dependency graph:**
   ```
   bucket-07 (no dependencies) → first
   bucket-08 (no dependencies) → parallel with 07
   bucket-00 (depends on 07) → after 07
   bucket-01 (depends on 00, 02, 03, 07) → after those
   ...etc
   ```

### Strategic (prevent recurrence)

4. **Separate "verification review" from "merge execution":**
   - Phase A: 20+ agents review all 88 flagged branches, produce approve/reject per branch
   - Phase B: Merge only approved branches using the merge-tree engine
   - These phases can overlap: start merging approved branches while reviewing continues

5. **The 40 documentation-only branches:** Heritage-merge them (create merge commits that preserve ancestry without reintroducing file contents).

6. **Update `docs/MERGE_STRATEGY.md`** to add a "Policy Pre-clearance" phase before the partitioning phase.

---

## Summary

| Problem | Branches Affected | Fix |
|---|---|---|
| Frozen-test verification gate | 88 | Parallel reviewer agents to classify RED→GREEN |
| Already integrated (nothing to do) | 67+71 = 138 | Remove from work list |
| Documentation/bookkeeping only | 40 | Heritage-merge or skip |
| Cross-bucket ancestry violations | All 9 buckets | Re-partition with ancestry awareness |
| Debugging thrash (132 selftests) | N/A | Better manifest = no thrash needed |
| **Actually merged** | **16** | Working correctly, just starved of input |

**Bottom line:** The merge-sort tree engine works. It's starved of eligible branches because the analysis phase quarantines 93% of them. Fix the verification pipeline, fix the partitioning, and the existing engine can finish in under 2 hours.
