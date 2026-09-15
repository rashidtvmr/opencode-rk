# TOOL-022

Status: NOT STARTED. Kind: product. Runtime optional: True.
Mandatory for full declared release: yes.
Requirements: REQ-039 (proposed, terse auto-clarity guard).
Dependencies: none.
Test obligations: TOOL-022-T01, TOOL-022-T02, TOOL-022-T03, TOOL-022-T04, TOOL-022-T05.

## User-observable outcome

A terse-mode auto-clarity guard that forces full-sentence rendering when the
response is a security warning, an irreversible action, a multi-step ordered
sequence, or a repeated user question. Guarded responses render in full
sentences in the user's dominant language with no self-reference to
compression or style. Guard is fail-closed (ambiguous classification renders
full, never terse) and default-on (opt-out flag disables terse output
entirely, never the guard). Bypass of the guard on a flagged response is a
security failure, never a style choice.

## Source evidence

- PLAN.md sections 5-6: slice independence rules, mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash, GREEN minimum.
- docs/SECURITY.md sections 5-6: fake-grant HITL fixtures, never disable a
  safeguard to keep the loop moving (guard bypass is a security failure).
- tasks/TOOL-014.md: task-card model (Status/Kind/contract/test obligations).
- tasks/TOOL-021.md: sibling terse renderer card (REQ-039 proposed,
  runtime-optional precedent); this guard sits in front of that renderer.
- docs/proposals/CV-SLICE-02-clarity-spec.md: clarity spec, contract, bounds,
  CV-CLR-T01..T05 definitions mirrored below as TOOL-022-T01..T05.
- Classification: new user requirement (REQ-039 proposed), deliberate
  safety deviation (full output costs more bytes but prevents
  misread of warnings, confirmations, sequences, repeats).

## Observable contract

- `Mode = { Terse, Full }`.
- `Flags { security: bool, irreversible: bool, ordered_steps: bool, repeat: bool }`.
- `RepeatCtx`: caller-supplied bounded history of the last N=3 question
  hashes; guard retains nothing itself.
- `RenderCtx { kind_flags: Flags, lang: LangTag, history: RepeatCtx }`.
- `select_mode(ctx: &RenderCtx) -> Mode`: any flag true => `Mode::Full`;
  all false => `Mode::Terse`.
- `render(ctx: &RenderCtx, body: &str) -> String`: `Full` returns complete
  sentences; `Terse` returns compressed form. `Full` output contains none of
  the banned self-reference strings (`terse`, `compressed`, `caveman`,
  `mode active`, case-insensitive).
- `Full` output uses `ctx.lang`; never falls back to another language.
- Default-on: `config.clarity_guard: bool = true`. Opt-out
  (`terse_enabled = false`) forces `Mode::Full` for every response; there is
  no flag that disables the guard while keeping terse output.
- Classification is pure and synchronous; no I/O, no clock, no model call,
  no env read.
- Suggested module boundary: `crates/policy/src/clarity.rs` owning
  `RenderCtx`, `Flags`, `RepeatCtx`, `LangTag`, `Mode`, `select_mode`,
  `render`; `lib.rs` only re-exports via integrator fragment.

## Failure states

- Ambiguous classification (unknown flag, missing lang tag, empty history on
  a possible repeat) => `Mode::Full`. Never terse on doubt.
- Classifier internal error => `Mode::Full` plus an error receipt; the
  response still renders, uncompressed.
- `Full` output containing a banned self-reference string => test failure,
  treated as guard bypass (cf. SECURITY.md section 6).
- Irreversible action rendered without confirmation copy => guard bypass.
- Security/irreversible/ordered-steps/repeat input rendered terse for any
  reason => guard bypass, fail-closed violated.
- Empty body with any flag true => still `Mode::Full` (no empty-passthrough
  exemption for flagged content).

## Resource bounds

- `select_mode` + `render` are O(n) in response bytes, O(1) extra state.
- No retained transcript: `RepeatCtx` is caller-supplied, bounded to the
  last N=3 question hashes. No unbounded queue or retained output.
- No I/O, no network, no clock, no global state; pure library over caller
  inputs. Deterministic: same `ctx` + `body` => byte-identical output.

## Test obligations (frozen)

- TOOL-022-T01 (security renders full, mirrors CV-CLR-T01): `ctx` with
  `security=true`, body `"Token expiry uses < not <="`. Assert
  `select_mode == Full` and output contains at least two complete sentences
  (two `.` terminated clauses) and contains `"Token expiry"`. Assert no
  banned self-reference string.
- TOOL-022-T02 (irreversible requires confirmation, mirrors CV-CLR-T02):
  `ctx` with `irreversible=true`, body `"drop database"`. Assert
  `select_mode == Full` and output contains case-insensitive `"confirm"`
  plus the exact action noun `"database"`. Assert absence of terse-only
  shortening (no line without a verb). Banned self-reference strings absent.
- TOOL-022-T03 (ordered sequence numbered, mirrors CV-CLR-T03): `ctx` with
  `ordered_steps=true`, 3-step body. Assert `select_mode == Full` and output
  matches numbered lines `1.`, `2.`, `3.` in order. Assert banned
  self-reference strings absent.
- TOOL-022-T04 (repeat triggers full, mirrors CV-CLR-T04): `ctx` with
  `repeat=true`, all other flags false. Assert `select_mode == Full` even
  though content is otherwise terse-eligible. Second call with
  `repeat=false` on identical body asserts `Mode::Terse` (guard releases
  when repeat clears).
- TOOL-022-T05 (language preserved, no self-reference, mirrors CV-CLR-T05):
  `ctx` with `lang="vi"`, all flags false except forced `Full` via
  `repeat=true`, body `"xoa database"`. Assert output contains
  `"xac nhan"` (Vietnamese confirmation) and contains no English-only
  fallback sentence. Assert none of `terse|compressed|caveman|mode active`
  appear (case-insensitive).

## TDD steps

1. Inspect: PLAN.md 5-6, TDD.md 2-5, SECURITY.md 5-6, TOOL-014.md,
   TOOL-021.md, CV-SLICE-02-clarity-spec.md (done, see evidence).
2. Contract: defined above.
3. Author tests TOOL-022-T01..T05; establish compiling RED (fail: no guard).
4. Freeze test hash + command manifest.
5. Implement minimum `clarity.rs` natively in Rust.
6. GREEN, refactor, rerun; negative tests (ambiguous => Full, empty flagged
   body => Full, banned-string scan, `terse_enabled=false` forces Full);
   fixture: fake HITL grant, no live user, no `.env`.
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
test -f tasks/TOOL-022.md
grep -c TOOL-022-T0 tasks/TOOL-022.md
grep -i fail-closed tasks/TOOL-022.md
git status --short -- tasks/TOOL-022.md
cargo test -p opencode-rk-policy clarity
cargo check -p opencode-rk-policy
```

## Remaining gaps / unknowns

- Exact banned-string list is seed; integrator may extend without weakening
  (additive only). Repeat-hash function chosen at implement.
