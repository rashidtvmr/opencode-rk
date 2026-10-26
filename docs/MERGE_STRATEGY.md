# Parallel Merge-Sort Strategy for 500+ Worktree Branches → `main-v2`

> **Synthesized from 6 independent AI agent analyses + live repo investigation.**
> Repo: `/Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3`
> Date: 2026-10-01 | `main-v2` tip: `8d5866e3`

---

## 0. Ground Truth (Measured, Not Assumed)

| Fact | Value | Source |
|---|---|---|
| Local branches | 418 | `git for-each-ref refs/heads/ \| wc -l` |
| Already integrated (ancestor of main-v2) | 71 | `merge-base --is-ancestor` loop |
| **Divergent (actual merge work)** | **347** | 418 − 71 |
| Active worktrees | 189 | `git worktree list` |
| Dirty worktrees | 0 (at time of check) | `git worktree list --porcelain` |
| Unreferenced commits | 49 (2-13 days old) | Space Bunny `git fsck` |
| 12 Rust crates | agents, catalog, cli, contracts, foundation, opentui-bridge, providers, security, server, sessions, storage, tools | `ls crates/` |
| `cargo check` time | ~14s on main-v2 | measured |
| `git merge-tree --write-tree` | works (git 2.54) | verified: exit 0 = clean tree SHA |
| Branch families | lane/ (142), v2/ (73), red/ (54), plan/ (39), integration/ (26), verify/ (23), reconcile/ (17), review/ (9), proposal/ (7), prewire/ (6), repair/ (4), policy/ (4), docs/ (3), controller/ (2), misc (7) | `for-each-ref` prefix counts |
| Repo is shallow | graft at `4954312d`, parent absent | Space Bunny discovery |

### Hub Files (Poison the Naive Conflict Graph)

These files are touched by 200+ branches. A naive file-overlap union-find collapses 347 branches into **one 333-branch component** — useless.

| File | Branches touching it |
|---|---|
| `tasks/completion/claims.json` | 286 |
| `crates/opentui-bridge/src/safe_renderer.rs` | 216 |
| `crates/cli/src/tui_entry.rs` | 210 |
| `crates/server/src/lib.rs` | 209 |
| `Cargo.lock` | 206 |
| `crates/server/src/daemon.rs` | 205 |
| `crates/tools/src/shell_tool.rs` | 204 |
| `crates/cli/src/main.rs` | 204 |
| `worklog/*.md` | 203 each |

### Policy Constraints (from `config/convergence-v2.json` + `docs/AGENT_STRATEGY_V2.md`)

| Constraint | Value | Implication |
|---|---|---|
| `integrationWriters` | 1 | Only ONE agent lands on `main-v2` |
| `maxHeavyLocalValidations` | 1 | Only 1 concurrent `cargo check/test` against main-v2 |
| `candidateHighWater` | 4 | Max 4 verified-but-unintegrated candidates |
| `forcePushAllowed` | false | Never force-push |
| `preserveLegacyBranches` | true | Never delete historical branches |
| `logicalAgentCeiling` | 48 | Max 48 agents total |
| `cargoBuildJobs` | 2 | `cargo` limited to 2 parallel rustc |
| Salvage policy | Selective, not indiscriminate | Don't blindly merge stale file contents |

---

## 1. Strategy Overview

**Merge-sort reduction tree** with three key innovations discovered across agents:

1. **`git merge-tree --write-tree`** (Space Bunny) — compute merge results in the object DB with **no checkout, no index, no worktree**. Eliminates the 25-worktree bottleneck entirely. Exit 0 = clean merge tree SHA; exit 1 = conflict with paths listed.

2. **Hub-filtered conflict graph** (TH-DSv4.1) — exclude files touched by >10% of branches from the union-find. At 10% cutoff: 347 branches → 25 balanced buckets (largest component 126, 110 isolated). Without filtering: one useless 333-branch blob.

3. **Ancestry-ordered lane families** (TH-DSv4.1, Luna) — `plan/`/`red/`/`lane/`/`verify/` variants must merge in ancestry order, not arbitrary order. File overlap alone can't express this.

### Architecture

```
                    ┌─────────────────────┐
                    │   ORCHESTRATOR (1)   │
                    │  owns main-v2 ref    │
                    │  owns manifest       │
                    │  owns status board   │
                    └─────────┬───────────┘
                              │
              ┌───────────────┼───────────────┐
              │               │               │
     ┌────────▼──────┐  ┌────▼────┐  ┌───────▼───────┐
     │ ANALYZER (1)  │  │ MERGER  │  │ VERIFIER (1)  │
     │ Opus-class    │  │  POOL   │  │ Independent   │
     │ conflict graph│  │ 25 max  │  │ cargo check   │
     │ → manifest    │  │ flash   │  │ + gate.py     │
     └───────────────┘  └────┬────┘  └───────────────┘
                             │
          Level 0: 25 bucket merges (parallel)
          Level 1: 13 pair merges (parallel)
          Level 2:  7 pair merges
          Level 3:  4 pair merges
          Level 4:  2 pair merges
          Level 5:  1 final ← INTEGRATION WRITER lands this
```

**Total merge operations:** 347 (L0) + 12+6+3+1+1 (merge-up) = ~370
**Total levels:** ceil(log2(25)) + 1 = 6

---

## 2. Phase-by-Phase Algorithm

### Phase 0: BACKUP & PREFLIGHT

```bash
#!/bin/bash
set -euo pipefail
REPO=/Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3
SAFE=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/antigravity/merge-run
mkdir -p "$SAFE"/{state,audit,worktrees,bundles}

# 1. Full backup (refs + objects, NOT dirty files)
git -C "$REPO" bundle create "$SAFE/bundles/premerge-$(date +%Y%m%d-%H%M%S).bundle" --all
git -C "$REPO" bundle verify "$SAFE/bundles"/premerge-*.bundle

# 2. Snapshot every branch tip SHA (provenance manifest)
git -C "$REPO" for-each-ref --format='%(refname:short) %(objectname)' refs/heads/ \
  > "$SAFE/state/branch-tips.tsv"

# 3. Record main-v2 base
BASE=$(git -C "$REPO" rev-parse main-v2)
echo "$BASE" > "$SAFE/state/base-sha.txt"

# 4. Verify cargo check passes on base
CARGO_BUILD_JOBS=2 cargo check --manifest-path "$REPO/Cargo.toml" --workspace --locked

# 5. Rescue unreferenced commits (49 found by fsck)
git -C "$REPO" fsck --unreachable --no-reflogs 2>/dev/null \
  | grep "^unreachable commit" | awk '{print $3}' \
  > "$SAFE/state/unreachable-commits.txt"
# Do NOT create refs for the shallow graft commit (Space Bunny's bug discovery)

# 6. Check for uncommitted work in all worktrees
git -C "$REPO" worktree list --porcelain | grep -B1 "dirty" && {
  echo "STOP: dirty worktrees detected. Commit or stash first."
  exit 1
}
```

### Phase 1: ANALYSIS — Conflict Graph + Manifest

**Key insight (TH-DSv4.1):** `git diff --name-only main-v2..branch` (two-dot) is WRONG when main-v2 is not an ancestor — it inflates changesets. Use explicit merge-base.

```bash
#!/bin/bash
# tools/merge-analysis/collect.sh — one file per branch, no concurrent-write corruption
set -euo pipefail
BRANCH="$1"; OUTDIR="$2"; BASE="$3"

slug="${BRANCH//\//__}"
mb=$(git merge-base "$BASE" "$BRANCH" 2>/dev/null) || {
  echo "${BRANCH}	<DISJOINT-HISTORY>" > "$OUTDIR/$slug.edges.tsv"
  exit 0
}
tip=$(git rev-parse "$BRANCH")

# Already integrated or empty → skip
if [ "$tip" = "$mb" ] || [ "$(git rev-list --count "$mb..$BRANCH")" -eq 0 ]; then
  : > "$OUTDIR/$slug.edges.tsv"
  exit 0
fi

# Changed files (with rename detection)
git diff --name-only -M "$mb..$BRANCH" | sed "s|^|${BRANCH}\t|" > "$OUTDIR/$slug.edges.tsv"
# Line counts + binary detection
git diff --numstat -M "$mb..$BRANCH" > "$OUTDIR/$slug.numstat.tsv"
```

```bash
#!/bin/bash
# Drive collection in parallel — measured: 3s wall for 418 branches with -P20
set -euo pipefail
BASE=$(cat "$SAFE/state/base-sha.txt")
TMPD=$(mktemp -d)

git for-each-ref --format='%(refname:short)' refs/heads/ \
  | grep -Ev '^(main|main-v2)$' \
  | xargs -P20 -I{} bash tools/merge-analysis/collect.sh {} "$TMPD" "$BASE"

# Join AFTER all writers finish (avoids interleave corruption)
cat "$TMPD"/*.edges.tsv   > "$SAFE/state/edges.tsv"
cat "$TMPD"/*.numstat.tsv > "$SAFE/state/numstat.tsv" 2>/dev/null || true
rm -rf "$TMPD"
```

**Partitioning (Python):** Hub-filtered union-find + Jaccard-weighted sub-partitioning.

```python
#!/usr/bin/env python3
"""tools/merge-analysis/partition.py — build manifest.json from edges.tsv + numstat.tsv"""
import sys, json, collections, datetime, subprocess

# Artifact paths that are per-lane bookkeeping, NOT product intent
EXCLUDE_PREFIXES = (
    "worklog/", "tasks/completion/", "sources/", "controller-worker-status/",
    "controller/", ".agents/", "target/", "crates/server/web_dist/assets/",
)
HUB_PCT = 0.10  # files touched by >10% of branches are hubs
MAX_BUCKET = 25
TARGET_BUCKETS = 25

def load_edges(path):
    f2b = collections.defaultdict(set)
    b2f = collections.defaultdict(set)
    for line in open(path):
        if "\t" not in line:
            continue
        b, f = line.rstrip("\n").split("\t", 1)
        if f == "<DISJOINT-HISTORY>":
            continue
        if any(f.startswith(p) for p in EXCLUDE_PREFIXES):
            continue
        f2b[f].add(b)
        b2f[b].add(f)
    return f2b, b2f

def load_weights(path, branches):
    w = {b: 0 for b in branches}
    try:
        for line in open(path):
            parts = line.split("\t")
            if len(parts) >= 3 and parts[0] != "-":
                fname = parts[2].strip()
                # weight is not per-branch here; approximation
                w[fname] = w.get(fname, 0) + int(parts[0]) + int(parts[1])
    except FileNotFoundError:
        pass
    return w

# Union-Find
class UF:
    def __init__(self, items):
        self.parent = {x: x for x in items}
        self.rank = {x: 0 for x in items}
    def find(self, x):
        while self.parent[x] != x:
            self.parent[x] = self.parent[self.parent[x]]
            x = self.parent[x]
        return x
    def union(self, a, b):
        ra, rb = self.find(a), self.find(b)
        if ra == rb: return
        if self.rank[ra] < self.rank[rb]: ra, rb = rb, ra
        self.parent[rb] = ra
        if self.rank[ra] == self.rank[rb]: self.rank[ra] += 1

def partition(edges_path, numstat_path, output_path, base_ref):
    f2b, b2f = load_edges(edges_path)
    weights = load_weights(numstat_path, b2f)

    n = len(b2f)
    if n == 0:
        json.dump({"buckets": [], "quarantine": [], "metadata": {"total_branches": 0}},
                  open(output_path, "w"), indent=2)
        return

    # Hub filtering
    cut = max(2, int(n * HUB_PCT))
    hubs = {f for f, bs in f2b.items() if len(bs) > cut}
    effective_b2f = {b: (files - hubs) for b, files in b2f.items()}

    # Union-find on non-hub file overlaps
    uf = UF(b2f.keys())
    for f, bs in f2b.items():
        if f in hubs:
            continue
        bs_list = list(bs)
        for i in range(1, len(bs_list)):
            uf.union(bs_list[0], bs_list[i])

    # Extract components
    comps = collections.defaultdict(list)
    for b in b2f:
        comps[uf.find(b)].append(b)
    comps = sorted(comps.values(), key=len, reverse=True)

    # Partition: small components are atomic buckets; large ones get sub-partitioned
    buckets = []
    singletons = []

    def branch_weight(b):
        return sum(weights.get(f, 1) for f in b2f.get(b, set()))

    def jaccard(a, b):
        sa, sb = effective_b2f.get(a, set()), effective_b2f.get(b, set())
        if not sa or not sb: return 0.0
        return len(sa & sb) / len(sa | sb)

    for comp in comps:
        if len(comp) <= MAX_BUCKET:
            if len(comp) == 1:
                singletons.append(comp[0])
            else:
                buckets.append(comp)
        else:
            # Greedy nearest-neighbor sub-partition
            remaining = set(comp)
            while remaining:
                seed = min(remaining, key=branch_weight)
                remaining.discard(seed)
                bucket = [seed]
                while len(bucket) < MAX_BUCKET and remaining:
                    best = max(remaining, key=lambda b: sum(jaccard(b, x) for x in bucket))
                    bucket.append(best)
                    remaining.discard(best)
                buckets.append(bucket)

    # Bin-pack singletons
    singletons.sort(key=branch_weight, reverse=True)
    for i in range(0, len(singletons), MAX_BUCKET):
        buckets.append(singletons[i:i+MAX_BUCKET])

    # Merge smallest buckets until we hit target count
    while len(buckets) > TARGET_BUCKETS and len(buckets) > 1:
        buckets.sort(key=lambda b: sum(branch_weight(x) for x in b))
        victim = buckets.pop(0)
        buckets[0].extend(victim)

    # Order within each bucket: ancestry first, then weight ascending, then name
    def order_bucket(branches):
        # Simple: sort by weight ascending, name as tiebreaker
        # Ancestry ordering would require N² merge-base checks within bucket;
        # worth doing for buckets with lane-family prefixes
        return sorted(branches, key=lambda b: (branch_weight(b), b))

    manifest_buckets = []
    for i, bs in enumerate(sorted(buckets, key=lambda b: -sum(branch_weight(x) for x in b))):
        ordered = order_bucket(bs)
        est_conflicts = sum(
            1 for x in range(len(bs)) for y in range(x+1, len(bs))
            if effective_b2f.get(bs[x], set()) & effective_b2f.get(bs[y], set())
        )
        manifest_buckets.append({
            "id": f"bucket-{i:02d}",
            "branches": ordered,
            "merge_order": ordered,
            "estimated_conflicts": est_conflicts,
            "total_diff_lines": sum(branch_weight(b) for b in bs),
        })

    manifest = {
        "buckets": manifest_buckets,
        "quarantine": [],
        "metadata": {
            "total_branches": n,
            "component_count": len(comps),
            "isolated_count": len(singletons),
            "largest_component": max(len(c) for c in comps),
            "hub_files_excluded": len(hubs),
            "hub_cut_threshold": cut,
            "base_ref": base_ref,
            "generated": datetime.datetime.utcnow().isoformat() + "Z",
        }
    }
    json.dump(manifest, open(output_path, "w"), indent=2)
    print(f"Buckets: {len(manifest_buckets)}, "
          f"sizes: {[len(b['branches']) for b in manifest_buckets]}")

if __name__ == "__main__":
    partition(sys.argv[1], sys.argv[2], sys.argv[3],
             sys.argv[4] if len(sys.argv) > 4 else "unknown")
```

### Phase 2: LEVEL 0 — Parallel Bucket Merges

**The `merge-tree` breakthrough:** Each merge is computed in-memory in the git object DB. No worktree needed. No index lock contention. 25 agents can run simultaneously against the same repo.

```bash
#!/bin/bash
# tools/merge-engine/merge_bucket.sh — one bucket, uses merge-tree (no worktree needed)
set -euo pipefail
BUCKET_JSON="$1"  # {"id":"bucket-00","branches":[...],"merge_order":[...]}
BASE="$2"         # base SHA
AUDIT_DIR="$3"    # where to write audit log

BUCKET_ID=$(echo "$BUCKET_JSON" | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])")
BRANCHES=$(echo "$BUCKET_JSON" | python3 -c "
import sys,json
for b in json.load(sys.stdin)['merge_order']: print(b)
")

HEAD="$BASE"
MERGED=()
QUARANTINED=()
COMMITS=()

while IFS= read -r branch; do
  [ -z "$branch" ] && continue
  BRANCH_SHA=$(git rev-parse "$branch" 2>/dev/null) || { QUARANTINED+=("$branch:unresolvable-ref"); continue; }

  # Skip if already ancestor
  if git merge-base --is-ancestor "$BRANCH_SHA" "$HEAD" 2>/dev/null; then
    MERGED+=("$branch:already-contained")
    continue
  fi

  # Try merge-tree (no worktree, no index, no lock)
  RESULT=$(git merge-tree --write-tree "$HEAD" "$BRANCH_SHA" 2>&1) || {
    # Exit 1 = conflict. Capture conflicted paths.
    CONFLICTS=$(echo "$RESULT" | grep "^CONFLICT" | head -10)
    QUARANTINED+=("$branch:conflict:${CONFLICTS}")
    # Log to audit
    echo "{\"ts\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",\"bucket\":\"$BUCKET_ID\",\"branch\":\"$branch\",\"action\":\"quarantine\",\"reason\":\"conflict\",\"details\":\"${CONFLICTS}\"}" \
      >> "$AUDIT_DIR/$BUCKET_ID.jsonl"
    continue
  }

  # RESULT is a tree SHA on success. Create a merge commit.
  TREE_SHA=$(echo "$RESULT" | head -1)
  MERGE_COMMIT=$(git commit-tree "$TREE_SHA" -p "$HEAD" -p "$BRANCH_SHA" \
    -m "merge($BUCKET_ID): $branch into integration")
  HEAD="$MERGE_COMMIT"
  COMMITS+=("$MERGE_COMMIT")
  MERGED+=("$branch")

  echo "{\"ts\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",\"bucket\":\"$BUCKET_ID\",\"branch\":\"$branch\",\"action\":\"merged\",\"commit\":\"$MERGE_COMMIT\"}" \
    >> "$AUDIT_DIR/$BUCKET_ID.jsonl"

done <<< "$BRANCHES"

# Create a ref for this bucket's result
git update-ref "refs/merge/$BUCKET_ID" "$HEAD"
git tag -a "merge-tags/$BUCKET_ID" "$HEAD" -m "L0 $BUCKET_ID: ${#MERGED[@]} merged, ${#QUARANTINED[@]} quarantined"

# Output result JSON
python3 -c "
import json, sys
print(json.dumps({
    'bucket_id': '$BUCKET_ID',
    'status': 'ok' if ${#QUARANTINED[@]} == 0 else 'partial',
    'output_ref': 'refs/merge/$BUCKET_ID',
    'output_sha': '$HEAD',
    'merged_count': ${#MERGED[@]},
    'quarantined_count': ${#QUARANTINED[@]},
    'quarantined': $(printf '%s\n' "${QUARANTINED[@]}" | python3 -c "import sys,json; print(json.dumps([l.strip() for l in sys.stdin if l.strip()]))"),
    'merge_commits': $(printf '%s\n' "${COMMITS[@]}" | python3 -c "import sys,json; print(json.dumps([l.strip() for l in sys.stdin if l.strip()]))"),
}))
"
```

### Phase 3: MERGE-UP (Levels 1→N)

Same `merge-tree` engine, but input is pairs of L(N-1) results instead of raw branches.

```bash
#!/bin/bash
# tools/merge-engine/merge_pair.sh — merge two integration refs
set -euo pipefail
LEFT_REF="$1"   # refs/merge/bucket-00 or refs/merge/L1-pair-00
RIGHT_REF="$2"
OUTPUT_REF="$3" # refs/merge/L1-pair-00
AUDIT_DIR="$4"

LEFT_SHA=$(git rev-parse "$LEFT_REF")
RIGHT_SHA=$(git rev-parse "$RIGHT_REF")

RESULT=$(git merge-tree --write-tree "$LEFT_SHA" "$RIGHT_SHA" 2>&1) || {
  echo "{\"status\":\"conflict\",\"left\":\"$LEFT_REF\",\"right\":\"$RIGHT_REF\",\"details\":\"$RESULT\"}"
  exit 1
}

TREE_SHA=$(echo "$RESULT" | head -1)
MERGE_COMMIT=$(git commit-tree "$TREE_SHA" -p "$LEFT_SHA" -p "$RIGHT_SHA" \
  -m "merge-up: $LEFT_REF + $RIGHT_REF → $OUTPUT_REF")

git update-ref "$OUTPUT_REF" "$MERGE_COMMIT"
git tag -a "merge-tags/$(basename $OUTPUT_REF)" "$MERGE_COMMIT" \
  -m "Merge-up: $LEFT_REF + $RIGHT_REF"

echo "{\"status\":\"ok\",\"output_ref\":\"$OUTPUT_REF\",\"output_sha\":\"$MERGE_COMMIT\"}"
echo "{\"ts\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",\"action\":\"merge-up\",\"left\":\"$LEFT_REF\",\"right\":\"$RIGHT_REF\",\"result\":\"$MERGE_COMMIT\"}" \
  >> "$AUDIT_DIR/merge-up.jsonl"
```

### Phase 4: VERIFICATION

After each level AND on the final result:

```bash
#!/bin/bash
# tools/merge-engine/verify_level.sh
set -euo pipefail
MERGE_SHA="$1"
EXPECTED_TIPS="$2"  # file with one branch-tip SHA per line

# 1. Ancestry verification — every merged tip must be ancestor of result
MISSING=0
while IFS= read -r tip; do
  [ -z "$tip" ] && continue
  git merge-base --is-ancestor "$tip" "$MERGE_SHA" 2>/dev/null || {
    echo "MISSING ANCESTOR: $tip"
    MISSING=$((MISSING + 1))
  }
done < "$EXPECTED_TIPS"

[ "$MISSING" -gt 0 ] && { echo "FAIL: $MISSING tips not in ancestry"; exit 1; }

# 2. Compile gate (only on final or per-level if budget allows)
TMPWT=$(mktemp -d)
git worktree add --detach "$TMPWT" "$MERGE_SHA" 2>/dev/null
CARGO_BUILD_JOBS=2 cargo check --manifest-path "$TMPWT/Cargo.toml" --workspace --locked 2>&1 \
  | tail -5
GATE_EXIT=$?
git worktree remove --force "$TMPWT" 2>/dev/null
exit $GATE_EXIT
```

### Phase 5: FINAL INTEGRATION

```bash
#!/bin/bash
# Only the single integration writer runs this
set -euo pipefail
FINAL_SHA="$1"

# Verify main-v2 hasn't moved
CURRENT=$(git rev-parse main-v2)
EXPECTED_BASE=$(cat "$SAFE/state/base-sha.txt")

if [ "$CURRENT" != "$EXPECTED_BASE" ]; then
  echo "STOP: main-v2 advanced from $EXPECTED_BASE to $CURRENT. Rebuild required."
  exit 1
fi

# Fast-forward main-v2 to the final merge result
git update-ref refs/heads/main-v2 "$FINAL_SHA" "$CURRENT"
git push origin main-v2

# Never force-push. If push fails, stop and investigate.
```

---

## 3. Orchestrator Main Loop (Pseudocode)

```
orchestrator():
  # Phase 0
  backup_and_preflight()
  base = read("state/base-sha.txt")

  # Phase 1: Analysis
  collect_diffs_parallel(P=20)           # 3s wall
  manifest = partition(edges, numstat)    # <1s

  # Phase 2: Level 0 — 25 parallel bucket merges
  L0_results = parallel_map(
    manifest.buckets,
    fn=merge_bucket,
    max_concurrent=25,           # merge-tree has no lock contention
    timeout=600s
  )

  # Phase 3: Merge-up
  level = 0
  current = [r.output_ref for r in L0_results if r.status in ("ok", "partial")]
  while len(current) > 1:
    level += 1
    pairs = pair_adjacent(current)  # odd one passes through
    current = parallel_map(
      pairs,
      fn=merge_pair,
      max_concurrent=13,  # at most ceil(N/2) pairs
      timeout=300s
    )
    tag_level(level, current)
    verify_level(current, expected_tips)

  # Phase 4: Final verification
  final = current[0]
  verify_ancestry(final, all_merged_tips)
  cargo_check(final)                     # ~14s

  # Phase 5: Land (single integration writer)
  fast_forward_main_v2(final)

  # Report
  generate_quarantine_report()
  generate_audit_summary()
```

---

## 4. Subagent Delegation Architecture

### Agent Roles and Model Selection

| Role | Count | Model Class | Timeout | Retry |
|---|---|---|---|---|
| Orchestrator | 1 | Strong (Opus) | ∞ | — |
| Analyzer | 1 | Strong (Opus) | 300s | — |
| L0 Bucket Merger | 25 max | Flash (cheap) | 600s | 1x transient |
| L1+ Pair Merger | 13→7→4→2→1 | Flash | 300s | 1x transient |
| Conflict Resolver | 0-4 on demand | Strong (Opus) | 300s | 1x then quarantine |
| Verifier | 1 (independent) | Code-aware | 300s | — |
| Integration Writer | 1 | Strong | 120s | — |

### Communication Protocol

**Input to each merger:**
```json
{
  "task_type": "merge-bucket | merge-pair | resolve",
  "level": 0,
  "bucket_id": "bucket-07",
  "input_branches": ["lane/APP-012-...", "red/DB-022"],
  "base_ref": "8d5866e328306dbf8041d91bb41fed9a38c0971c",
  "timeout_s": 600
}
```

**Output from each merger:**
```json
{
  "status": "ok | partial | failed",
  "output_ref": "refs/merge/bucket-07",
  "output_sha": "abc123...",
  "merged_count": 18,
  "quarantined": ["lane/XYZ: conflict in crates/server/src/lib.rs"],
  "merge_commits": ["sha1", "sha2", "..."],
  "duration_s": 42
}
```

### Escalation Flowchart

```
merge attempt (merge-tree)
  ├─ exit 0 (clean) → commit-tree → record → next branch
  ├─ exit 1 (conflict, auto-resolvable sections only) → quarantine branch, continue
  │   └─ post-L0: attempt quarantined branches against final result one-by-one
  └─ agent timeout/crash → cancel → re-delegate once → still failing → quarantine
```

### Backpressure Rules

- `merge-tree` has NO lock contention → 25 concurrent is safe
- `cargo check` (worktree-based): max 1 concurrent (policy: `maxHeavyLocalValidations=1`)
- Total agents: max 48 (policy: `logicalAgentCeiling=48`)
- Allocation: 25 mergers + 1 analyzer + 1 verifier + 1 integration writer = 28 (within ceiling)
- Refill-on-completion, not barrier: as L0 buckets finish, start L1 pairs immediately

---

## 5. Time Estimates

| Approach | Wall Clock | Bottleneck |
|---|---|---|
| **Sequential** (current) | ~12-17 hours | 347 × (merge + check) |
| **This strategy (optimistic)** | **35-45 min** | L0 tail + 5 merge-up levels + 1 cargo check |
| **This strategy (realistic)** | **60-90 min** | 15% quarantine + resolver queue + cargo check |
| **This strategy (pessimistic)** | **2-3 hours** | Hub-file contention + multiple rebuild cycles |

### Breakdown (realistic)

| Phase | Duration | Why |
|---|---|---|
| Backup + preflight | 2 min | bundle create + verify |
| Analysis (diff collection + partition) | 10 sec | measured: 3s diffs + <1s partition |
| L0 (25 parallel, ~14 branches each) | 10-15 min | tail = slowest bucket; merge-tree is fast |
| L1-L5 merge-up (5 levels) | 5-10 min | pair merges are fast (2 inputs each) |
| Quarantine retry | 5-15 min | attempt quarantined branches against final |
| Final cargo check | 14 sec | measured on main-v2 |
| **Total** | **~40-90 min** | **10-25x faster than sequential** |

---

## 6. Safety & Rollback

### Pre-flight Checklist

1. ✅ `git bundle create --all` + `bundle verify`
2. ✅ Record all branch tip SHAs
3. ✅ Verify no dirty worktrees
4. ✅ Verify `cargo check` passes on base
5. ✅ Verify disk space (only ~500MB objects needed, NOT 25× repo)
6. ✅ Rescue unreferenced commits (but NOT shallow grafts — Space Bunny's bug)
7. ✅ Record `main-v2` base SHA for fast-forward check

### Rollback Procedures

| Failure | Recovery |
|---|---|
| Single branch conflict | Quarantine branch, continue bucket |
| Entire bucket fails | Skip bucket, merge remaining L0 results at L1 |
| Level N merge-up conflict | Delete Level N refs, re-partition the conflicting pair |
| `cargo check` fails on final | Binary search: which L0 bucket introduced the break |
| `main-v2` moved during run | Abort, fetch new base, rebuild from Phase 1 |
| Complete disaster | `git bundle unbundle premerge.bundle` restores everything |

### Verification at Every Level

```bash
# After each merge commit:
git merge-base --is-ancestor $BRANCH_TIP $MERGE_RESULT

# After each bucket:
for tip in $BUCKET_TIPS; do
  git merge-base --is-ancestor $tip $BUCKET_RESULT
done

# After final:
for tip in $ALL_MERGED_TIPS; do
  git merge-base --is-ancestor $tip $FINAL_RESULT
done
```

### Audit Trail

Every operation logged to `$SAFE/audit/*.jsonl`:
```json
{"ts":"2026-10-01T15:41:52Z","bucket":"bucket-07","branch":"lane/APP-012","action":"merged","commit":"abc123"}
{"ts":"2026-10-01T15:42:01Z","bucket":"bucket-07","branch":"lane/DB-022","action":"quarantine","reason":"conflict","details":"CONFLICT (content): crates/server/src/lib.rs"}
```

---

## 7. Failure Modes & Mitigations

| Failure Mode | Probability | Mitigation |
|---|---|---|
| Hub files create false conflicts | HIGH | Hub filtering at 10% threshold; exclude worklog/claims/assets |
| Naive file-overlap = one giant component | CERTAIN without filtering | Union-find with hub exclusion; measured: 333→126 largest component |
| `git diff` two-dot inflates changesets | HIGH | Use explicit `merge-base` + two-dot, not `main-v2..branch` |
| Concurrent `xargs` corrupts shared output | PROVEN | One output file per branch, join after completion |
| Shallow graft poisons backup | PROVEN | Exclude graft commits from rescue refs |
| `cargo check` fails after merge | MEDIUM | Binary search across L0 buckets to isolate cause |
| Branch has merge commits inside it | MEDIUM | Detect with `git rev-list --merges`; order after merged-in branch |
| AI "auto-resolves" semantic conflict | — | **Never.** Only git's deterministic merge. Human review for semantic. |
| More agents than ceiling (48) | LOW | 28 peak (25 mergers + 3 support) = within budget |
| `main-v2` moves during run | LOW | Fast-forward push rejects non-ff; rebuild from new base |
| Stale branch content reintroduced | MEDIUM | Salvage policy: selective activation, not blind merge |

---

## 8. Key Differences from Naive Approaches

| Naive Assumption | Reality (Discovered) | Fix |
|---|---|---|
| "500+ branches" | 347 divergent (71 already integrated) | Filter integrated branches first |
| "Need 25 worktrees" | `merge-tree --write-tree` needs ZERO worktrees | Use merge-tree for all merges except final cargo check |
| "File overlap = conflict" | Hub files collapse graph to 1 component | Hub filtering + artifact exclusion |
| "Two-dot diff is correct" | Inflates when main-v2 isn't ancestor | Explicit merge-base |
| "Parallel `xargs` to shared file" | Line interleaving/truncation | Per-branch output files |
| "Bundle backs up everything" | Misses dirty files, uncommitted work, unreferenced commits | Separate snapshot for each category |
| "Commit count ≥ sum of branch commits" | Mathematically impossible (union, not sum) | Use `rev-list --count --not main-v2` per branch |
| "25 parallel cargo checks" | Policy: `maxHeavyLocalValidations=1` | Check only on final result (or per-level with queue) |

---

## 9. Execution Checklist

```
□ 1. Review this document with operator
□ 2. Run preflight.sh — must exit 0
□ 3. Commit/stash any dirty worktree state
□ 4. Rescue unreferenced commits (excluding grafts)
□ 5. Run analysis: collect.sh + partition.py → manifest.json
□ 6. Review manifest: bucket sizes, quarantine list, hub files excluded
□ 7. Run L0: 25 parallel merge_bucket.sh
□ 8. Verify L0: ancestry checks on all bucket results
□ 9. Run L1-L5: pair merges until single result
□ 10. Verify final: ancestry of ALL 347 merged tips
□ 11. cargo check on final (single worktree, 14s)
□ 12. Attempt quarantined branches against final, one-by-one
□ 13. Integration writer: fast-forward main-v2
□ 14. Push (non-force)
□ 15. Generate final audit report
```

---

## 10. Files Produced by Subagents (Reference)

| Agent | Files | Location |
|---|---|---|
| Space Bunny | `lib.sh`, `preflight.sh` | `/private/var/.../T/opencode/merge-safety/` |
| Vyce | `README.md` (analysis doc) | `worklog/merge-orchestration/README.md` |
| GLM 4.7 | `merge_orchestrator.sh`, `merge_worker.sh`, `merge-validate.sh`, `merge-test.sh`, docs | `/Users/mymac/Projects/opencode-rk/scripts/` |
| TH-DSv4.1 | Inline scripts + partition.py | (in agent response, not written to disk) |

---

## Attribution

| Agent | Key Contribution |
|---|---|
| **Luna** (9router-luna-free) | Overall algorithm design, time estimates, policy compliance analysis, failure table |
| **Vyce** (vyce-deepseek-v41) | Repo exploration, branch inventory, crate structure mapping |
| **TH-DSv4.1** (9router-th-dsv41-flash-free) | Hub-file discovery, measured union-find degeneracy, Python partition code, 5 critical bug finds |
| **GLM 4.7** (9router-cf-glm47) | Production shell scripts, state machine, CI/CD integration |
| **Muse Spark 1.3** | Subagent delegation architecture, wire protocol, pool management, progress board |
| **Space Bunny** (oc-space-bunny-free) | `merge-tree --write-tree` discovery, safety/rollback, 2 bug reproductions, shallow graft hazard |
| **Orchestrator** (this agent) | Repo investigation, cross-evaluation, ground-truth validation, synthesis |
