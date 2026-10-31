#!/usr/bin/env bash
# merge_worker.sh: Individual worker for merging branches in a bucket
# Usage: ./merge_worker.sh --worker [--bucket-index <idx>] [--bucket <json>] [--worktree-base <dir>] [--output <file>]
# Usage: ./merge_worker.sh --pair-merge [--pair-index <idx>] [--result1 <json>] [--result2 <json>] [--output <file>]
# Usage: ./merge_worker.sh --batch-merge [--batch-index <idx>] [--batch-content <json>] [--output <file>]

set -euo pipefail

MODE=${1:-worker}

# Worker mode
if [[ "$MODE" == "--worker" ]]; then
  BUCKET_INDEX=${2?:Bucket index required}
  BUCKET=${3?:Bucket JSON required}
  WORKTREE_BASE=${4?:Worktree base directory required}
  OUTPUT=${5?:Output file required}
  
  mkdir -p "$WORKTREE_BASE"
  
  echo "[Worker-$BUCKET_INDEX] Starting merge operation"
  
  # Create timestamp directory
  TIMESTAMP_DIR="$WORKTREE_BASE/.merge-timestamps"
  mkdir -p "$TIMESTAMP_DIR"
  
  START_TIME=$(date +"%Y-%m-%d %H:%M:%S")
  
  # Create worktree from base
  WORKTREE_NAME="merge-bucket-${BUCKET_INDEX}"
  WORKTREE_PATH="$WORKTREE_BASE/$WORKTREE_NAME"
  
  echo "[Worker-$BUCKET_INDEX] Creating worktree at $WORKTREE_PATH"
  git -C "$WORKTREE_BASE" worktree add -b "$WORKTREE_NAME" "$WORKTREE_PATH" main || echo "Worktree might already exist"
  
  cd "$WORKTREE_PATH"
  
  # Get merge order from bucket (sort by base commit first, then by branch name)
  MERGE_ORDER=$(echo "$BUCKET" | jq -c '[
    .branches | 
    sort_by(.baseCommit, .branchName) |
    .[] |
    {baseCommit: .baseCommit, branchName: .branchName, source: .source}
  ]')
  
  echo "[Worker-$BUCKET_INDEX] Merge order: $(echo "$MERGE_ORDER" | jq -r '[.[] | .branchName] | join(", ")')"
  
  MERGE_COMMIT=""
  QUARANTINED=()
  CONFLICT_DETAILS=""
  
  # Process each branch in merge order
  COUNT=0
  TOTAL=$(echo "$BUCKET" | jq '.branches | length')
  
  while IFS= read -r branch_info; do
    ((COUNT++))
    branch_name=$(echo "$branch_info" | jq -r '.branchName')
    base_commit=$(echo "$branch_info" | jq -r '.baseCommit')
    source=$(echo "$branch_info" | jq -r '.source')
    
    echo "[Worker-$BUCKET_INDEX] Processing branch $COUNT/$TOTAL: $branch_name (base: $base_commit)"
    
    # Create temporary file for conflict details
    CONFLICT_FILE="/tmp/merge-conflict-$BUCKET_INDEX-$COUNT-$$.txt"
    
    # Attempt merge with conflict logging
    if git merge --no-ff --no-commit "$branch_name" 2> >(tee "$CONFLICT_FILE"); then
      # Merge succeeded
      MERGE_COMMIT=$(git rev-parse HEAD)
      echo "[Worker-$BUCKET_INDEX] Successfully merged $branch_name into $MERGE_COMMIT"
      
      # Determine file status
      FILES_CHANGED=$(git diff --name-only --cached | wc -l | tr -d ' ')
      echo "[Worker-$BUCKET_INDEX] Files changed: $FILES_CHANGED"
    else
      # Merge failed or had conflicts
      if git merge --abort 2>/dev/null; then
        # Conflicts found - quarantine this branch
        CONFLICT_COUNT=$(grep -c "CONFLICT" "$CONFLICT_FILE" 2>/dev/null || echo 0)
        CONFLICT_FILES=$(git diff --name-only 2>/dev/null || echo "none")
        
        echo "[Worker-$BUCKET_INDEX] CONFLICT: $branch_name - Found $CONFLICT_COUNT conflicts"
        
        if [[ "$CONFLICT_FILES" != "none" ]]; then
          CONFLICT_DETAILS="$branch_name conflicts with files: $CONFLICT_FILES"
        fi
        
        QUARANTINED+=("{\"branch\": \"$branch_name\", \"conflicts\": $CONFLICT_COUNT, \"files\": \"$CONFLICT_FILES\",\"source\":\"$source\",\"base\":\"$base_commit\"}")
      else
        # Merge error, not just conflicts
        echo "[Worker-$BUCKET_INDEX] ERROR: Failed to merge $branch_name"
        QUARANTINED+=("{\"branch\": \"$branch_name\", \"error\": \"merge error\",\"source\":\"$source\",\"base\":\"$base_commit\"}")
      fi
    fi
    
    rm -f "$CONFLICT_FILE"
  done < <(echo "$MERGE_ORDER" | jq -c '.[]')
  
  # Prepare result
  RESULT=$(jq -n \
    --arg bucketIndex "$BUCKET_INDEX" \
    --arg status "${#QUARANTINED[@]} -gt 0 ? \"QUARANTINED\" : \"SUCCESS\"" \
    --argjson mergeCommit "$MERGE_COMMIT" \
    --argjson quarantined "$(printf '%s\n' "${QUARANTINED[@]}" | jq -s '.') \
    --arg conflictDetails "$CONFLICT_DETAILS" \
    --arg startTime "$START_TIME" \
    --arg endTime "$(date +"%Y-%m-%d %H:%M:%S")" \
    {
      bucketIndex: $BUCKET_INDEX,
      status: $status,
      mergeCommit: ($MERGE_COMMIT // null),
      quarantined: $quarantined,
      conflictDetails: $conflictDetails,
      startTime: $startTime,
      endTime: $endTime,
      branchesProcessed: $COUNT,
      branchesTotal: $TOTAL,
      branchesQuarantined: (length($quarantined))
    })
  
  echo "$RESULT" > "$OUTPUT"
  echo "[Worker-$BUCKET_INDEX] Output written to $OUTPUT"
  
  # Cleanup worktree (keep it if there were quarantined branches)
  if [[ "${#QUARANTINED[@]}" -eq 0 ]]; then
    echo "[Worker-$BUCKET_INDEX] Cleaning up worktree (no conflicts)"
    git -C "$WORKTREE_BASE" worktree remove -f "$WORKTREE_PATH"
  else
    echo "[Worker-$BUCKET_INDEX] Keeping worktree for inspection: $WORKTREE_PATH"
  fi
  
# Pair-merge mode
elif [[ "$MODE" == "--pair-merge" ]]; then
  PAIR_INDEX=${2?:Pair index required}
  RESULT1=${3?:Result 1 JSON required}
  RESULT2=${4?:Result 2 JSON required}
  OUTPUT=${5?:Output file required}
  WORKTREE_BASE=${6:-./.merge-wtrees}
  
  mkdir -p "$WORKTREE_BASE"
  
  echo "[PairWorker-$PAIR_INDEX] Starting pairwise merge"
  
  # Extract merge commits
  COMMIT1=$(echo "$RESULT1" | jq -r '.mergeCommit // empty')
  COMMIT2=$(echo "$RESULT2" | jq -r '.mergeCommit // empty')
  
  if [[ -z "$COMMIT1" ]] || [[ -z "$COMMIT2" ]]; then
    echo "[PairWorker-$PAIR_INDEX] ERROR: Both inputs must have merge commits"
    exit 1
  fi
  
  TIMESTAMP_DIR="$WORKTREE_BASE/.merge-timestamps"
  mkdir -p "$TIMESTAMP_DIR"
  
  START_TIME=$(date +"%Y-%m-%d %H:%M:%S")
  
  # Create worktree for merging
  WORKTREE_NAME="merge-pair-$PAIR_INDEX"
  WORKTREE_PATH="$WORKTREE_BASE/$WORKTREE_NAME"
  
  echo "[PairWorker-$PAIR_INDEX] Creating worktree at $WORKTREE_PATH"
  git -C "$WORKTREE_BASE" worktree add -b "$WORKTREE_NAME" "$WORKTREE_PATH" main || echo "Worktree might already exist"
  
  cd "$WORKTREE_PATH"
  
  # Start from result1
  echo "[PairWorker-$PAIR_INDEX] Checking out $COMMIT1"
  git checkout "$COMMIT1"
  
  # Merge result2
  echo "[PairWorker-$PAIR_INDEX] Merging $COMMIT2"
  
  MERGE_COMMIT=""
  CONFLICT_FILES=""
  
  if git merge --no-ff --no-commit "$COMMIT2" 2> >(tee "/tmp/pair-conflict-$PAIR_INDEX-$$.txt"); then
    MERGE_COMMIT=$(git rev-parse HEAD)
    CONFLICT_FILES=$(git diff --name-only --cached | tr '\n' ',' | sed 's/,$//')
    echo "[PairWorker-$PAIR_INDEX] SUCCESS: Merged $COMMIT1 + $COMMIT2 into $MERGE_COMMIT"
  else
    if git merge --abort 2>/dev/null; then
      CONFLICT_FILES=$(git diff --name-only 2>/dev/null || echo "none")
      echo "[PairWorker-$PAIR_INDEX] CONFLICT: $COMMIT2 conflicts with: $CONFLICT_FILES"
    else
      echo "[PairWorker-$PAIR_INDEX] ERROR: Failed to merge $COMMIT2"
      exit 2
    fi
  fi
  
  rm -f "/tmp/pair-conflict-$PAIR_INDEX-$$.txt"
  
  # Prepare result
  RESULT=$(jq -n \
    --arg pairIndex "$PAIR_INDEX" \
    --arg status "$CONFLICT_FILES" == "none" ? "SUCCESS" : "QUARANTINED" \
    --arg mergeCommit "$MERGE_COMMIT" \
    --arg commit1 "$COMMIT1" \
    --arg commit2 "$COMMIT2" \
    --arg conflictFiles "$CONFLICT_FILES" \
    --arg startTime "$START_TIME" \
    --arg endTime "$(date +"%Y-%m-%d %H:%M:%S")" \
    {
      pairIndex: $PAIR_INDEX,
      status: $status,
      mergeCommit: ($MERGE_COMMIT // null),
      commit1: 
{
",
      commit2: 
{
",
      conflictFiles: $conflictFiles,
      startTime: $startTime,
      endTime: $endTime
    })
  
  echo "$RESULT" > "$OUTPUT"
  echo "[PairWorker-$PAIR_INDEX] Output written to $OUTPUT"
  
  # Cleanup
  echo "[PairWorker-$PAIR_INDEX] Cleaning up worktree"
  git -C "$WORKTREE_BASE" worktree remove -f "$WORKTREE_PATH"
  
# Batch-merge mode
elif [[ "$MODE" == "--batch-merge" ]]; then
  BATCH_INDEX=${2?:Batch index required}
  BATCH_CONTENT=${3?:Batch content JSON required}
  OUTPUT=${4?:Output file required}
  WORKTREE_BASE=${5:-./.merge-wtrees}
  
  mkdir -p "$WORKTREE_BASE"
  cd "$WORKTREE_BASE"
  
  echo "[BatchWorker-$BATCH_INDEX] Starting batch merge of $(echo "$BATCH_CONTENT" | jq '. | length') results"
  
  # Extract first commit as starting point
  FIRST_COMMIT=$(echo "$BATCH_CONTENT" | jq -r '.[0] | .mergeCommit')
  
  if [[ -z "$FIRST_COMMIT" ]]; then
    echo "[BatchWorker-$BATCH_INDEX] ERROR: First item must have mergeCommit"
    exit 1
  fi
  
  # Start new worktree for batch merge
  WORKTREE_NAME="merge-batch-$BATCH_INDEX"
  WORKTREE_PATH="$WORKTREE_BASE/$WORKTREE_NAME"
  
  echo "[BatchWorker-$BATCH_INDEX] Creating worktree at $WORKTREE_PATH"
  git worktree add -b "$WORKTREE_NAME" "$WORKTREE_PATH" main || echo "Worktree might already exist"
  
  cd "$WORKTREE_PATH"
  
  # Check out first commit
  echo "[BatchWorker-$BATCH_INDEX] Checking out $FIRST_COMMIT"
  git checkout "$FIRST_COMMIT"
  
  # Merge remaining items in batch
  PASSED=0
  FAILED=0
  
  while IFS= read -r batch_item; do
    merge_commit=$(echo "$batch_item" | jq -r '.mergeCommit')
    
    if [[ -z "$merge_commit" ]]; then
      echo "[BatchWorker-$BATCH_INDEX] Skipping item without mergeCommit: $batch_item"
      ((FAILED++))
      continue
    fi
    
    echo "[BatchWorker-$BATCH_INDEX] Merging $merge_commit"
    
    if git merge --no-ff --no-commit "$merge_commit" 2> >(tee "/tmp/batch-conflict-$BATCH_INDEX-$PASSED-$$.txt"); then
      PASSED=$((PASSED+1))
      echo "[BatchWorker-$BATCH_INDEX] SUCCESS: Merged $merge_commit"
    else
      FAILED=$((FAILED+1))
      CONFLICT_FILES=$(git diff --name-only 2>/dev/null || echo "none")
      echo "[BatchWorker-$BATCH_INDEX] CONFLICT or ERROR with $merge_commit"
      rm -f "/tmp/batch-conflict-$BATCH_INDEX-$PASSED-$$.txt"
    fi
  done < <(echo "$BATCH_CONTENT" | jq -c '.[1:]')  # Skip first item (already checked out)
  
  rm -f "/tmp/batch-conflict-$BATCH_INDEX-$PASSED-$$.txt"
  
  MERGE_COMMIT=""
  CONFLICT_FILES=""
  
  if git merge --no-ff --no-commit "$FIRST_COMMIT" 2> >(tee "/tmp/batch-final-$BATCH_INDEX-$$.txt"); then
    MERGE_COMMIT=$(git rev-parse HEAD)
    CONFLICT_FILES=$(git diff --name-only --cached | tr '\n' ',' | sed 's/,$//')
  fi
  
  rm -f "/tmp/batch-final-$BATCH_INDEX-$$.txt"
  
  # Prepare result
  RESULT=$(jq -n \
    --arg batchIndex "$BATCH_INDEX" \
    --arg status "$CONFLICT_FILES" == "none" && "$FAILED" -eq 0 ? "SUCCESS" : "QUARANTINED" \
    --arg mergeCommit "$MERGE_COMMIT" \
    --arg passed "$PASSED" \
    --arg failed "$FAILED" \
    --arg conflictFiles "$CONFLICT_FILES" \
    --arg startTime "$(date +"%Y-%m-%d %H:%M:%S")" \
    --arg endTime "$(date +"%Y-%m-%d %H:%M:%S")" \
    {
      batchIndex: $BATCH_INDEX,
      status: $status,
      mergeCommit: ($MERGE_COMMIT // null),
      passed: $PASSED,
      failed: $FAILED,
      conflictFiles: $conflictFiles,
      startTime: $startTime,
      endTime: $endTime
    })
  
  echo "$RESULT" > "$OUTPUT"
  
  echo "[BatchWorker-$BATCH_INDEX] Batch complete: $PASSED passed, $FAILED failed"
  echo "[BatchWorker-$BATCH_INDEX] Output: $OUTPUT"
  
  # Cleanup
  echo "[BatchWorker-$BATCH_INDEX] Cleaning up worktree"
  git worktree remove -f "$WORKTREE_PATH"
  
else
  echo "Usage:"
  echo "  $0 --worker [--bucket-index <N>] [--bucket <json>] [--worktree-base <dir>] [--output <file>]"
  echo "  $0 --pair-merge [--pair-index <N>] [--result1 <json>] [--result2 <json>] [--output <file>]"
  echo "  $0 --batch-merge [--batch-index <N>] [--batch-content <json>] [--output <file>]"
  exit 1
fi