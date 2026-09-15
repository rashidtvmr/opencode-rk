# CV-SLICE-02 - Terse-Mode Auto-Clarity Guard

Status: PROPOSED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: new user requirement (terse-mode compression guard).
Dependencies: none.
Test obligations: CV-CLR-T01, CV-CLR-T02, CV-CLR-T03, CV-CLR-T04, CV-CLR-T05.

## User-observable outcome

Terse compression mode auto-disables per response when the content is a
security warning, an irreversible action, a multi-step ordered sequence, or
a repeated user question. Guarded responses render in full sentences in the
user's dominant language with no self-reference to compression or style.
Guard is fail-closed (ambiguous classification renders full, never terse)
and default-on (opt-out flag disables terse output entirely, never the guard).

## Source evidence

- PLAN.md sections 5-6: slice template + RED/GREEN lifecycle (this card).
- docs/TDD.md sections 2-5: lifecycle, compiling RED, frozen hash, GREEN.
- docs/SECURITY.md sections 5-6: fake-grant HITL fixtures, never disable
  safeguard to keep loop moving (guard bypass is a security failure).
- tasks/TOOL-014.md: task-card model (contract + gaps + test obligations).

## Observable contract

- `select_mode(ctx: &RenderCtx) -> Mode` where `Mode = { Terse, Full }`.
- `RenderCtx { kind_flags: Flags, lang: LangTag, history: RepeatCtx }`.
- `Flags { security, irreversible, ordered_steps, repeat: bool }`.
- Any flag true => `Mode::Full`. All false => `Mode::Terse`.
- `render(ctx, body: &str) -> String`: `Full` returns complete sentences;
  `Terse` returns compressed form. `Full` output contains no
  self-reference strings (`terse`, `compressed`, `caveman`, `mode active`).
- `Full` output uses `ctx.lang`; never falls back to another language.
- Opt-out: `config.clarity_guard: bool = true` (default-on). Setting
  `terse_enabled = false` forces `Mode::Full` for every response.
- Classification is pure and synchronous; no I/O, no model call.

## Failure states

- Ambiguous classification (unknown flag, missing lang tag, empty history
  on a possible repeat) => `Mode::Full`. Never terse on doubt.
- Classifier internal error => `Mode::Full` plus an error receipt; the
  response still renders, uncompressed.
- `Full` output containing a banned self-reference string => test failure,
  treated as guard bypass (cf. SECURITY.md section 6).
- Irreversible action rendered without confirmation copy => guard bypass.

## Resource bounds

- `select_mode` + `render` are O(n) in response bytes, O(1) extra state.
- No retained transcript: `RepeatCtx` is caller-supplied, bounded to the
  last N=3 question hashes. No unbounded queue or retained output.

## Frozen test obligations

- CV-CLR-T01 (security renders full): `ctx` with `security=true`, body
  `"Token expiry uses < not <="`. Assert `select_mode==Full` and output
  contains at least two complete sentences (two `.` terminated clauses)
  and contains `"Token expiry"`. Assert no banned self-reference string.
- CV-CLR-T02 (irreversible requires confirmation): `ctx` with
  `irreversible=true`, body `"drop database"`. Assert `select_mode==Full`
  and output contains case-insensitive `"confirm"` plus the exact action
  noun `"database"`. Assert absence of terse-only shortening (no line
  without a verb). Banned self-reference strings absent.
- CV-CLR-T03 (ordered sequence numbered): `ctx` with
  `ordered_steps=true`, 3-step body. Assert `select_mode==Full` and output
  matches numbered lines `1.`, `2.`, `3.` in order. Assert banned
  self-reference strings absent.
- CV-CLR-T04 (repeat triggers full): `ctx` with `repeat=true`, all other
  flags false. Assert `select_mode==Full` even though content is
  otherwise terse-eligible. Second call with `repeat=false` on identical
  body asserts `Mode::Terse` (guard releases when repeat clears).
- CV-CLR-T05 (language preserved, no self-reference): `ctx` with
  `lang="vi"`, all flags false except forced `Full` via `repeat=true`,
  body `"xoa database"`. Assert output contains `"xac nhan"` (Vietnamese
  confirmation) and contains no English-only fallback sentence. Assert
  none of `terse|compressed|caveman|mode active` appear (case-insensitive).

## Suggested module boundary

- Proposed owner: `crates/policy/src/clarity.rs` (new file; do NOT move
  existing modules). Structs `RenderCtx`, `Flags`, `RepeatCtx`, enum
  `Mode`, fns `select_mode`, `render`. Registration fragment proposed to
  integrator; implementer never edits shared `lib.rs` directly.
- Tests: `crates/policy/tests/clarity_red.rs` (frozen at RED).

## TDD steps

1. Cite commit + file:line per behavior; classify per PLAN.md section 2.
2. Define contract above; record deviations in worklog.
3. Author `clarity_red.rs` with CV-CLR-T01..T05; run RED (compiles, fails).
4. Freeze test hash + command manifest; submit hash to controller.
5. Implement minimum `clarity.rs`; run GREEN; refactor; rerun.
6. Regressions: fuzz flags x lang x repeat; negative: empty body still
  Full when flagged; fixture: fake HITL grant, no live user, no `.env`.
7. Submit evidence + patch, never acceptance.

## Verification

- `test -f docs/proposals/CV-SLICE-02-clarity-spec.md`
- `grep -c CV-CLR-T0 docs/proposals/CV-SLICE-02-clarity-spec.md` (>=5)
- `grep -i fail-closed docs/proposals/CV-SLICE-02-clarity-spec.md`
- `git status --short` shows only this file.

## Remaining gaps / unknowns

- Exact banned-string list is seed; integrator may extend without
  weakening (additive only). Repeat-hash function chosen at implement.
