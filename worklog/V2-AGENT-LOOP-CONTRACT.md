# V2 agent-loop contract evaluator candidate

Package: **V2-AGENT-LOOP-CONTRACT**  
Role: independent contract evaluator / test owner (source-only)  
Base: `96edb1b18438e1d7311b280374e43ffa8b6e22d9`  
Allowed changed paths: `crates/server/tests/agent_loop_turns.rs`,
`worklog/V2-AGENT-LOOP-CONTRACT.md`

## Disposition

The historical assertion at `crates/server/tests/agent_loop_turns.rs:310` was
unambiguously superseded. It asserted only the second provider delta
(`"Loop done"`) and discarded the first-round assistant text
(`"Running ls…"`). That contradicts both the pinned upstream session
accumulation behavior and the already accepted native streaming contract. The
correction is exact, not a `contains`/flexible assertion:

```text
Running ls…Loop done
```

All other assertion intent is preserved: real bash execution, provider-round
count, tool schema and typed call/output replay, transcript persistence, cap
semantics, deny-by-default behavior, capabilities honesty, and metadata/stop
reason checks. No fixture provider response, product code, E2/E3 scenario, or
safety policy was changed. The unused `mpsc` import was removed mechanically.

## Authority evidence

Pinned upstream is commit
`95daf90670b7c039c436c85537da5fbfe2205b41`, inspected in the full read-only
checkout `/Users/mymac/Projects/opencode-upstream-reference`:

* `packages/core/src/session/runner/publish-llm-event.ts:255-263`, symbol
  `publish` (`text-delta`): appends each provider delta to the text accumulator
  and publishes the same delta.
* `packages/core/src/session/message-updater.ts:230-247`, handlers
  `session.next.text.started`, `session.next.text.delta`, and
  `session.next.text.ended`: create one text part, append every delta with
  `match.text += event.data.delta`, and settle it with the complete text.
* `packages/core/src/session/runner/to-llm-message.ts:70-112`, helper
  `assistant`: sends the durable assistant content (including the accumulated
  text part) back to the provider alongside tool call/result parts.
* `packages/tui/src/context/sync.tsx:398-414`, `message.part.delta` handler:
  applies each delta as `existing + delta`, preserving prior streamed text.

The accepted local native streaming contract
`worklog/V2-NATIVE-STREAM-TOOL-LIVE.md:11-15,61-69` explicitly requires
accumulated assistant/reasoning text across server tool rounds and cites the
same upstream accumulation symbols. Its approved fixture
`tests/e2e/native_stream_tool.py:59-64,206-208` freezes
`ASSISTANT_FIRST = PARTIAL + " " + FINAL` and requires that exact accumulated
assistant history after continuation and restart. This is the higher-authority
current requirement over the stale historical line 310 expectation.

## RED evidence preserved

The approved canonical RED artifact parent was
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-agent-loop-canonical-red-96edb1b-htz899kv`.
The target compiled and reported `0 pass / 1 fail` at the historical line 310:
actual `Running ls…Loop done`, expected `Loop done`. The same failure was
observed on mechanical candidate `ae99bf4` (`215 pass / 1 fail`), independently
showing this is not a lint-work regression.

This candidate does **not** claim GREEN or acceptance. Existing helper reads and
provider accepts remain as-is; no source-only bounded-resource or cleanup claim
is made. The parent must run the complete server gate with its serial bounded
runner and fresh HOME after integration.

## Verification and handoff

Performed only the owned-file formatting and whitespace checks requested:

```text
rustfmt --edition 2021 crates/server/tests/agent_loop_turns.rs
git diff --check
```

Both completed successfully. Candidate status: **CANDIDATE**, not
PREVERIFIED/ACCEPTED. Required next exact verification is the parent’s complete
server gate on the exact integrated SHA, using the canonical serial bounded
runner and fresh HOME; acceptance requires that gate to pass on that integrated
revision. No Cargo/build/runtime/network/child-agent command was run by this
contract evaluator.

Original test file at base `96edb1b18438e1d7311b280374e43ffa8b6e22d9`:
`crates/server/tests/agent_loop_turns.rs` (original blob/hash must be retained
by Git history; this candidate changes only the stale exact expectation and the
unused import). Candidate hash and full candidate SHA are supplied by the
controller after the normal commit is created from these two paths.
