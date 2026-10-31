# Merge Orchestrator: State Machine Diagram

## High-Level Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                           PHASE BACKUP                           │
├─────────────────────────────────────────────────────────────────┤
│  git bundle create full-backup.bundle --all                     │
│  → Create backup bundle at worktree-base / full-backup.bundle   │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                           PHASE ANALYSIS                         │
├─────────────────────────────────────────────────────────────────┤
│  Parse manifest.json structure                                  │
│  Validate buckets array, count branches per bucket               │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                         PHASE L0-BUCKET                          │
├─────────────────────────────────────────────────────────────────┤
│  For each bucket:                                               │
│    ┌──────────────────────┐                                     │
│    │  Create worktree      │                                     │
│    │  from base branch    │                                     │
│    └──────────────────────┘                                     │
│         ↓                                                        │
│  For each branch in sorted order:                               │
│    ┌──────────────────────────────────────────────┐              │
│    │  git merge --no-ff branch                    │              │
│    │  ┌──────────┐                               │              │
│    │  │ Success  │  →  Record merge commit       │              │
│    │  └──────────┘                               │              │
│    │  ┌──────────┐                               │              │
│    │  │Conflict  │  →  Quarantine branch         │              │
│    │  └──────────┘                               │              │
│    └──────────────────────────────────────────────┘              │
│  Collect result JSON for bucket                                  │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                         PHASE L1-PAIR                           │
├─────────────────────────────────────────────────────────────────┤
│  Take L0 results, group in pairs                                │
│  Merge each pair sequentially:                                   │
│    ┌──────────────────────┐                                     │
│    │  git merge --no-ff   │                                     │
│    │  result1 + result2   │                                     │
│    └──────────────────────┘                                     │
│    ┌──────────┐                               │                 │
│    │ Success  │  →  Capture merge commit      │                 │
│    └──────────┘                               │                 │
│    ┌──────────┐                               │                 │
│    │Conflict  │  →  Quarantine pair           │                 │
│    └──────────┘                               │                 │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                   PHASE L2..LN (REPEAT)                         │
├─────────────────────────────────────────────────────────────────┤
│  While more than 1 result:                                       │
│    Batch results (MAX_PARALLELISM entries per batch)             │
│    Merge each batch sequentially                                 │
│    Tag level: merge-LN-complete                                  │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                        PHASE VERIFY                              │
├─────────────────────────────────────────────────────────────────┤
│  `cargo check` on final result branch                           │
│  Verify compilation success                                     │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                       PHASE REPORT                              │
├─────────────────────────────────────────────────────────────────┤
│  Summary:                                                        │
│    * Branches merged                                             │
│    * Branches quarantined (conflicts)                            │
│    * Failed merges                                               │
│  Tags created                                                    │
│  Success/Failure exit code                                      │
└─────────────────────────────────────────────────────────────────┘
```

## Detailed Level Operations

### Level 0 (Bucket Merging)

**State:**
- Each bucket has independent worktree
- Worktrees created from `BASE_BRANCH` (usually `main`)

**Input:** Manifest with 25 buckets × 20 branches each

**Output:** 25 L1-results (single merge commits per bucket or quaranteed)

**Conflict Handling:**
- Quarantine single branch
- Resume with next branch in merge order
- Log detailed conflict information
- Keep worktree for inspection

### Level 1 (Pairwise Merging)

**State:**
- Input: Pairwise L0 merge results
- Each pair processed sequentially (parallelism within pair is inherent)

**Input:** L0-results (25 results)

**Output:** ~13 L2-results

**Conflict Handling:**
- Quarantine entire pair
- Record conflicting files
- Keep worktree for inspection (if conflict)

### Level N (Batch Merging)

**State:**
- Input: Batch of merge results
- Maximum `MAX_PARALLELISM` results per batch
- Batches processed sequentially but in parallel with other batches at same level

**Input:** All L1 results (~13)

**Output:** Single final merge commit

**Conflict Handling:**
- Quarantined pair makes entire batch quarantined
- Detailed conflict reporting

## State Directory Structure

```
.worktree-base/
  └── .merge-state/
      ├── L0-results.json                 # All bucket merge results
      ├── L1-results.json                 # All pairwise merge results
      ├── L2-results.json                 # Final merge result
      ├── merge-L0-complete              # Git tag
      ├── merge-L1-complete              # Git tag
      ├── bucket-0.lock                  # Lock file (Linux-style)
      ├── bucket-1.pid                   # Process ID file
      ├── bucket-0.pid                   # Parent PID, worker PID
      ├── bucket-1.lock
      ├── bucket-1.pid
      └── ...
```

## Lock File Pattern

```
[bucket|pair|batch]-<ID>.lock
```

- Created before worker starts
- Removed on worker completion
- Guarantees no duplicate processing per item

## Process ID File Pattern

```
[bucket|pair|batch]-<ID>.pid
```

- Format: `parent_pid,worker_pid,level_id,payload`
- Parent PID: orchestrator process PID
- Worker PID: Actual merge worker PID
- Level ID: L0, L1, L2, etc.
- Payload: Additional context for recovery

## Recovery Flow

### Scenario: Orchestrator crash during Level N

1. **Identify crashed level:**
   ```bash
   git tag -l "merge-LN-complete"
   ```

2. **Locate incomplete results:**
   ```bash
   ls -la .merge-state/LN-results.json
   ```

3. **Resume from last successful level:**
   - Rename L(N-1)-results.json to appropriate input format
   - Modify orchestrator to skip forward levels
   - Restart with same manifest

### Scenario: Worker crashes during merge

1. **Identify affected bucket:**
   ```bash
   cat .merge-state/bucket-N.pid
   ```

2. **Check lock file:**
   ```bash
   ls -la .merge-state/bucket-N.lock
   ```

3. **Manual cleanup (if needed):**
   ```bash
   rm -f .merge-state/bucket-N.lock
   rm -f .merge-state/bucket-N.pid
   git worktree remove -f .merge-wtrees/merge-bucket-N
   ```

4. **Restart orchestrator:**
   ```
   ./merge_orchestrator.sh manifest.json worktree-base main
   ```
   Orchestrator automatically detects stale PIDs and skips completed items

## Conflict Reporting

### Quarantined Branch Format

```json
{
  "branch": "feature/topology-add-edge",
  "conflicts": 3,
  "files": "src/topology/mod.rs, src/graph/mod.rs, tests/test_topology.rs",
  "source": "upstream/opencode-v2-feature-slice",
  "base": "abc123def456"
}
```

### Quarantined Pair Format

```json
{
  "pairIndex": 5,
  "status": "QUARANTINED",
  "mergeCommit": null,
  "commit1": "abc123...",
  "commit2": "def456...",
  "conflictFiles": "src/protocol/stream.rs"
}
```

## Memory/CPU Safety

- Bounded parallelism: `MAX_PARALLELISM=25`
- Worktree cleanup: Automatic after success
- Persistent worktrees: Retained only for quarantined items
- Lock files prevent duplicate processing
- Per-bucket isolation guarantees