# TOOL-021

Status: NOT STARTED. Kind: product. Runtime optional: True.
Mandatory for full declared release: yes.
Requirements: REQ-039 (proposed, terse response renderer).
Dependencies: none.
Test obligations: TOOL-021-T01, TOOL-021-T02, TOOL-021-T03, TOOL-021-T04, TOOL-021-T05.

## User-observable outcome

A native terse renderer with intensity levels (`Intensity: Lite, Full, Ultra`)
that compresses agent response text into `thing-action-reason-next-step`
pattern output. Code, file paths, commands, error strings, URLs stay
byte-exact. Security warnings, irreversible-action confirmations, multi-step
ordered sequences render verbatim, never compressed. Level switches at
runtime via `set_intensity`. Renderer only, never alters underlying data.
Default-on with opt-out; zero cost when off (passthrough, no allocation
beyond input clone).

## Source evidence

- PLAN.md sections 5-6: slice independence rules, mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash, GREEN minimum.
- tasks/TOOL-014.md: task-card model (Status/Kind/contract/test obligations).
- docs/proposals/CV-SLICE-01-render-spec.md: render spec, contract, bounds,
  CV-RND-T01..T05 definitions mirrored below as TOOL-021-T01..T05.
- Classification: new user requirement (REQ-039 proposed), deliberate
  resource-bounded deviation (compression is presentational; source text
  retained by caller).

## Observable contract

- `TerseRenderer::new() -> TerseRenderer` defaults to `Intensity::Full`, enabled.
- `set_enabled(bool)` toggles. Disabled = `render` returns input unchanged.
- `set_intensity(Intensity)` switches level; affects subsequent `render` only.
- `render(&self, input: &str) -> String`:
  - empty input returns empty string.
  - Lite: strips filler/status phrases, keeps full sentences.
  - Full: `thing-action-reason-next-step` fragments; drops conjunctions.
  - Ultra: single word where one suffices; identifiers/numbers/paths exact.
  - All levels: identifiers, numbers, paths, commands, errors, URLs byte-exact.
  - Bypass list renders verbatim: lines matching `SECURITY:` / `CONFIRM:` /
    ordered `1. 2. 3.` sequences pass through uncompressed.
- `render` never mutates caller data; pure function of `(enabled, intensity, input)`.
- Renderer-only: never mutates, stores, persists, or reorders underlying data.
- Suggested module boundary: `crates/render/src/terse.rs` owning
  `Intensity`, `TerseRenderer`; `lib.rs` only re-exports.

## Failure states

- Empty input: defined, returns `""`, no error.
- Disabled renderer: passthrough, no truncation, no panic.
- Oversize input (> `MAX_INPUT_BYTES` = 1_048_576): returns input unchanged,
  no allocation for compression pass.
- Non-UTF8 boundary callers: contract is `&str`; invalid bytes rejected at
  type level, never lossy-converted inside renderer.

## Resource bounds

- Zero cost when off: single branch, no regex compile, no heap beyond return.
- Bounded: output length <= input length; single pass, O(n) time, O(n) output.
- No retained state between calls except `enabled: bool` + `intensity` enum.
- No I/O, no clock, no network, no global state.

## Test obligations (frozen)

- TOOL-021-T01 (lite compresses, mirrors CV-RND-T01): `Lite` on input with
  filler `"Sure! I'd be happy to help. Bug in auth middleware."` asserts output
  contains `"Bug in auth middleware"` and contains neither `"Sure!"` nor
  `"I'd be happy"`.
- TOOL-021-T02 (ultra minimal, mirrors CV-RND-T02): `Ultra` on
  `"Token expiry check uses wrong operator"` asserts ultra shape
  (`"Bug auth-middleware. Token expiry check use `<` not `<=`. Fix:"` shape):
  word count strictly less than `Full` output word count for same input,
  and length < input length.
- TOOL-021-T03 (identifiers verbatim, mirrors CV-RND-T03): all levels on input
  containing `` `OutputStore::new` ``, `crates/tools/src/output_store.rs:10`,
  `"E0599"` assert byte-exact substring presence in output.
- TOOL-021-T04 (numbers exact + empty defined, mirrors CV-RND-T04):
  `render("") == ""`; input `"retry after 24h, see SUBAGENT_COOLDOWN.md"`
  asserts `"24h"` and `"SUBAGENT_COOLDOWN.md"` present verbatim.
- TOOL-021-T05 (switching + opt-out, mirrors CV-RND-T05):
  `set_intensity(Ultra)` then `Lite` outputs differ for same filler-heavy
  input; `set_enabled(false)` asserts `render(x) == x` exactly, including filler.

## TDD steps

1. Inspect: PLAN.md 5-6, TDD.md 2-5, TOOL-014.md, CV-SLICE-01-render-spec.md
   (done, see evidence).
2. Contract: defined above.
3. Author tests TOOL-021-T01..T05; establish compiling RED (fail: no renderer).
4. Freeze test hash + command manifest.
5. Implement minimum `TerseRenderer` natively in Rust.
6. GREEN, refactor, rerun; negative tests (bypass list, oversize passthrough).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/TOOL-021.md
cargo test -p opencode-rk-render terse
cargo check -p opencode-rk-render
```
