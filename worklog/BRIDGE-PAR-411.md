# BRIDGE-PAR-411 scratchpad (UNCLAIMED - orchestrator owns claims.json)

Task: ONE new file `crates/opentui-bridge/src/ui_spinner_full.rs`.
Status: file written, ledger untouched per delegation (no claim/update by design).

## Claim
- Unclaimed. Delegation forbade touching tasks/completion/claims.json.
- Proceeded file-only per instruction.

## Source evidence
- TS truth `packages/tui/src/component/spinner.tsx:10`: `SPINNER_FRAMES = ["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"]`.
- TS truth `packages/tui/src/ui/spinner.ts` read fully (368 lines): Knight Rider scanner only, no 4-frame const; sibling coverage confirmed.
- Existing `crates/opentui-bridge/src/spinner_full.rs:6`: first 8 braille frames; `spinner.rs`: Knight Rider from `ui/spinner.ts:272-329`.

## Observed scenario
- Need first-4 subset only; no overlap edit to `spinner_full.rs`/`spinner.rs`.

## Target boundary
- New file only. No lib.rs/Cargo.toml edits. std-only, forbid(unsafe_code).

## Tests written
- frames_match_ts, frame_at_wraps, count_is_four.

## Decisions
- `frame_at` wraps via modulo; `frame_count` const fn. No Spinner struct (spinner_full.rs owns cursor state); minimal per XState/machine-minimal framing.
- ponytail: skipped cursor struct; add when animated tick needed.

## Remaining unknowns
- None for file scope. Wiring into lib.rs is orchestrator/integration job.
