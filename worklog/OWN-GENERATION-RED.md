# OWN-GENERATION-RED scratchpad

Claim: OWN-GENERATION-RED, session ses_f3c4de578ffelQv59xDXmOs03B.

Base: `7262682e31c1f473912c9483804c9430eb4ee4c5` on
`red/APP012-GENERATION-ENFORCEMENT`.

Source evidence:

- Current code, `crates/storage/src/execution_v2.rs:20-51`,
  `ExecV2::start_execution`, validates only non-negative generation and writes
  the caller-supplied value without comparing it to workspace authority.
- Current schema, `crates/storage/schema/v2/workspace.sql:9-20`, stores the
  canonical `workspace_state.owner_generation`; lines 144-165 store/index each
  execution's generation but do not enforce equality.
- Planned contract, `docs/storage/CRASH_CONSISTENCY.md:25-31`, requires advancing
  the owner generation after lock acquisition and rejecting writer commands from
  earlier generations.
- Current audit, `docs/storage/SECURITY-REVIEW-MODULES-REAL.md:38-45`, identifies
  the missing explicit generation capability check in `execution_v2.rs`.
- New requirement/stage, synthesis commit `7262682`,
  `worklog/PHASE1-VERTICAL-SYNTHESIS.md` symbol `OWN-GENERATION-RED`, assigns the
  test boundary `crates/storage/tests/app012_generation_enforcement_red.rs`.

Observable contract: only the generation exactly equal to the current workspace
generation can insert an execution. Both stale and unissued future generations
fail closed before insertion. Denial leaves zero execution rows. Fixtures are
disposable SQLite databases; each has one session and one three-byte payload.
Every added query has at most three bound parameters and all result reads are
single-row aggregates.

Target boundary: RED test only. No product, schema, manifest, library, or frozen
test changes. The future-generation guard prevents a caller from self-issuing
authority, while the stale-generation guard provides same-host fencing.

## RED receipt

- Focused command, run twice:
  `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p
  opencode-rk-storage --test app012_generation_enforcement_red --
  --test-threads=1`.
- Both runs compiled and produced the same behavioral result: 1 passed, 2
  failed, 0 ignored. The current-generation control passed. Both stale
  generation 6 and unissued future generation 8 were accepted instead of
  failing before insertion.
- Frozen SHA-256:
  `0c528325a57d2d8eeb48e2f917f3181fd70f476a9e17a3948310da6b516e56a3`.
- After freezing: hash unchanged, `git diff --check` passed, and no Cargo,
  rustc, or focused-test process survived.

Remaining: `V1-FREEZE-RED` authority must independently record this hash before
the synthesis graph permits `OWN-GENERATION-IMPL`. No implementation or
acceptance is authorized by this lane.
