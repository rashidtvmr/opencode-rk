#!/usr/bin/env bash
# merge-validate.sh: Validates merge scripts against requirements
# Usage: ./merge-validate.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ERRORS=0

echo "=== Merge Orchestration Validation ==="
echo ""

# Checklist
echo "REQUIREMENT CHECKLIST"
echo "====================="
echo ""

# 1. Required files exist
echo "1. File Presence"
echo "----------------"
FILES=(
  "merge_orchestrator.sh"
  "merge_worker.sh"
  "merge-manifest.json"
  "merge-test.sh"
  "MERGE_STATE_MACHINE.md"
  "MERGE_USAGE.md"
  "README.md"
)

for file in "${FILES[@]}"; do
  if [[ -f "$SCRIPT_DIR/$file" ]]; then
    LINES=$(wc -l < "$SCRIPT_DIR/$file")
    printf "%-30s %8d lines ✓\n" "$file:" "$LINES"
  else
    printf "%-30s MISSING ✗\n" "$file:"
    ((ERRORS++))
  fi
done

# 2. Script syntax validation
echo ""
echo "2. Script Syntax"
echo "----------------"
if bash -n "$SCRIPT_DIR/merge_orchestrator.sh" 2>/dev/null; then
  printf "%-30s PASS ✓\n" "merge_orchestrator.sh"
else
  printf "%-30s FAIL ✗\n" "merge_orchestrator.sh"
  ((ERRORS++))
fi

if bash -n "$SCRIPT_DIR/merge_worker.sh" 2>/dev/null; then
  printf "%-30s PASS ✓\n" "merge_worker.sh"
else
  printf "%-30s FAIL ✗\n" "merge_worker.sh"
  ((ERRORS++))
fi

# 3. Required tools
echo ""
echo "3. Required Tools"
echo "-----------------"
REQUIRED=(
  "jq"
  "git"
  "make"
)

for tool in "${REQUIRED[@]}"; do
  if command -v "$tool" >/dev/null 2>&1; then
    VERSION=$(eval "$tool" --version 2>/dev/null | head -1 || echo "installed")
    printf "%-30s %s ✓\n" "$tool:" "$VERSION"
  else
    printf "%-30s MISSING ✗\n" "$tool:"
    ((ERRORS++))
  fi
done

# 4. Function signatures
echo ""
echo "4. Required Functions"
echo "----------------------"
FUNC_CHECKS=(
  "function_name:spawn_worker"
  "function_name:PHASE BACKUP"
  "function_name:PHASE ANALYSIS"
  "function_name:PHASE L0"
  "function_name:git merge --no-ff"
  "function_name:WORKTREE cleanup"
  "function_name:Lock file pattern"
)

for item in "${FUNC_CHECKS[@]}"; do
  ITEM=$(echo "$item" | cut -d: -f1)
  PATTERN=$(echo "$item" | cut -d: -f2)
  if grep -q "$PATTERN" "$SCRIPT_DIR/merge_orchestrator.sh"; then
    printf "%-30s Present ✓\n" "$ITEM:"
  else
    printf "%-30s MISSING ✗\n" "$ITEM:"
    ((ERRORS++))
  fi
done

# 5. Worker modes
echo ""
echo "5. Worker Modes"
echo "----------------"
MODES=(
  "must support worker mode"
  "must support pair-merge mode"
  "must support batch-merge mode"
)

for mode in "${MODES[@]}"; do
  if grep -q "MODE=\${1:-.*}" "$SCRIPT_DIR/merge_worker.sh" && \
     grep -q "\$MODE.*--\|MODE=--" "$SCRIPT_DIR/merge_worker.sh"; then
    printf "%-30s Implemented ✓\n" "Worker modes:"
  else
    printf "%-30s MISSING ✗\n" "Worker modes:"
    ((ERRORS++))
  fi
done

# 6. JSON processing
echo ""
echo "6. JSON Integration"
echo "-------------------"
JSON_CHECKS=(
  "jq used for manifest parsing"
  "JSON result format includes status"
  "JSON result includes mergeCommit"
  "JSON result includes quarantined"
  "jq -c used for compact output"
)

for check in "${JSON_CHECKS[@]}"; do
  if grep -qE "$check" "$SCRIPT_DIR/merge_worker.sh" "$SCRIPT_DIR/merge_orchestrator.sh" 2>/dev/null; then
    printf "%-30s OK ✓\n" "$check"
  else
    printf "%-30s MISSING ✗\n" "$check"
    ((ERRORS++))
  fi
done

# 7. Error handling
echo ""
echo "7. Error Handling"
echo "-----------------"
ERROR_MANAGERS=(
  "set -euo pipefail"
  "Error messages present"
  "Lock files created"
  "Lock files removed"
  "Worktrees deleted"
)

for item in "${ERROR_MANAGERS[@]}"; do
  if eval "grep -qE '$item'" "$SCRIPT_DIR/merge_worker.sh" "$SCRIPT_DIR/merge_orchestrator.sh" 2>/dev/null; then
    printf "%-30s Present ✓\n" "$item"
  else
    printf "%-30s Missing ✗\n" "$item"
    ((ERRORS++))
  fi
done

# 8. Documentation completeness
echo ""
echo "8. Documentation"
echo "----------------"
DOC_CHECKS=(
  "Quick start section in README"
  "CI/CD examples in MERGE_USAGE"
  "State diagram in MERGE_STATE_MACHINE"
  "Resume procedures documented"
  "Conflict handling examples"
)

for check in "${DOC_CHECKS[@]}"; do
  if eval "grep -qE '$check' $SCRIPT_DIR/*.md 2>/dev/null"; then
    printf "%-30s Included ✓\n" "$check"
  else
    printf "%-30s Missing ✗\n" "$check"
    ((ERRORS++))
  fi
done

# 9. Safety mechanisms
echo ""
echo "9. Safety Mechanisms"
echo "--------------------"
SAFETY=(
  "Backup bundle created"
  "Lock files for concurrency"
  "PID files for timing"
  "Worktree isolation"
  "Verification step included"
)

for item in "${SAFETY[@]}"; do
  if eval "grep -qE '$item' $SCRIPT_DIR/merge_orchestrator.sh 2>/dev/null"; then
    printf "%-30s Implemented ✓\n" "$item"
  else
    printf "%-30s Missing ✗\n" "$item"
    ((ERRORS++))
  fi
done

# 10. Total validation
TOTAL_CHECKS=40
echo ""
echo "=========================================================================="
echo "VALIDATION SUMMARY"
echo "=========================================================================="
PASSED=$((TOTAL_CHECKS - ERRORS))
FAILED=$ERRORS

echo "Total checks: $TOTAL_CHECKS"
printf "Passed: %d ✓\n" "$PASSED"
printf "Failed: %d ✗\n" "$FAILED"

if [[ $FAILED -eq 0 ]]; then
  echo ""
  echo "=== ALL VALIDATIONS PASSED! ==="
  echo ""
  echo "Next steps:"
  echo "  1. Run test suite: ./merge-test.sh"
  echo "  2. Execute small-scale test: ./merge-orchestrator.sh [args]"
  echo "  3. Scale up to production repository"
  echo ""
  exit 0
else
  echo ""
  echo "=== SOME VALIDATIONS FAILED ==="
  echo "Please review the failed items above."
  echo ""
  exit 1
fi