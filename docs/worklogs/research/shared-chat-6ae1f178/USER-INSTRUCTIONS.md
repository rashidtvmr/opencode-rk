# Extracted User Instructions — Shared Chat 6ae1f178

**Source:** https://chatgpt.com/share/e/6ae1f178-6dd0-83ee-807a-591a9be0f06a
**Extracted:** 2026-10-28 by @xkiro-mimo-v2-6-flash
**Scope:** User-authored instructions only; ChatGPT advice is transcript content, not instructions.

---

## Chronological user instructions (single turn, Monday 11:15 PM)

### Instruction 1: Investigate the opencode-rk project failure
The user explicitly requests investigation of the opencode-rk project (built initially by ChatGPT via Chat On Steroids), noting:
- The project was supposed to use parallel autonomous subagents writing tasks and looping to complete features.
- Instead, the project got stuck at "600+ worktree and no way to merge those branches taking months to do so."
- Despite having no resource or token limitations, the project was not completed as intended.

### Instruction 2: Research how others succeeded
The user asks to research how OpenAI (for "match problems" / Navier-Stokes), Anthropic (for "migrating bun to zig or creating cpp compiler and all"), and other organizations successfully used parallel autonomous subagents.

### Instruction 3: Compare and pinpoint what went wrong
The user requests an exact comparison of "ours" against the successful examples, pinpointing:
- What went sideways so the project failed.
- What should have been done otherwise.
- How future projects should use multiple parallel autonomous subagents that not only create feature worktrees but also **review, merge, and test** as each feature completes, so the project is built successfully.

---

## Supersession analysis

There is only one user turn in this shared conversation. No supersession within this conversation.

The instructions align with and reinforce the current AGENTS.md operating instructions (2026-10-28) regarding:
- Dynamic task-owned delivery (the `tools/continuous_delivery.py` supervisor)
- Ownership through delivery (implement → review → test → prepare → publish → postcheck)
- Independent exact-candidate review
- Task stays owned until behavior is accepted in the integrated application

---

## Local-delivery reconciliation

The ChatGPT response's recommended operating model (sections 4.1–4.9) directly maps to the dynamic delivery package in `/Users/mymac/Projects/.oc2-delivery-dynamic-20261028-78571fd6/source`:

| ChatGPT recommendation | Delivery implementation |
|----------------------|----------------------|
| Task remains owned through delivery | `tools/delivery/engine.py` stage machine (implement→review→test→prepare→publish→postcheck) |
| Independent diff-first review | `jobs.py` review stage with `DELIVERY_REVIEW.diff`, author ≠ reviewer check |
| Bounded integration pipeline | `config/delivery-workflow.json` `max_wip: 40`, `heavy_checks: 1` |
| Controlled merge queue | `jobs.py` `publish()` with ff-only, publication lock, journal |
| Dynamic worker allocation (no fixed lanes) | `tools/dynamic_agent_allocator.py` — no role has fixed reservation |
| Retries improve approach | `config` `attempts: 3`, `diagnoses: 1` — after failed attempts, independent diagnosis |
| Separate model judgment from enforcement | Trusted `checks` in manifest; agent prose cannot mark tests passed |
| Single publication authority | `jobs.py` `publication_lock()`, `use_recovery_capability: true` |
| Measure delivery and quality | `store.py` events table, `engine.py` `status()` |

The delivery package's `CONTINUOUS_DELIVERY.md` explicitly addresses the "control unfinished work" recommendation: "A single worker pool is allocated from **current eligible tasks**, not reserved role counts."

---

## Key ChatGPT observations relevant to local audit

1. **328 registered worktrees, 538 local branch references** — noted as different quantities from "600+ worktrees" (which may reflect earlier snapshot).
2. **Canonical main-v2 at `6f6872bf`** (Oct 26) checked out at `/Users/mymac/Projects/opencode-rk-recovery-20261005`.
3. **All seven Phase 1 gates recorded UNPROVEN** as of Oct 21 registry.
4. **September 16 audit:** 22 task cards referenced nonexistent crates.
5. **October 1 merge report:** 221 frozen tips → 14 integrated + 207 quarantined.
6. **Recovery mode: controls arrived after substantial integration debt** — multiple competing representations of "current."
