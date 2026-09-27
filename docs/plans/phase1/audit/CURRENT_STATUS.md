# Current status audit

Baseline: `767a86a84376a69b2814ea959bb2c94831783768`. Candidate PR #1: `6f3737f10853d1ba08f396efa1be0e4a33616748`. Prepared September 21, 2026.

## Complete metadata inventory

| Source | Recorded inventory | Interpretation |
|---|---:|---|
| ralph.json | 258 stories; 258 accepted | Legacy recorded status, not fresh product evidence. |
| prd.json | 231 entries; 221 not-started, 9 accepted, 1 in-progress | Conventional export contradicts canonical flags; not an independent truth. |
| claims.json | 161 entries; 96 completed, 58 in-progress, 7 blocked | Coordination claims; completion is not integration/release acceptance. |
| Completion JSON includes | 89 stories | 12 APP, 11 TUI, 10 PAR, 21 NET/MOB, 16 COORD/SHIP, 19 discovered. |
| Audit shards | 20 | Audit completion does not mean the audited subsystem is complete. |
| Formal completion/audit tasks with no claim | 41 | Unclaimed, not proven untouched source code. |
| Root Markdown task cards | 216 | 50 canonical legacy IDs lack a matching card. |
| Claim-only IDs outside formal definitions | 93 | Every ID is retained and routed for reconciliation. |
| Card-only IDs absent from plans/claims | 8 | DISC-003, REL-005..008, TOOL-021..023. |
| Distinct IDs across sources | 468 | Complete union, not a summed feature-completion percentage. |

There are 222 shared ralph/prd status disagreements and 27 omitted legacy export IDs. The legacy file has 237 empty dependency arrays, 82 TBD titles and 96 generic discovery titles. `recorded-status-inventory.json` preserves every observed ID/status pair. Its provenance is an exact ID/status projection from the read-only audit, not a full copy of all original prose. `tools/refresh_inventory.py` regenerates full records from a checkout.

## Seven recorded blockers

| ID | Recorded issue | Plan owner |
|---|---|---|
| AUD-016 | Mobile framework/device/signing prerequisites. | SCOPE-01; later mobile audit retained. |
| FIX-CI-APPROVAL | Full CI fixture hangs despite a focused pass. | P1-W1-05 |
| FIX-MCP-STUBS | Invalid frozen JSON repetition syntax / unresolved names and HashMap imports. | P1-W1-05 |
| FIX-STREAM | Global environment fixture races; serial passes are not parallel safety. | P1-W1-05, P1-W2-04 |
| LANE-CI-TOKEN | Older integrated-tree/build blockers. | P1-W1-05, P1-W1-07; revalidate stale note. |
| LANE-DEFAULTTUI-FIX | Older native/build/entrypoint blockers. | P1-W1-02, P1-W1-03; revalidate stale note. |
| LANE-SHAPES | Missing observable box/border/grid contract. | P1-W1-10, P1-W2-02 |

**Correction to the earlier review:** `crates/tools/src/mcp_config.rs` exists in this newer main. The test-contract issue is still visible in source; the earlier missing-file diagnosis is not current.

## Source-checked product findings

| Finding | Source at audited main | Required disposition |
|---|---|---|
| Interactive TUI still reads lines and only appends a user message. | `crates/cli/src/tui_entry.rs:373-437`, especially POST `/messages` and `stdin.lock().lines()`. | P1-W2-01..06: actual input, renderer and coding engine. |
| Native snapshot is not proof of a persistent interactive app. | `tui_entry.rs:600-610`, `Renderer::render_once`. | Real PTY/console tests through installed entrypoint. |
| Native parity tests acknowledge gaps and permit typed native refusal. | `crates/cli/tests/native_tui_parity.rs:10-14,286-307`. | Retain tests, add independent real interactive acceptance. |
| Live turn handlers reject non-openai provider IDs and instantiate Responses from environment. | `crates/server/src/lib.rs:763,775,896,908,1306`. | Protocol-selected transport; immutable request configuration; exact upstream model. |
| Foreground pointer contract differs across the bridge. | Rust `safe_renderer.rs:277-292` sends null foreground; pinned Zig `packages/native/src/lib.zig:1800-1810` requires non-null fg and calls `ptrToRGBA(fg)`. | Safety-critical P1-W1-04. Symbol-name equality was insufficient. |
| Workspace membership/memory and several web execution capabilities are explicitly unavailable. | `crates/server/src/lib.rs:219-291` capability/workspace handlers. | Real shared services and truthful UI; no fake enabled controls. |
| Source differential ledger does not certify release. | `sources/completion/surface-evidence.json`: certified=false; 32 family dispositions. | Source-to-journey mapping and independent final proof. |

Buffer next/current semantics, native color representation and allocation limits also need a real ABI audit; those are questions to test, not asserted corrections from this document.

## Candidate PR #1 is not main

The existing draft is unmerged. Review its persistent-loop proposal rather than merge wholesale. Specific review targets are byte-to-character input, absent/provisional raw-mode handling, blocking provider calls, static command labels, ignored host decisions, unbounded draft growth, ignored keymap, short snapshot timeout applied to provider turns and a lease Drop that kills the spawned shared daemon. Audit the platform library selection and cleanup code against actual OpenTUI semantics. Preserve useful hunks after regression tests.

## Evidence limits

This pass inventoried all metadata and inspected critical runtime/wiring and test contracts. It did not execute every feature, run current Cargo/native/browser/live-provider suites, or establish a reliable percentage complete. Attempts to invoke the convergence gate through the intermittent connector were not dispatched; there is no fresh gate result from those attempts. Previous CI failures do not identify a current source failure without runner logs and execution evidence.

Treat a row as: recorded legacy acceptance, recorded claim completion, in-progress, blocked, or unclaimed/card-only; track source implementation and app proof separately. The crosswalk's family mappings are proposed triage ownership, not 468 independently verified semantic equivalences.

## Preservation

The user's original checkout and its uncommitted provider edit were preserved. A detached read-only audit worktree was created at `/projects/opencode-rk-phase1-review`. No product code, canonical status ledger or frozen test was changed during this planning request.
