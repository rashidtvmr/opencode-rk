# Merge Orchestrator - Usage Examples

## Prerequisites

```bash
# Install required tools
brew install jq                      # JSON processing
git --version                        # Git 2.30 or later
make                                 # Build tool (required for verify)
```

## Basic Usage

### 1. Starting the Merge Orchestration

```bash
./scripts/merge_orchestrator.sh \
  ./scripts/merge-manifest.json \
  /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3 \
  main
```

This will:
- Create a backup bundle at the worktree base
- Parse the manifest to get 25 buckets of 20 branches each
- Start 25 parallel workers (one per bucket)
- Wait for all L0 workers to complete
- Merge results pairwise in Level 1
- Continue merging until single branch remains
- Run `cargo check` on the final result
- Generate a summary report

### 2. With Custom Parallelism

```bash
MAX_PARALLELISM=50 \
./scripts/merge_orchestrator.sh \
  ./scripts/merge-manifest.json \
  /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3 \
  main
```

### 3. With Logging

```bash
MAX_PARALLELISM=25 \
LOG_DIR="/tmp/merge-logs-$(date +%Y%m%d-%H%M%S)" \
LOG_LEVEL=verbose \
./scripts/merge_orchestrator.sh \
  ./scripts/merge-manifest.json \
  /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3 \
  main 2>&1 | tee "$LOG_DIR/merge-complete.log"
```

## Resume from Failure

### Detect Last Complete Level

```bash
# Check which levels completed
git tag -l "merge-L*-complete" | sed 's/merge-L//' | sed 's/-complete//' | sort -n

# Example output:
# 1
# 5
# 12
# 25
# This indicates L1, L5, L12, and L25 completed. L25 is the highest, but we should resume from the next level that wasn't complete.
```

### Resume Strategy

```bash
# Scenario: Orchestrator crashed at Phase L3

# 1. Check current state
git tag -l "merge-L3-complete"

# 2. If not tagged, need to resume from L0
# Delete all incomplete state and restart
rm -rf /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3/.merge-state
rm -rf /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3/.merge-wtrees

# 3. Restart full orchestration
MAX_PARALLELISM=25 \
./scripts/merge_orchestrator.sh \
  ./scripts/merge-manifest.json \
  /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3 \
  main
```

## Manual Inspection

### View Merge Results

```bash
# View L0 bucket results
jq '.' /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3/.merge-state/L0-results.json

# View quarantined branches from a specific bucket
jq '.[] | select(.status == "QUARANTINED") | .quarantined' \
  /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3/.merge-state/L0-results.json
```

### Inspect Quarantined Worktrees

```bash
# List all worktrees
git worktree list

# Each quarantined worktree will be named:
# - merge-bucket-0
# - merge-bucket-1
# - merge-pair-0
# - merge-batch-0
# etc.

# See conflicted files in a worktree
cd .merge-wtrees/merge-bucket-0
git diff --name-only
```

## Manual Conflict Resolution

### Resolving a Conflicted Branch

```bash
# 1. Enter worktree for quarantined worktree
cd /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3/.merge-wtrees/merge-bucket-0

# 2. Create resolution branch
git checkout main
git checkout -b resolved-feature-topology-add-edge

# 3. Manually resolve conflicts
# Edit conflict files
vim src/topology/mod.rs

# 4. Complete the merge
git add .
git commit -m "Resolve conflicts in topology: add edge, remove node"

# 5. Update quarantine tracking
cd /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3

# 6. Recalculate quarantine list
# Keep the branch out of the merge order list for next run

# 7. Clean up resolved worktree
cd /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3/.merge-wtrees/merge-bucket-0
git worktree remove -f .
```

### Quickest Restart After Conflicts

```bash
# After manually resolving a branch:
# 1. Move the resolved branch to a separate queue
./scripts/resolve-conflicts.sh

# 2. Update manifest with resolved branches
# 3. Resume:
MAX_PARALLELISM=25 \
./scripts/merge_orchestrator.sh \
  ./scripts/merge-manifest-resolved.json \
  /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3 \
  main
```

## Cleanup

### After Successful Merge

```bash
# All worktrees already cleaned up by orchestrator
# Only state directory remains

# View final result
git checkout main
git log --oneline --graph --all --decorate

# Tag the final result
git tag -a v2-merged-$(date +%Y%m%d-%H%M%S) -m "Automated merge of 500+ branches"

# Remove state directory (optional, for cleanup)
rm -rf .merge-state
```

### Manual Cleanup After Failure

```bash
# Stop all orphaned workers
pkill -f merge_worker.sh

# Clean up state
rm -rf .merge-state
rm -rf .merge-wtrees

# Also remove all orphaned worktrees
git worktree list | awk '{print $2}' | xargs -I {} git worktree remove -f {}

# Re-create backup from scratch
cd /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3
git bundle create full-backup.bundle --all
```

## Event Logging

### Parallel Worker Logs

```bash
# Worker logs go to console output during orchestration
# For persistent logs:
LOG_LEVEL=verbose \
WORKTREE_BASE=/Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3 \
./scripts/merge_orchestrator.sh \
  ./scripts/merge-manifest.json \
  /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3 \
  main \
  | tee merge-run-$(date +%Y%m%d-%H%M%S).log
```

### Worker Output Files

```bash
# Each worker writes its results to:
ls -lh /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3/.merge-state/l0-results/
ls -lh /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3/.merge-state/l1-results/

# Subset for inspection
jq '.[] | select(.status == "QUARANTINED")' .merge-state/l0-results.json
```

## Integration with CI/CD

### GitHub Actions Example

```yaml
name: Merge Worktrees

on:
  workflow_dispatch:
    inputs:
      worktree_dir:
        description: 'Worktree directory'
        required: true
        default: '/Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3'

jobs:
  merge:
    runs-on: ubuntu-latest
    steps:
      - name: Check out repo
        uses: actions/checkout@v3
        with:
          fetch-depth: 0

      - name: Install dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y jq git

      - name: Start merge orchestration
        env:
          MAX_PARALLELISM: 25
        run: |
          chmod +x scripts/merge_orchestrator.sh
          chmod +x scripts/merge_worker.sh
          ./scripts/merge_orchestrator.sh \
            scripts/merge-manifest.json \
            /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3 \
            main
```

### GitLab CI Example

```yaml
merge_worktrees:
  image: alpine:latest
  before_script:
    - apk add --no-cache jq git
    - chmod +x scripts/merge_orchestrator.sh
    - chmod +x scripts/merge_worker.sh
  script:
    - |
      ./merge_orchestrator.sh \
        ./merge-manifest.json \
        /ci/worktree-base \
        main
  artifacts:
    paths:
      - .merge-state/
      - merge-logs/
```

## Troubleshooting

### "Manifest must have a 'buckets' array"

```bash
# Validate manifest syntax
jq empty ./merge-manifest.json
jq '.[:100]' ./merge-manifest.json  # Print first 100 chars
```

### "Worktree already exists"

```bash
# This is normal in resume scenarios
# If it's stale, remove it manually:
cd <worktree-base>
git worktree remove -f .merge-wtrees/merge-bucket-0
```

### "Permission denied" on lock files

```bash
# Fix lock files
find . -name "*.lock" -type f -delete
find . -name "*.pid" -type f -delete
```

### Orphaned workers consuming resources

```bash
# Find and kill orphaned workers
ps aux | grep merge_worker | awk '{print $2}' | xargs kill -9

# Clean up state
rm -rf .merge-state .merge-wtrees
git worktree prune
```

## Production Checklist

Before running on production merge:

- [ ] Manifest JSON is validated with `jq empty`
- [ ] Worktree base directory has sufficient disk space (minimum 50GB)
- [ ] `MAX_PARALLELISM` is appropriate for system resources
- [ ] Backup bundle was successfully created
- [ ] Git remote has sufficient bandwidth
- [ ] Orchestration logs are captured for audit
- [ ] Known conflicts documented in quarantine report
- [ ] Manual rollback plan documented

## Success Metrics

Successful merge confirms:

1. All levels completed (levels L0..LN tagged)
2. Final `cargo check` passed
3. `merge-LN-complete` tag exists
4. No zombie workers running
5. Disk space usage returned to normal
6. Quarantine report documents all conflicts
7. Final result is on `main` or target branch

## Additional Resources

- `scripts/MERGE_STATE_MACHINE.md` - Detailed state machine and recovery flow
- `scripts/merge-manifest.json` - Example manifest format
- `.plan/tasks/canonical/` - Full task specifications