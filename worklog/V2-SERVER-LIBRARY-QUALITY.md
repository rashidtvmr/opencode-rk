# V2 server library quality maintenance

## Package and revision

- Package/gate: **V2-SERVER-LIBRARY-QUALITY / server-library-clippy**, with
  server-unit, agent-loop, frozen execution-owner and HTTP regression controls.
- Status: **CANDIDATE DONE; product acceptance PENDING**. Independent
  PREVERIFIED and exact-integrated-SHA ACCEPTED remain parent-owned.
- Worktree: `/Users/mymac/Projects/opencode-rk-v2-server-quality-y52o0gi3`.
- Branch: `v2/server-quality-y52o0gi3`.
- Base SHA: `7628b61d10625ea35aa09392394c7b689b20d55d`.
- Candidate SHA: the commit containing this file; its full immutable SHA is
  recorded in the handoff and external completion receipt under
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-today-control-y52o0gi3/worker-status/server-quality/`.
- Selective source provenance: preserved candidate
  `353821578700cf9b22ef1b429042f41989f756cc`, inspected using exact
  `git diff 9f6428c1763381d57ab83cf82a42bda284c8fd0f..353821578700cf9b22ef1b429042f41989f756cc`.
  The nine granted maintenance sources were ported with the patch tool onto the
  current base. The test import has the separate independent-owner provenance
  below.

The parent-supplied observed RED is on
`8fce37918a82efaf9b476e29eface0f82ae14322`:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-today-current-quality-8fce379-a_tk7mj9/server-library-clippy.log`.
It contains exactly 14 server-library errors. The source-only comparison
`git diff 8fce379..7628b61d10625ea35aa09392394c7b689b20d55d -- crates/server`
is empty, so those server sources are also the package base. This is mechanical
quality maintenance against an observed compiler/lint gate, not a new server
feature or a behavioral RED/GREEN claim.

## Exact changed paths

```text
crates/server/src/acp_session.rs
crates/server/src/clients.rs
crates/server/src/daemon.rs
crates/server/src/lib.rs
crates/server/src/remote_files.rs
crates/server/src/rules_globs.rs
crates/server/src/runtime_wiring.rs
crates/server/src/sync_log.rs
crates/server/src/web_artifact.rs
crates/server/tests/agent_loop_turns.rs
worklog/V2-SERVER-LIBRARY-QUALITY.md
```

## Diagnostic disposition and source/caller review

References in the first column are the actual locations in the supplied RED
log/base source.

| Diagnostic reference | Maintenance and preserved semantics |
| --- | --- |
| `lib.rs:928`, unread `agent_plan` | Private field becomes `_agent_plan`; the same `AgentExecutor::new` four-step plan, constructor failure check, allocation and stream lifetime are retained at base `lib.rs:1132-1138`. Source search found only its declaration and construction. The actual `LoopController` and provider/tool state machine remain intact. |
| `acp_session.rs:132`, unused `DaemonEvent::kind` | `#[cfg(test)]` scopes this private helper to its actual sole caller, `tests::disc112_t01_start_message_cancel_status_round_trip` at base line 560. The complete module was inspected: there are zero production callers. The event enum, public `session()` method, bus, live bridge and helper body retain their contents. |
| `rules_globs.rs:44`, unread `round_started_loaded` | Keep both `RuleState` fields and `Clone`; manual `Debug` uses the same struct name and the same field names, order and values as the derived formatter. `evaluate` still records `m.is_loaded` at base line 125 and reads `last_match_round` at line 143 for hysteresis. |
| `clients.rs:71`, map key/value iteration | Iterate `guard.values()`, retaining the same lock, cloned message, `try_send` and fan-out behavior. |
| `daemon.rs:90`, unspecified create/truncate behavior | Explicit `.truncate(false)` preserves PID-file bytes before the exclusive lock claim. Existing `try_lock_exclusive` still precedes `set_len(0)`, seek, PID write and sync at base lines 94-104. |
| `daemon.rs:420`, readiness loop | Equivalent `while let Ok(()) = s.readable().await`: EOF, readiness error and non-WouldBlock read errors still end the connection; successful reads and WouldBlock still repeat. |
| `remote_files.rs:185`, manual membership | Slice `contains(&workspace)` preserves exact allowlist equality, after the same workspace validation and before path validation. |
| `runtime_wiring.rs:413,429`, manual membership (2) | `contains(&session)` retains idempotent registration, session bound, locks and the poisoned-lock fallback. |
| `sync_log.rs:81`, complex projector type | Private `SyncProjector` alias retains `Box<dyn Fn(&SequencedEvent) + Send + Sync>`, its lifetime, registration, ordering and replay. |
| `web_artifact.rs:459`, vector initialization | `std::vec!` retains the five initial controls in exact order and with all fields intact; the two conditional code-control pushes remain intact. The preserved controller correction is used: the local `alloc::vec` module only reexports `Vec`, not the macro. |
| `lib.rs:282,1086,1370`, manual membership (3) | `contains` preserves tool-id equality for capability reporting, provider schema and execution allowlisting. The same permission broker and execution/persistence branches remain intact. |

Dead-code/counter inventory was checked against the actual diagnostics and source
callers. The log identifies the three private items above; retained counters are
not deleted to satisfy it. In `acp_session.rs`, `LiveBus.next_id`,
`LiveSession.next_seq` and `LiveSession.delivered` retain their initialization and
increments (`subscribe`, `send`, `restore`); `PublishSummary.delivered` retains
fan-out accounting. In `lib.rs`, `round_ordinal` retains round-ID use, persistence
and saturating increment. Client/daemon identity and client-count counters retain
their uses. Product module exports are retained, and no broad lint suppression is
introduced.

MSRV source review: workspace Rust version is **1.85**, inherited by the server.
The imported forms use existing stable `contains`, map `values`,
`OpenOptions::truncate`, `while let`, `DebugStruct`, type aliases, `cfg(test)` and
`std::vec!` facilities. This is a compatibility review, not an executed MSRV
compiler gate. Manifests and lockfile retain their exact base bytes.

## Current execution-owner preservation

The complete base `TurnExecutionPermit` / fixed `ACTIVE_SESSION_TURNS` registry
source region is byte-identical after the port:
`3ef0f6bfa92d237c12d94a44330f8a9cb41f19590c213dd92b29533b0cb1ef7e`
(SHA-256 of the region from `const MAX_ACTIVE_TURNS` to `AppState`).

A whole-file check reconstructs `lib.rs` from the base with only five approved
replacements: three membership expressions and the declaration/construction
spelling of `_agent_plan`. It matches the resulting file exactly. This also
preserves both endpoint acquisitions, `_permit: TurnExecutionPermit`, stream
guard retention, fixed capacity, poison handling, drop cleanup and all remaining
owner implementation bytes.

## Independently corrected agent-loop contract import

Independent test-owner commit:
`17d780ff5aa06ed4710e6927d3e799683fdc6966`.
Its `worklog/V2-AGENT-LOOP-CONTRACT.md` was inspected from Git and the preserved
worktree. The worklog bytes also match the copy in `3538215`, with SHA-256
`b8bfcff484685ae2f85cd032ab64fefd19905d4b25a4adeddebf6f4b4ee6afb4`.

The imported `crates/server/tests/agent_loop_turns.rs` is byte-for-byte identical
to both `3538215` and that independent owner commit:

- Git blob: `ed920a72fce883928a9da9a0f775ad43ab54b3be`.
- SHA-256: `7865e9c540b8b4a3e1d2cd3dc425b2f7692560d2b70d6e898b1b24f7b986a18a`.
- Base historical blob retained in Git:
  `2a3251b288f79195b1d80cdf710c131d2edcc9b9`.

This is the approved exact contract `Running ls…Loop done`, retaining the
first-round stream data plus the continuation, and the same mechanical unused
`mpsc` import removal. It is an import of the independently corrected blob, not
an assertion authored by this maintenance worker.

Pinned upstream was verified at full checkout
`/Users/mymac/Projects/opencode-upstream-reference`, HEAD
`95daf90670b7c039c436c85537da5fbfe2205b41`. The complete relevant source files were
read:

- Native V2 `packages/core/src/session/runner/publish-llm-event.ts`,
  `fragments` / `publish`, lines 91-129 and 255-263: append every text fragment,
  settle their join and publish the same deltas.
- Native V2 `packages/core/src/session/message-updater.ts`, `update`,
  lines 230-247: retain each text part, append `match.text += event.data.delta`
  and settle complete text.
- Native V2 `packages/core/src/session/runner/to-llm-message.ts`, `assistant`,
  lines 70-112: lower retained assistant text and tool results into provider
  history.
- TUI `packages/tui/src/context/sync.tsx`, `message.part.delta`, lines 398-414:
  accumulate `(existing ?? "") + delta`.
- Approved local requirement `worklog/V2-NATIVE-STREAM-TOOL-LIVE.md:11-15,61-69`
  and its exact-integrated acceptance section require cumulative text across
  tool continuation. This supplies the local round-combination contract above
  the stale historical final-text assertion.

## Source-only verification performed

- `git diff --check`: exit **0**.
- Exact diff and Git-object/hash inspection of the preserved candidate and
  independent test-owner correction: exit **0**, matching blobs/hashes above.
- `python3 - <<'PY'` read-only source comparison using `Path.read_bytes()` and
  `git show <ref>:<path>`: exit **0**. All eight maintenance source files other
  than `lib.rs` exactly match `3538215`; the complete `lib.rs` equals base plus
  exactly its five replacements; the execution-owner region and imported
  contract hashes match; all tracked writes fall within the explicit grant.
  The initial comparison assumed a module named `tests`; `lib.rs` names it
  `broker_gate_tests`. The source-check marker was generalized to
  `\n#[cfg(test)]\nmod ` and the comparison rerun successfully.
- Seven embedded unit-test regions match the base exactly, measured from that
  module marker to EOF:

| Source | Protected-region SHA-256 |
| --- | --- |
| `acp_session.rs` | `f62a44c3e452a1f08ac1758184b5267622062b5a0c03ea9a8f6f3f1dca0f79b2` |
| `clients.rs` | `4e2d7f7ec7531461d11ec26fd6d3aea3e5b5545be720eb52eec2c819a9049806` |
| `daemon.rs` | `9bd8054f1783d64dd7c46f08ac5c25a779db8225bc43d0bf3d8b3dbeb054c2e9` |
| `lib.rs` | `933ba19ceae89e8d66bc5116534762fde498c262d52d31e1e3e4debd7d88a3db` |
| `remote_files.rs` | `fc0dcc0f463adf7bd0e659ac3955196725bf429bff055d76393aef2d87612dfd` |
| `rules_globs.rs` | `5959b62311ae3e654c3c2d185c51d202b5b9440baef5128b3e644948a052b42e` |
| `runtime_wiring.rs` | `c8fbfc6aa17f2c0ef2ec5b49604617bc560e9268954bb74c9a130bb77869aa07` |

`sync_log.rs` and `web_artifact.rs` contain no embedded test modules. The frozen
`session_execution_ownership.rs`, the three HTTP turn regression targets,
`discovery_auth_red.rs`, workspace/server manifests and `Cargo.lock` also match
their base bytes exactly. Frozen owner-test SHA-256 is
`9fdb5a6c01eb7d663421956fda1cc6c5bbf3ffcd43f7873f44bfb8e1f1cbb70c`;
lockfile SHA-256 is
`63ef5299dd93286950af00388796375b06aefc5a4a3eedfa38361954fefb6f03`.

## Historical runtime evidence and remaining observed failures

The preserved receipt
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-server-library-contract-preverify-3538215-9h1r_0le/receipt.json`
was read and records the exact historical candidate:

| Exact historical command | Recorded result |
| --- | --- |
| `/usr/bin/arch -arm64 cargo fmt --all -- --check` | exit 0 |
| `/usr/bin/arch -arm64 cargo clippy --offline --locked -p opencode-rk-server --lib -- -D warnings` | exit 0 |
| `/usr/bin/arch -arm64 cargo test --offline --locked -p opencode-rk-server --all-targets --all-features -- --test-threads=1` | exit 101; 288 passed, 3 failed, 0 ignored |

The log confirms `agentic_loop_e2e_execute_cap_and_policy` passed, then the three
historical `discovery_auth_red` controls failed:
`disc101_t01_stale_descriptor_rejected`, `disc101_t02_pid_mismatch_rejected`, and
`disc101_t03_legacy_empty_token_rejected`. Their fixture disposition remains
independent-test-owner work. These are historical results, not execution on this
new candidate. The current supplied Clippy RED is the latest observed quality
failure until the parent executes the new revision.

## Precise parent-owned verification commands

Run serially through the bounded parent runner with disposable HOME/XDG state,
`CARGO_BUILD_JOBS=2`, bounded timeout/output and the frozen test-owner hashes.
For process-global configuration/ownership controls use one test thread:

```text
cargo fmt --all -- --check
cargo clippy --offline --locked -p opencode-rk-server --lib --no-deps -- -D warnings
cargo test --offline --locked -p opencode-rk-server --lib -- --test-threads=1
cargo test --offline --locked -p opencode-rk-server --test agent_loop_turns -- --test-threads=1
cargo test --offline --locked -p opencode-rk-server --test session_execution_ownership -- --test-threads=1
cargo test --offline --locked -p opencode-rk-server --test session_turn_api --test session_turn_stream_api --test session_turn_activity_api -- --test-threads=1
```

The same controls must pass after parent integration on the exact integrated SHA
before ACCEPTED. The broad historical server gate additionally awaits the
independent discovery-fixture package. This worker ran source/Git comparisons
only; execution and acceptance remain **PENDING**.
