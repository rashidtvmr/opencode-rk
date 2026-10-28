# Extracted User Instructions — v2 (corrected) — Shared Chat 6ae1f178

**Source:** https://chatgpt.com/share/e/6ae1f178-6dd0-83ee-807a-591a9be0f06a
**Extracted:** 2026-10-28 by @xkiro-mimo-v2-6-flash
**Scope:** User-authored instructions only; ChatGPT advice is transcript content, not instructions.
**v2 changes:** Added reconciliation note on section 4.6 fixed-lanes vs dynamic-pool distinction; clarified external case-study assertions are quoted ChatGPT content.

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

## Reconciliation: ChatGPT advice vs. current user direction

**Important distinction (v2):**

ChatGPT's response section 4.6 proposes an **illustrative fixed allocation** of 20 worker assignments (4 implementers, 4 reviewers, 4 test-prep, 3 investigators, 2 integration-prep, 2 adversarial, 1 build/validation). This is ChatGPT's **historical advisory content**, not a user instruction.

The user's **later explicit direction** (recorded in AGENTS.md 2026-10-28 and implemented in the dynamic delivery package) supersedes this fixed-quota approach:
- **Dynamic 20/40 pool**: no role has a fixed reservation or lane count
- `tools/dynamic_agent_allocator.py` allocates from current eligible demand
- "Twenty independent implementation tasks may use twenty implementation slots; forty slots do not mean forty invented tasks"

The **general principles** from ChatGPT's section 4.6 that remain valid:
- Bound unfinished work across the pipeline
- Move capacity to the bottleneck (review/integration when backlog fills)
- Do not manufacture tasks to keep a numerical target occupied
- Serialize heavy validation

The **specific fixed slot numbers** (4/4/4/3/2/2/1) are historical advice and should not be treated as current operating instructions.

---

## Local-delivery reconciliation

The ChatGPT response's recommended operating model (sections 4.1–4.9) maps to the dynamic delivery package in `/Users/mymac/Projects/.oc2-delivery-dynamic-20261028-78571fd6/source` as follows:

| ChatGPT recommendation | Delivery implementation | Match quality |
|----------------------|----------------------|---------------|
| Task remains owned through delivery | `engine.py` stage machine | Direct match |
| Independent diff-first review | `jobs.py` review stage, author ≠ reviewer | Direct match |
| Bounded integration pipeline | `delivery-workflow.json` `max_wip: 40` | Principle match (dynamic vs fixed) |
| Controlled merge queue | `jobs.py` `publish()` ff-only + journal | Direct match |
| Dynamic worker allocation | `dynamic_agent_allocator.py` | **Supersedes** ChatGPT's fixed 4/4/4 allocation |
| Retries improve approach | `attempts: 3`, `diagnoses: 1` | Direct match |
| Separate model judgment from enforcement | Trusted `checks` in manifest | Direct match |
| Single publication authority | `publication_lock()` + `use_recovery_capability` | Direct match |

---

## External case-study assertions

The 8 industry case studies (Bun, Anthropic C compiler, OpenAI harness engineering, OpenAI Symphony, Cursor, Stripe Minions, Dropbox Nova, StrongDM) are **quoted ChatGPT content preserved as transcript evidence**. They were not independently verified by this investigator. The URLs cited are as rendered in the ChatGPT response.

---

## Key ChatGPT observations relevant to local audit (quoted, not independently verified)

1. **328 registered worktrees, 538 local branch references** — noted as different quantities from "600+ worktrees."
2. **Canonical main-v2 at `6f6872bf`** (Oct 26) checked out at `/Users/mymac/Projects/opencode-rk-recovery-20261005`.
3. **All seven Phase 1 gates recorded UNPROVEN** as of Oct 21 registry.
4. **September 16 audit:** 22 task cards referenced nonexistent crates.
5. **October 1 merge report:** 221 frozen tips → 14 integrated + 207 quarantined.
6. **Recovery mode: controls arrived after substantial integration debt.**
