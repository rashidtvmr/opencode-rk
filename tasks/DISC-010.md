# DISC-010 - Verify coverage gate completeness

Status: IN PROGRESS. Kind: enabler-or-assurance. Runtime optional: False.
Mandatory for full declared release: yes. Depends on: DISC-001 (pins), DISC-002 (inventory), DISC-003 (surfaces).

## User-observable outcome

No release completion certificate may be issued while the discovery/coverage gate
is open. DISC-010 runs the structural coverage gate and the DISC-003
reconciliation validator against the current frozen state, captures the exact
command output, and records every gap as an unresolved finding rather than
silently closing the gate.

## Scope

Honest accounting only. No source changes, no schema edits, no file rewrites.
This card captures the state of the coverage gate and lists gaps for the
controller and future implementers.

Files read:
- tools/coverage_gate.py
- tools/reconcile_surfaces.py
- tools/inventory.py (check_coverage, read_jsonl)
- sources/inventory/manifest.json
- sources/surface-candidates.jsonl
- sources/disc-003-reconciliation.json
- sources/disc-003-reconciliation.manifest.json
- sources/upstream.lock.json
- sources/evidence.json
- sources/behavior-surface-rules.json
- ralph.json
- PLAN.md section 4 and section 12

## Exact commands and output

### Command 1: coverage gate

Command run:

    python3 tools/coverage_gate.py 2>&1

Exit code: 2 (failure)

Full JSON output (captured):

    {
      "passed": false,
      "sourceEntries": 8196,
      "surfaces": 0,
      "errors": [
        "Unsupported or stale inventory manifest schema",
        "Unreviewed source: 9router:17c4cc76877bd1755030a8414f8d0083f48dcccf:.dockerignore",
        "Unreviewed source: 9router:17c4cc76877bd1755030a8414f8d0083f48dcccf:.editorconfig",
        ... (8196 unreviewed source entries total) ...
        "Behavior-surface ledger is absent"
      ]
    }

Summary of error categories:

| Error category | Count | Example |
|---|---|---|
| Schema mismatch | 1 | "Unsupported or stale inventory manifest schema" |
| Unreviewed sources | 8196 | "Unreviewed source: 9router:17c4cc7...:src/lib/oauth/providers/cursor.js" |
| Behavior-surface ledger absent | 1 | "Behavior-surface ledger is absent" |
| **Total** | **8198** | |

Unreviewed sources broken down by repository:

| Repository | Unreviewed sources | Total inventory entries |
|---|---|---|
| opencode:95daf90670b7c039c436c85537da5fbfe2205b41 | 6626 | 6626 |
| 9router:17c4cc76877bd1755030a8414f8d0083f48dcccf | 1570 | 1570 |
| **Total** | **8196** | **8196** |

`surfaces` is 0 because `sources/surfaces.jsonl` does not exist.

### Command 2: DISC-003 reconciliation validator

Command run:

    python3 tools/reconcile_surfaces.py 2>&1

Exit code: 0 (passes)

Full JSON output:

    {
      "passed": true,
      "surfaceFamilies": 32,
      "reviewStateCounts": {
        "partial": 11,
        "queued": 21
      },
      "implementationStatusCounts": {
        "partial": 7,
        "reference-implemented": 4,
        "unresolved": 21
      },
      "evidenceReferences": 43,
      "unresolvedFindings": 44,
      "errors": [],
      "manifest": "/home/rashid/projects/opencode-rk/sources/disc-003-reconciliation.manifest.json"
    }

### Command 3: plan validator

Command run:

    python3 tools/validate_plan.py 2>&1

Exit code: 0 (passes)

Output:

    validate_plan: OK  stories=178 requirements=35 obligations=890 deps_synthesized=True

## Analysis of findings

### Finding 1: inventory manifest schema version mismatch (BLOCKING)

`tools/inventory.py` `create_inventory` writes the manifest with
`schemaVersion: 3` (tools/inventory.py:320). The coverage gate
`coverage()` function checks for `schemaVersion: 2` only:

    if manifest.get("schemaVersion") != 2:
        errors.append("Unsupported or stale inventory manifest schema")

The actual manifest at `sources/inventory/manifest.json` line 116
contains `schemaVersion: 3`. This is a tooling bug: the generator and the
gate disagree on the expected version. Either the gate should accept
schemaVersion 3, or the generator should emit schemaVersion 2.

Source evidence:
- tools/inventory.py line 320: `"schemaVersion": 3,`
- tools/coverage_gate.py line 20-21: `if manifest.get("schemaVersion") != 2:`
- sources/inventory/manifest.json line 116: `"schemaVersion": 3,`

### Finding 2: zero reviewed sources (BLOCKING)

`check_coverage` (tools/inventory.py:247-285) iterates every inventory entry
and requires a matching entry in `sources/reviews.jsonl`. Every inventory key
must be present in the reviews file with `status: "reviewed"` and a
`reviewReceipt`. The reviews file does not exist:

    $ ls sources/reviews.jsonl
    ls: cannot access 'sources/reviews.jsonl': No such file or directory

Result: all 8196 inventory entries (6626 opencode + 1570 9router) report
"Unreviewed source". This is the core DISC-002 gap -- no upstream file has
been reviewed yet. PLAN.md section 2 (line 40-44) explicitly states that
"Inspected evidence includes ... [not] a review of every file." and section 12
(lines 262-269) states that the full source inventory, user database importer
and product benchmarks "have not been built or executed here."

### Finding 3: behavior-surface ledger absent (BLOCKING)

The coverage gate reads `sources/surfaces.jsonl` (coverage_gate.py:87-89) and
appends "Behavior-surface ledger is absent" when it is missing. The file does
not exist. This is the surface ledger that maps reviewed sources to concrete
behavior surfaces and test IDs (PLAN.md section 7, lines 182-200).

### Finding 4: reconcile_surfaces.py passes but all surfaces are partial or queued

The DISC-003 reconciliation validator passes structurally (exit 0, no errors),
but this is a shallow pass. The reconciliation document
(`sources/disc-003-reconciliation.json`) records:

- 11 surfaces marked `reviewState: "partial"` with `implementationStatus` as
  either `reference-implemented` (4 of them) or `partial` (7 of them).
- 21 surfaces marked `reviewState: "queued"` with
  `implementationStatus: "unresolved"`.

Total unresolved findings in the reconciliation: 44 (from the manifest).

The 4 `reference-implemented` surfaces (9router.account-storage,
9router.model-combos, 9router.routing, 9router.token-refresh) carry explicit
unresolved items, e.g.:

- "Reconcile migrations/schema/versioning and every settings/usage
  persistence table before claiming full reference parity"
- "Attach direct model alias/combo tests or record a typed upstream test gap"
- "Reconcile all provider-specific cooldown and quota paths"
- "Enumerate every provider refresh implementation and background refresh caller"
- "Verify concurrency/single-flight behavior and failure persistence across
  providers"

The 7 `partial` surfaces (opencode.agent-delegation, opencode.catalog-provider,
opencode.extensibility, opencode.integration-auth, opencode.permission-runtime,
opencode.session-runtime, opencode.tools-security) all carry unresolved findings
such as:

- "Do not treat process-local BackgroundJob status as durable child-agent state"
- "Reconcile resume/steer/navigation and persisted child context from legacy
  task/session code separately"
- "Cross-provider child selection is target behavior, not established by this
  evidence"
- "Enumerate provider adapter/plugin matrix and per-provider tests"
- "Full session caller/test enumeration still requires the exact pinned
  inventory"
- "Enumerate every side-effecting tool and non-tool mutation caller"
- "Separate upstream ordinary permission semantics from target mandatory
  non-bypassable security rules"

The 21 `queued` surfaces remain unresolved by definition (DISC-003
scopeNote: "queuedSurfaceIds remain unresolved by definition").

### Finding 5: schema version drift between reconciler and coverage gate

The reconciliation manifest
(`sources/disc-003-reconciliation.manifest.json`) reports
`schemaVersion: 1` for the reconciliation document itself, which the
reconciler expects (reconcile_surfaces.py:55). The inventory manifest uses
`schemaVersion: 3` (inventory.py:320) but the coverage gate expects 2
(coverage_gate.py:20). These are independent schema versioning tracks and are
not directly comparable, but the inventory/gate mismatch is a concrete bug.

### Finding 6: reconcile_surfaces.py manifest output not consumed by coverage_gate

The coverage gate reads `sources/reviews.jsonl` for review receipts, not the
DISC-003 reconciliation manifest. The reconciliation validator produces
`sources/disc-003-reconciliation.manifest.json` as a separate artifact. There
is no wiring connecting reviewed-surface findings from DISC-003 into the
per-source review receipts that DISC-010 / coverage_gate.py requires. This
means the DISC-003 reconciliation pass does not reduce the 8196 unreviewed
sources reported by the coverage gate.

## Unresolved findings (gaps)

1. **Schema version mismatch between inventory generator and coverage gate.**
   - tools/inventory.py writes schemaVersion 3.
   - tools/coverage_gate.py checks for schemaVersion 2 only.
   - Status: BLOCKING. Needs a tooling fix: either align the gate to accept
     schema 3, or align the generator to emit schema 2. The correct fix
     depends on which version is current. Evidence in sources/inventory/
     manifest.json shows schema 3 is what exists on disk.

2. **No per-source review receipts exist (sources/reviews.jsonl absent).**
   - 8196 of 8196 inventory entries are unreviewed.
   - This is the core DISC-002 scope: every upstream file must be reviewed and
     assigned a disposition (product/nonproduct) with a trusted review receipt.
   - Status: BLOCKING. Requires DISC-002 completion (per-file source reviews).

3. **Behavior-surface ledger absent (sources/surfaces.jsonl missing).**
   - The coverage gate requires this file to map behavior surfaces to test IDs.
   - PLAN.md section 7 states: "Maintain a second surface ledger and map
     surfaces to concrete test IDs."
   - Status: BLOCKING. Requires DISC-003 surface ledger publication.

4. **44 unresolved findings across 32 DISC-003 reconciliation surfaces.**
   - 11 partial surfaces each carry unresolved work items.
   - 21 queued surfaces are unresolved by definition.
   - No surface has been fully reconciled to `reviewState: "reconciled"`.
   - Status: BLOCKING per PLAN.md section 2: "No completeness certificate
     may be issued while that discovery gate is open."

5. **No connection between DISC-003 reconciliation evidence and coverage gate
   review receipts.**
   - The reconciler produces a summary manifest but does not populate
     sources/reviews.jsonl.
   - Status: BLOCKING. Without per-source review receipts, the coverage gate
     will always report all sources as unreviewed even after DISC-003
     reconciliation.

## Verification

Command run:

    python3 tools/validate_plan.py

Output:

    validate_plan: OK  stories=178 requirements=35 obligations=890 deps_synthesized=True

Exit code: 0. This satisfies the success criteria for DISC-010:
"python3 tools/validate_plan.py must pass." The plan structure itself is valid.

However, the coverage gate (the substantive gate for DISC-010's goal) fails:

    python3 tools/coverage_gate.py
    # exit code 2
    # passed: false

This is an honest failure: the coverage gate is not a substitute for source
review, and no source review has been completed yet.

## Result

DISC-010 is NOT complete. The coverage gate fails with 8198 errors:
1 schema mismatch, 8196 unreviewed sources, 1 missing behavior-surface ledger.
The DISC-003 reconciliation validator passes structurally but represents only
a shallow pass over 44 unresolved findings across 32 surface families.

Per PLAN.md section 10 (lines 241-248): "Completion requires all mandatory
tasks accepted, all source/surface audits resolved and every applicable release
gate passed." The coverage gate is a release gate. It does not pass.

DISC-010's own mandate is to document findings honestly, not to force the gate
green. This task card accomplishes that: exact commands, exact output, exact
source evidence, and listed gaps.

## Evidence paths

- tools/coverage_gate.py (lines 20-21: schema check; lines 87-89: surfaces
  check; line 86: check_coverage call)
- tools/reconcile_surfaces.py (lines 55-102: validate_reconciliation)
- tools/inventory.py (lines 247-285: check_coverage; line 320: schemaVersion 3)
- sources/inventory/manifest.json (line 116: schemaVersion 3)
- sources/surface-candidates.jsonl (28 candidate families)
- sources/disc-003-reconciliation.json (32 surface families: 11 partial,
  21 queued)
- sources/disc-003-reconciliation.manifest.json (44 unresolvedFindings)
- sources/upstream.lock.json (2 pinned repos: opencode, 9router)
- sources/evidence.json (pinned evidence catalog)
- sources/behavior-surface-rules.json (surface rules for DISC-002 candidates)
- ralph.json (DISC-010 status: not-started)
- PLAN.md sections 2, 4, 7, 10, 12
