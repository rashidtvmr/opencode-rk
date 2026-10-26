# Will Merging All Branches Complete the Project?

## Short Answer: **No.**

Merging all 305 remaining branches will NOT complete the project. It advances some gates partially but leaves critical gaps, introduces ~60 conflict markers per branch, and contradicts the G8 single-SHA requirement.

---

## Current State (Oct 2, 2026)

| Metric | Value |
|---|---|
| `main-v2` HEAD | `e298c4c6` (165 commits ahead of yesterday) |
| Still divergent | 305 branches |
| `cargo check` | ✅ PASSES (452 warnings) |
| `cargo test` | ❌ FAILS — `session_interrupt_live` E0382 compile error |
| `convergence_gate.py` | ❌ BLOCKED — missing E2E spine markers + ledger notes |
| Branch conflict markers | ~59-60 per large branch vs main-v2 |

## Per-Gate Assessment

| Gate | Current State | What Unmerged Branches Add | Will Merge Close It? | Remaining Work |
|---|---|---|---|---|
| **G0 Salvage** | In progress (heritage merges landing) | History preservation | Partially — OCM session is doing this | Track that all 418 have dispositions |
| **G1 Startup** | **Partial** — `plan_default_launch` called but `plan.view`/`plan.role` never dispatched; `creds_configured` checks env only, ignores persisted auth | `lane/INSTALLED-DEFAULT-CONTRACT` wires daemon lease + E2E (644 lines). `APP-001-CREDENTIAL-SEMANTICS-W1` fixes credential validation | **Partial** — closes dispatch + E2E spine; persisted-auth unification still open | Small: rebase + wire persisted-auth |
| **G2 Provider** | **Partial** — openai-hardcoded client; `registry.rs` ProviderConfig is empty stub | `lane/CUSTOM-PROVIDER-CLIENT-W1` adds real `from_env_for(provider_id)`. But turn-adapter self-admitted "blocked on unwired" | **Partial** — client half closes; turn adapter stays open | Medium: wire non-openai turn adapters |
| **G3 Tool** | **Working** — broker→tool→result chain exists; shell_tool consults broker pre-spawn | `APP-012-LIVE-FILE-TOOL-IMPL-W2` adds secret-safe write + tests. `APP012-APPROVAL-EXPIRY-GREEN` adds boundary expiry | **Yes** for G3 scope | Small: rebase lib.rs/daemon.rs conflicts |
| **G4 Durability** | **Unknown/Partial** — second-turn code exists but restart/resume + second-client unproven | `APP012-RECOVERY-CHILD-BOUND-GREEN` (26k lines), `APP012-RESTART-RECOVERY` exist but wiring status unverified | **Uncertain** | Medium-Large: verify + wire |
| **G5 Native TUI** | **Partial** — `render_once` exists and called; suspend/resume/restore implemented but lifecycle state untracked | `TUI-013-corrected` adds Cell-based state tracking. `TUI-015` adds PTY child lifecycle tests | **Partial** — lifecycle correctness improves; full PTY/raw/resize E2E still unproven | Small: rebase + E2E proof |
| **G6 Web** | **Partial** — browser client skeleton (1595-line App.tsx); bearer fragment auth works | `AUTHWEB-BROWSER-CLIENT-IMPL` hardens auth with HTTP 401 RED tests | **Partial** — auth hardened; tool + reload/resume browser E2E missing | Medium: browser E2E |
| **G7 Ubuntu** | **Missing** — only worklogs + asset branches, no verified archive run | 4 branches (mostly TUI-011 asset loaders) | **No** | Large: build Ubuntu archive + test |
| **G8 Release** | **Structurally impossible via merge** — requires ALL gates GREEN on ONE exact SHA | 3 branches (release gate drivers) | **No** — this is the FINAL gate after everything else passes | Depends on all other gates |

## The 5 Blocking Problems

### 1. `session_interrupt_live` Compile Error (Blocks ALL Tests)
```
error[E0382]: use of moved value: `done`
```
Every `cargo test` fails. No gate can be proven GREEN until this is fixed. This is a 1-line fix on main-v2 (clone the `done` Arc before moving it), but no branch addresses it.

### 2. ~60 Conflict Markers Per Branch
The salvage branches were created on old bases (`merge-base 8a91a7b4` vs HEAD `e298c4c6`). They all touch the same hot files:
- `crates/cli/src/main.rs` — 204 branches
- `crates/server/src/lib.rs` — 209 branches  
- `crates/server/src/daemon.rs` — 205 branches
- `crates/cli/src/tui_entry.rs` — 210 branches

A bulk merge produces ~60 conflict markers per branch. These require **sequential rebase**, not parallel merge.

### 3. Frozen Test Verification (88 branches)
88 branches modify frozen test assertions. Per `AGENTS.md`, these need independent review before merge. Many are legitimate RED→GREEN transitions, but the review is inherently human-gated.

### 4. Cross-Gate Dependencies
Gates are not independent:
- G3 (Tool) depends on G1 (Startup) — tool calls need a running daemon
- G4 (Durability) depends on G3 — second turn needs tool completion
- G6 (Web) depends on G1 + G3 — browser needs daemon + tools
- G8 depends on ALL of G0-G7

### 5. Missing Capabilities (No Branch Covers)
- **G7 Ubuntu**: No branch contains a verified Ubuntu install-and-run
- **G4 Durability**: Recovery/restart code exists but integration unverified
- **G8 Release**: Structurally impossible until G0-G7 all GREEN on one SHA

## What You Should Do Instead

### Phase 1: Fix the Blocker (30 min)
Fix `session_interrupt_live` E0382 on main-v2. This unblocks all test compilation.

### Phase 2: Rebase-Land in Dependency Order (not bulk merge)
```
Order:
1. APP-001 base (G1 foundation)
2. INSTALLED-DEFAULT-CONTRACT (G1 E2E spine)
3. CUSTOM-PROVIDER stack (G2)
4. APP-012 tool chain (G3)
5. TUI-013 + TUI-015 (G5)
6. AUTHWEB (G6)
7. APP012-RECOVERY + RESTART (G4)
```
Each rebase resolves conflicts against current main-v2 HEAD. Run `convergence_gate.py` after each landing.

### Phase 3: Build What's Missing
- G7: Ubuntu archive build + install test
- G4: Wire and verify restart/resume E2E
- G8: Final pass after G0-G7 GREEN

### Estimated Timeline
| Phase | Duration | Method |
|---|---|---|
| Fix compile error | 30 min | Single fix on main-v2 |
| Heritage merges (G0) | 2-4 hours | OCM session (already running) |
| Rebase-land 20 key branches | 4-8 hours | Sequential, 25 min each |
| Missing G4/G7 work | 1-2 days | New implementation |
| G8 release gate | 1 day | Integration + security smoke |
| **Total to ship** | **~3-5 days** | Not achievable by merge alone |

## Summary

```
Can merge complete the project?  NO
Can merge advance the project?   YES — G1, G3 partially, G5 partially
What blocks completion?           
  1. Test compile error (fix first)
  2. Sequential rebase required (not bulk merge)
  3. G4/G7 need new implementation work
  4. G8 requires all gates GREEN on one SHA
```

**Merging is necessary but not sufficient.** The project needs ~20 key branches rebased in order, a compile fix, and new work on G4/G7 before it can ship.
