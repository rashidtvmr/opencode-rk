# CV-RES-01: Terse-response (caveman-style) compression research

Status: proposal research only. No product code.
Sources: PLAN.md section 1 (product contract, safety deterministic); docs/SECURITY.md section 5 (HITL approvals); tasks/UI-014.md (composer: draft, queue-while-busy, bounded output).

## 1. Behavior inventory

Compression rules observed in terse-response modes:
- Strip filler openers, closers, hedges. No greeting, no apology, no "happy to help".
- Telegraphic sentences. Fragment allowed when unambiguous. Pattern: thing, action, reason, next step.
- Preserve verbatim: file paths, code symbols, commands, error strings, numbers, URLs. Never paraphrase or round.
- One idea per line or bullet. Prefer lists over paragraphs for findings.
- Code first, then at most 1-3 short lines of explanation.

Intensity ladder (lite / full / ultra):
- Lite: full sentences, filler removed, structure kept. Default for mixed audiences.
- Full: fragments, dropped conjunctions, compressed transitions. Default for expert repeat users.
- Ultra: single words, symbols, diffs only. Opt-in, reversible, never default.

Auto-clarity triggers (exit terse mode, use full explicit sentences):
- Security warnings: auth, secrets, permissions, sandbox, network exposure.
- Irreversible actions: delete, overwrite, migrate, publish, billing, access grants.
- Multi-step ordered sequences where fragment ambiguity risks misread.
- User repeats a question: prior terseness failed, expand and confirm understanding.
- Legal, compliance, data-loss, or human-approval requests.

Invariants:
- Language preservation: reply in user's dominant language; identifiers and errors stay verbatim.
- No invented abbreviations. Standard tech acronyms only.
- Style never overrides correctness, safety, or quoted evidence.

## 2. Why users ask for it (verbose-AI failure complaints)

- Filler tax: long openers and restatements bury the answer; experts scroll past half the output.
- Attention cost: wall-of-text replies raise time-to-first-action versus a diff plus one line.
- Token and latency cost: verbosity slows streaming, grows transcripts, raises provider bills.
- False helpfulness: over-explaining simple fixes reads as uncertainty; users trust short exact answers.
- Consistency demand: repeat users want the same compressed shape every turn, not a new essay.

## 3. When terse hurts (risk table)

| Context | Harm | Required behavior |
|---|---|---|
| Security warning (secret, sandbox, grant scope) | Compressed warning gets skipped or misread | Full explicit sentences. State threat, consequence, required action. Per SECURITY.md section 5, never compress approvals. |
| Irreversible action (delete, publish, migrate) | User confirms without grasping scope | Full confirmation: what, scope, undo path. Require explicit grant. |
| Multi-step sequence (ordered setup, recovery) | Fragment order ambiguity causes wrong step | Numbered full sentences. One action per step. Confirm state between steps. |
| Ambiguous request | Terse guess looks confident, solves wrong problem | Ask one clarifying question in full sentences before acting. |
| Repeated question | Prior brevity already failed | Expand, restate understanding, check interpretation explicitly. |
| New or non-expert user | Jargon fragments exclude | Lite level minimum. Define terms once. |
| Quoted evidence (logs, errors, policy) | Paraphrase corrupts meaning | Quote verbatim. Never compress inside quotes, code, or numbers. |

Rule: auto-clarity triggers are mandatory overrides, not suggestions. Safety content stays fully explicit.

## 4. Minimal native design recommendation

- Renderer-level, content-preserving: implement terseness as a presentation preference on the composer output path (UI-014 precedent: draft, send, queue-while-busy), not as a rewrite of model content or stored transcript.
- Setting: three levels (lite/full/ultra) plus auto-clarity toggle locked ON for safety, irreversible, and approval flows. Default lite. Ultra requires explicit opt-in per session.
- Guardrails: renderer must never alter code identifiers, file paths, commands, numbers, error strings, or quoted policy text. Compression applies to prose wrapper only.
- Authority alignment: per PLAN.md section 1 and SECURITY.md section 5, prompts and style hints cannot grant access, bypass human-only grants, or shrink approval text. Approval prompts always render full.
- Bounds: cap retained compressed preview; full transcript stays canonical and bounded per UI-014 resource rules. No unbounded queue of pending compressions.
- Measure: time-to-action, repeat-question rate, approval comprehension spot-checks. Revert to lite on repeated-question signal.
