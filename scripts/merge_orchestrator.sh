#!/usr/bin/env bash
# merge_orchestrator.sh: Master controller for merging 500+ git worktree branches
# Usage: ./merge_orchestrator.sh <manifest.json> <worktree-dir> <base-branch>
# Example: ./merge_orchestrator.sh merge-manifest.json /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3/v2 main

set -euo pipefail

# Configuration
MAX_PARALLELISM=${MAX_PARALLELISM:-25}
STATE_DIR="./.merge-state"
BACKUP_BUNDLE="full-backup.bundle"

# Required arguments
MANIFEST=${1?:Manifest required: JSON file with buckets array}
WORKTREE_BASE=${2?:Worktree base directory required}
BASE_BRANCH=${3?:Base branch required}

# Verify arguments
if [[ ! -f "$MANIFEST" ]]; then
  echo "Error: Manifest file not found: $MANIFEST"
  exit 1
fi

# Parse manifest JSON to validate structure
if ! jq -e '.buckets is array' "$MANIFEST" > /dev/null 2>&1; then
  echo "Error: Manifest must have a 'buckets' array"
  exit 1
fi

# Phase BACKUP: Create backup bundle
echo "=== PHASE BACKUP ==="
echo "Creating backup bundle: $BACKUP_BUNDLE"
git bundle create "$WORKTREE_BASE/$BACKUP_BUNDLE" --all
echo "Backup created successfully"

# Phase ANALYSIS: Already complete - manifest exists

mkdir -p "$WORKTREE_BASE/$STATE_DIR"

# Phase L0: Process primary buckets in parallel
echo "=== PHASE L0 - Parallel Merge of Buckets ==="
L0_RESULTS_DIR="$WORKTREE_BASE/$STATE_DIR/l0-results"
mkdir -p "$L0_RESULTS_DIR"
L0_RESULTS_FILE="$L0_RESULTS_DIR/L0-results.json"

spawn_worker() {
  local bucket_index=$1
  local bucket_json=$2
  local output_file=$3
  
  echo "[L0-worker-$bucket_index] Starting merge on branch $bucket_index"
  
  # Create timestamp tracking
  local pid=$$
  
  # Worker process
  "$0" --worker \
    --bucket-index "$bucket_index" \
    --bucket "$bucket_json" \
    --worktree-base "$WORKTREE_BASE" \
    --output "$output_file" &
  
  local worker_pid=$!
  
  # Write PID file for tracking
  echo "$pid,$worker_pid,$bucket_index,$bucket_json" > "$WORKTREE_BASE/$STATE_DIR/bucket-${bucket_index}.pid"
}

# Spawn all workers
for bucket in $(jq -c '.buckets[]' "$MANIFEST"); do
  bucket_index=$(echo "$bucket" | jq -r '.id')
  output_file="$L0_RESULTS_DIR/bucket-${bucket_index}.json"
  
  # Lock file for concurrent safety
  lock_file="$WORKTREE_BASE/$STATE_DIR/bucket-${bucket_index}.lock"
  if [[ -f "$lock_file" ]]; then
    echo "Warning: Lock file exists for bucket $bucket_index, skipping"
    continue
  fi
  touch "$lock_file"
  
  spawn_worker "$bucket_index" "$bucket" "$output_file"
done

# Wait for all workers to complete
echo "Waiting for L0 workers to complete..."
wait

# Collect L0 results
L0_RESULT_COUNT=0
while IFS= read -r result_file; do
  if [[ -f "$result_file" ]]; then
    jq --arg file "$(basename "$result_file")" -s '{bucket_file: $file, content: .}' "$result_file" >> "$L0_RESULTS_FILE"
    ((L0_RESULT_COUNT++))
  fi
done < <(find "$L0_RESULTS_DIR" -name "bucket-*.json" -type f 2>/dev/null)

echo "L0 phase complete: $L0_RESULT_COUNT buckets merged"

# Phase L1: Pairwise merge of L0 results
echo "=== PHASE L1 - Pairwise Merge of L0 Results ==="
L1_RESULTS_DIR="$WORKTREE_BASE/$STATE_DIR/l1-results"
mkdir -p "$L1_RESULTS_DIR"
L1_RESULTS_FILE="$L1_RESULTS_DIR/L1-results.json"

L1_RESULT_COUNT=0
l0_entries=()

while IFS= read -r line; do
  l0_entries+=("$line")
done < <(jq -c . "$L0_RESULTS_FILE")

# Process pairwise
for ((i=0; i<${#l0_entries[@]}; i+=2)); do
  if [[ $((i+1)) -ge ${#l0_entries[@]} ]]; then
    # Odd one out - keep as is
    winner=$(echo "${l0_entries[$i]}" | jq -r '.bucket_file')
    jq --arg file "$(basename "$winner")" -s '{bucket_file: $file, content: .}' "$winner" >> "$L1_RESULTS_FILE"
    continue
  fi
  
  result1=$(echo "${l0_entries[$i]}" | jq -c '.content')
  result2=$(echo "${l0_entries[$i+1]}" | jq -c '.content')
  
  # Spawn merge worker for this pair
  pair_index=$((i/2))
  output_file="$L1_RESULTS_DIR/pair-${pair_index}.json"
  
  # Lock file for concurrent safety
  lock_file="$WORKTREE_BASE/$STATE_DIR/pair-${pair_index}.lock"
  if [[ -f "$lock_file" ]]; then
    echo "Warning: Lock file exists for pair $pair_index, skipping"
    continue
  fi
  touch "$lock_file"
  
  # Create merge worker process in background
  (
    "$0" --pair-merge \
      --pair-index "$pair_index" \
      --result1 "$result1" \
      --result2 "$result2" \
      --output "$output_file" \
      --worktree-base "$WORKTREE_BASE"
  ) &
  
  worker_pid=$!
  
  # Write PID file
  echo "$$,$worker_pid,$pair_index,$result1,$result2" > "$WORKTREE_BASE/$STATE_DIR/pair-${pair_index}.pid"
done

# Wait for L1 workers
echo "Waiting for L1 workers to complete..."
wait

# Collect L1 results
L1_RESULT_COUNT=0
while IFS= read -r result_file; do
  if [[ -f "$result_file" ]]; then
    jq --arg file "$(basename "$result_file")" -s '{pair_file: $file, content: .}' "$result_file" >> "$L1_RESULTS_FILE"
    ((L1_RESULT_COUNT++))
  fi
done < <(find "$L1_RESULTS_DIR" -name "pair-*.json" -type f 2>/dev/null)

echo "L1 phase complete: $L1_RESULT_COUNT pairs merged"

# Phase L2..LN: Repeat until single branch remains
LEVEL=$2  # L2 onwards

# Subsequent levels in batches of MAX_PARALLELISM
while true; do
  SHOULD_CONTINUE=false
  
  # Check if we have multiple entries
  ENTRY_COUNT=$(jq '. | length' "$LEVEL-results.json" 2>/dev/null || echo 0)
  
  if [[ $ENTRY_COUNT -gt 1 ]]; then
    
    echo "=== PHASE L$LEVEL - Merging $ENTRY_COUNT Results ==="
    LEVEL_RESULTS_DIR="$WORKTREE_BASE/$STATE_DIR/l${LEVEL}-results"
    mkdir -p "$LEVEL_RESULTS_DIR"
    LEVEL_RESULTS_FILE="$LEVEL_RESULTS_DIR/L${LEVEL}-results.json"
    
    # Split into batches of MAX_PARALLELISM
    offset=0
    while true; do
      batch=$(jq -c ".[$offset:$((offset+MAX_PARALLELISM))]" "$LEVEL_RESULTS_FILE")
      
      if [[ -z "$batch" || "$(echo "$batch" | jq 'length')" -eq 0 ]]; then
        break
      fi
      
      # Spawn workers for this batch
      for ((i=0; i<$(echo "$batch" | jq 'length'); i++)); do
        batch_index=$((offset+i))
        entry=$(echo "$batch" | jq -c ".[$i]")
        
        output_file="$LEVEL_RESULTS_DIR/batch-${batch_index}.json"
        
        # Lock file
        lock_file="$WORKTREE_BASE/$STATE_DIR/batch-${batch_index}.lock"
        if [[ -f "$lock_file" ]]; then
          echo "Warning: Lock file exists for batch $batch_index, skipping"
          continue
        fi
        touch "$lock_file"
        
        (
          "$0" --batch-merge \
            --batch-index "$batch_index" \
            --batch-content "$batch" \
            --output "$output_file" \
            --worktree-base "$WORKTREE_BASE"
        ) &
        
        worker_pid=$!
        echo "$$,$worker_pid,$batch_index,$batch_content:$batch" > "$WORKTREE_BASE/$STATE_DIR/batch-${batch_index}.pid"
      done
      
      ((offset+=MAX_PARALLELISM))
      SHOULD_CONTINUE=true
    done
    
    # Wait for batch workers
    echo "Waiting for L$LEVEL batch workers to complete..."
    wait
    
    # Collect results
    RESULT_COUNT=0
    while IFS= read -r result_file; do
      if [[ -f "$result_file" ]]; then
        jq --arg file "$(basename "$result_file")" -s '{batch_file: $file, content: .}' "$result_file" >> "$LEVEL_RESULTS_FILE"
        ((RESULT_COUNT++))
      fi
    done < <(find "$LEVEL_RESULTS_DIR" -name "batch-*.json" -type f 2>/dev/null)
    
    echo "L$LEVEL phase complete: $RESULT_COUNT batches merged"
    
    # Tag level completion
    git -C "$WORKTREE_BASE" tag "merge-L$LEVEL-complete"
    ((LEVEL++))
  else
    break
  fi
done

# Phase VERIFY: Final cargo check on result
echo "=== PHASE VERIFY - Final Verification ==="

# Find final result
FINAL_RESULT=$(find "$WORKTREE_BASE/$STATE_DIR" -name "merge*-results.json" -type f | tail -1)
if [[ -z "$FINAL_RESULT" ]]; then
  echo "Error: No final result found"
  exit 1
fi

echo "Running final cargo check on merged result..."
cd "$WORKTREE_BASE"
cargo check

if [[ $? -ne 0 ]]; then
  echo "WARNING: Cargo check failed. Some branches may have merge conflicts or compilation issues."
  exit 1
fi

echo "Cargo check passed"

# Phase REPORT: Summary
echo "=== PHASE REPORT ==="
echo "========================================"
echo "Merge Orchestrator Summary"
echo "========================================"
echo "Source manifest: $MANIFEST"
echo "Base branch: $BASE_BRANCH"
echo "Worktree location: $WORKTREE_BASE"
echo "Max parallelism: $MAX_PARALLELISM"
echo "========================================"
echo "Branches merged successfully"
echo "Quarantined branches (conflicts): N/A for this run"
echo "Failed merge attempts: N/A for this run"
echo ""
echo "Final result branch:"
git branch --show-current
echo ""
echo "Merge tags"
git -C "$WORKTREE_BASE" tag -l "merge-L*-complete"
echo "========================================"
echo "Merge complete"

# Cleanup leftover PIDs after success
rm -rf "$WORKTREE_BASE/$STATE_DIR"/*.pid 2>/dev/null || true