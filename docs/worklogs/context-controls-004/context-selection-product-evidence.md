# Context inspection and selection — requirement and source evidence

## Authority

`tasks/NATIVE-CONTEXT-CONTROLS-004.md` sections 004-B and 004-C are the explicit
user extension: inspect context, selectively exclude items from actual requests,
preserve durable history, and retain mandatory current instructions. Per-item
selection is not being claimed as upstream parity.

The repository pin is **`95daf90670b7c039c436c85537da5fbfe2205b41`** in
`sources/upstream.lock.json`. The explicit task also requests comparison with
**`70a24697ea0028e19f22712fd63059538cb4bee7`** in the user's checkout; its
`packages/core/src/session/compaction.ts` was inspected earlier in this work.
These are distinct revisions and neither is silently substituted for the other.
The older pinned commit is also available read-only in
`/Users/mymac/Projects/tmp/opencode` (`git cat-file -t` returned `commit`).
The pinned `packages/opencode/src/session/prompt.ts` was inspected with
`git show <pin>:packages/opencode/src/session/prompt.ts`:

- `runLoop` calls `MessageV2.filterCompactedEffect` before model conversion.
- `SessionTools.resolve` and `MessageV2.toModelMessagesEffect` build tool and
  conversation payloads; `instruction.system` supplies current instructions.
- `handle.process` receives rebuilt system instructions, converted messages,
  tools, and the session permission rules.

This supports preserving the permission boundary and rebuilding instructions.
The specific Rust selection API and session persistence are the user-requested
extension, not an inferred upstream endpoint.

## Observable regression and implementation seams

- Frozen initial HTTP contract: `session-context-selection-frozen-red.json`;
  `GET /api/sessions/{id}/context` returned 404 instead of 200.
- `Storage::bounded_context_history` filters selected message IDs before item
  limits, byte accounting, and payload loading. Existing raw history loaders
  remain available for durable-history/integrity verification.
- A typed call/output pair shares its durable display message ID. Selected
  pair counts and emitted records use that identity; excluding one pair does
  not suppress other pairs in the same round.
- `checkpoint_provider_history` is the shared turn/continuation/compaction
  projection. Existing native opaque and fallback checkpoints are indivisible;
  excluding a checkpoint omits that checkpoint while keeping its post-boundary
  selected tail. Raw compacted history is not silently resurrected.
- Compaction-prefix entries remain inspectable in durable history, but their
  per-message selection is unavailable once represented by a checkpoint. The
  checkpoint itself has an explicit whole-checkpoint selector.
- POST selection uses the existing per-session turn permit. It cannot change
  the input selection halfway through an owned turn or compaction operation.
- `/context` metadata distinguishes stored-page bytes, selected projection
  estimates, and admission limits. Byte/4 estimates are labeled; prospective
  input, current instruction rebuilding, tools, and request-envelope costs are
  not misrepresented as measured model usage.

## Verification so far

`context-selection-fourth-check.log` records three supplemental payload tests
and the original restart/API test passing. The supplemental observer corrections
and initial genuine tool-pair leakage are retained in
`context-selection-projection-contract.json`; corrected assertions are not
misreported as an unchanged compiling RED contract.

`context-selection-storage-regressions.log` and
`context-selection-backend-regressions.log` record focused raw-history,
compaction, settings, provider-budget, and server-unit regression checks passing.
Independent review, opaque-checkpoint/auth checks, native controls verification,
and final installed release acceptance remain separate requirements.
