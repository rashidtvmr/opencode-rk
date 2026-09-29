# DB-019-BOUNDARY-REPAIR

- Task: `DB-019-BOUNDARY-REPAIR`
- Owner/session: `ses_f13309f86ffe2NKhQu12IaDBdy`
- Claim: API-mediated `completion_claims.claim` succeeded before source edit; canonical claim row is in-progress.
- Owned product path: `crates/storage/src/lib.rs` only. Frozen supplemental test was not edited.
- Base candidate: `729795fc9a7511b8424df3fa297a62caf0f1bce1`.

## Evidence

- Frozen source SHA: `7b798acec527ab8cbe78bd3eeea04ad80687f5ef49f710e683bb61fcced8b0aa`.
- Reviewed source baseline SHA: `aa76b730b3292adf9abebdfb6b72954015ef7f9f8388b79572645935752cdb42`.
- External freeze manifest and receipt were verified before edit.

## Repair decisions

1. `append_tool_pair` now loads `expected_pairs` from the persisted `tool_rounds` row and uses that value for pair-index admission. Caller-mutated `ToolRound.expected_pairs` cannot expand the accepted domain. The existing immediate transaction still validates session/message identity before writing and keeps message plus typed rows atomic.
2. `bounded_history` no longer charges a blob message's declared payload bytes before calling `get_bounded`; it first admits/reads the blob against the remaining budget after already-counted metadata, then charges the actual verified payload exactly once. Inline messages retain their prior metadata-plus-payload accounting. Global item/byte bounds and bounded reads remain enforced.

## Verification status

Source-only candidate. No Cargo, rustc, runtime, database, or heavy validation was run per resource instruction. Independent focused verification remains outstanding. Do not mark completed until the frozen supplemental and affected existing tests are independently GREEN.
