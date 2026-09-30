# Phase 1: 21-session review wave and proposed implementation slices

## Status and exact base

- Source base: `d382fdfd8966052c81ef7ad10a56d87314ab8a73`.
- Delegated and received 21 read-only reports: 11 Muse Spark, 5 Xkiro GPT-6
  Luna, 5 Xkiro DeepSeek V4.1 Flash.
- Assignment structure: four application-spine reviews, three independent
  verifier reviews, fourteen feature/repair reviews.
- These assignments are reviews, not product leases or acceptance records.
  No claim rows, accepted flags, frozen tests or controller configuration were
  changed by this wave.
- Preflight: system memory free percentage 83%; no Cargo/Rust/Zig build
  processes. Subagents were prohibited from local builds, runtimes, database
  access and browser/PTY sessions. The orchestrator ran validation serially.
- Ready unclaimed implementation tasks: zero at each dispatch preflight.

## Delegation contract

Every assignment was instructed to read `.agents/WORKER.md` first, then
`AGENTS.md`, applicable TDD/security/convergence documents and controller
settings. Reports must establish exact revision/path/symbol evidence, identify
real callers, distinguish actual behavior from state-only fixtures, name
single-file proposed slices and list executable test prerequisites.

Review completion requires a bounded evidence report. It does not authorize
implementation. Implementation completion additionally requires a valid card
and claim, independent compiling behavioral RED, immutable test/command hashes,
real caller wiring, GREEN, canonical/lane gates and integrated-revision rerun.
Platform/signing/device evidence must be real; missing evidence remains blocked.

## Roles, goals, scope and success criteria

All product paths below are proposed scopes, not new grants. Shared files must
be serialized; this table is not permission to run their writers concurrently.

| ID | Model | Role and goal | Proposed owned scope | Success criteria for a future implementation |
|---|---|---|---|---|
| S1 | Luna | Integration: default launch consumes owner/attacher and main/setup decisions | `crates/cli/src/main.rs` | Installed fresh-home PTY starts/attaches one authenticated daemon; redirected stdio remains bounded; closing one UI preserves other clients |
| S2 | Luna | Integration: bounded session/database operation lifetime | `crates/sessions/src/lib.rs` | Permits survive caller cancellation until blocking work ends; close rejects admission and observes drain; real disposable SQLite verifies no write after drain |
| S3 | Luna | Integration: durable typed coding turn and replay | `crates/server/src/lib.rs` | Header before effects, A output then atomic pair persistence before B dispatch; failure stops B and next provider request; restart uses bounded typed history |
| S4 | Muse | Integration: actual native interactive renderer/input loop | `crates/cli/src/tui_entry.rs` | Owned live renderer, authenticated effect dispatch, real PTY input/resize/quit and terminal restoration; no snapshot-only proof |
| V1 | Luna | Verifier: installed parent journey and frozen provenance | Existing journey test artifacts, read-only | Exact unchanged tests execute against installed native binary; provider fixture is the only substitute; receipts include artifact/revision/resource identity |
| V2 | DeepSeek | Verifier: claims versus content hashes and commanded test counts | Claims/worklog/test artifacts, read-only | Compare content SHA-256 to content SHA-256; identify unsupported notes and drift without rewriting evidence |
| V3 | Luna | Verifier: dependency graph and ownership | Plan/card paths, read-only | Graph has real contracts, no false-completed dependency, no test-author deadlock, and serialized shared-file integration |
| P1 | Muse | Native Linux builder/provenance | `crates/opentui-bridge/native/build_opentui.sh` candidate | Audit existing pinned builder before rebuilding; exact toolchain/source/target and runtime closure proven on actual Linux targets |
| P2 | Muse | Apple build/link/load contract | `crates/opentui-bridge/build.rs` | Accept real Apple artifacts, enforce platform/ABI selection, and verify clean-target loader closure with matching package layout |
| P3 | Muse | Windows installation transaction | `scripts/install-oc2.ps1` | Bounded staged format/identity checks, collision-safe prior-generation preservation, actual Windows rollback and runtime-library proof |
| P4 | Muse | POSIX archive installation transaction | `scripts/install-oc2.sh` | Bounded safe extraction, serialized install/uninstall, atomic publication, correct rollback and no user-data mutation |
| P5 | Muse | Web event/turn client recovery | `web/src/lib/api.ts` | Consume real registered transport with bounded reconnect/resync; no fallback that fabricates a completed turn; UI caller wiring has a separate serialized owner |
| P6 | Muse | Provider account/request authority | Provider request/account seam, scope pending source registration | Real account authority reaches provider construction; secret grants/storage are real; stale side-branch test references cannot drive a speculative type change |
| P7 | Muse | Broker checks on real file operations | `crates/tools/src/file_ops.rs` | Each action reaches the real broker; deny/human-only decisions prove absence of side effects; shell and registry adapters need separate owners |
| P8 | Muse | Storage typed-history semantics | `crates/storage/src/lib.rs`, review only unless a new gap is proved | Preserve integrated atomic pairs, bounds and migration; downstream callers, not another storage island, close persistence |
| P9 | Muse | Bridge ABI/status/single-owner lifecycle | `crates/opentui-bridge/src/safe_renderer.rs` | Verify pinned ABI first; one shared claim, truthful render outcomes, owned suspend/resume/restore; preserve existing frozen assertions |
| P10 | Muse | Fail-closed signed release workflow | `.github/workflows/release.yml` | Required native matrix, actual authorized signing, hashes/provenance after signing, publication only after all required proof; unsupported targets cannot be silently dropped |
| P11 | DeepSeek | Cross-platform authenticated singleton descriptor | `crates/server/src/daemon.rs` | Real caller identity and liveness on each OS, private publication, foreign/symlink/stale refusal, bounded negotiation and no unrelated-process termination |
| P12 | DeepSeek | Shared-daemon shutdown ownership | `crates/cli/src/main.rs`, serialized with S1 | Explicit stop drains listeners/tasks/storage; client exit does not stop shared daemon; no unowned hold-client tasks |
| P13 | DeepSeek | Durable workspace/session authority | `crates/server/src/workspace_sessions.rs` | Real route caller enforces scope and durable create/rename/archive/reopen/fork semantics; state-only module tests are insufficient |
| P14 | DeepSeek | Mandatory Phase 1/full-release coverage | Source/surface accounting artifacts, read-only | Retain every mandatory legacy/new obligation; distinguish local boundary from real remote/mobile/platform acceptance |

## Session receipts

Each session returned a report; the orchestrator still independently evaluates
every recommendation. Session IDs are routing receipts, not feature evidence.

| ID | Session |
|---|---|
| S1 | `ses_f0fa639deffeN4fb4cB6VfFL2f` |
| S2 | `ses_f0fa639ddffeRaTxNO9sNXV92Z` |
| S3 | `ses_f0fa639ddffdsoC6bHplVUZRDb` |
| S4 | `ses_f0fa639d3ffeYrb3TEyK1OFtZJ` |
| V1 | `ses_f0fa639c4ffekZIEN09VyyJgU3` |
| V2 | `ses_f0fa639c4ffdh3RuovzgPA2L8n` |
| V3 | `ses_f0fa63949ffe65VJYYwOBCqLrg` |
| P1 | `ses_f0fa3c3c1ffeYi72bgEo0Eydgh` |
| P2 | `ses_f0fa3c3bcffesGWZGXfqB2yNbw` |
| P3 | `ses_f0fa3c3a0ffePWJ0xJELG32Kja` |
| P4 | `ses_f0fa3c38fffeZ6lFzn4aKh5pt3` |
| P5 | `ses_f0fa3c38effefrZED097fsTZzB` |
| P6 | `ses_f0fa3c38dffe5TrsjFYT8pmYSl` |
| P7 | `ses_f0fa3c387ffeDO64sgSOk5mb0X` |
| P8 | `ses_f0fa3c372ffeBMvT971S0I6Pj5` |
| P9 | `ses_f0fa3c371ffe1i6eosfmGR7nlZ` |
| P10 | `ses_f0fa3c370ffeBx1CfnzQ2pLz1m` |
| P11 | `ses_f0fa3c352ffecCMDFk8izUjn64` |
| P12 | `ses_f0fa3c351ffeAyJXHLbQDTlYyf` |
| P13 | `ses_f0fa3c350ffeWfqt5oGu62kgPu` |
| P14 | `ses_f0fa3c33affe6JeWY9jPV8pNiS` |

## Independently run validations

### Real live tool-loop baseline

```text
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test agent_loop_turns --offline -- --test-threads=1
```

One test passed, zero ignored/failed. The target drives execute/cap/default-deny
scenarios with real loopback provider and tool execution, but uses an in-memory
store and manually constructed router. It does not prove installed launch,
authenticated descriptor discovery, typed restart or PTY behavior.

- Supervisor duration: 30.278175 seconds, timeout 120 seconds, exit 0.
- Receipt SHA-256:
  `db161e0d7f231c0f84a2397e9217381b3b3edb4f608b8741b88d29ed1ca8c01b`.
- Receipt directory:
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/phase1-live-turn-baseline-20260930`.

### Existing macOS daemon identity RED

```text
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --lib daemon::tests::rc02_current_uid_matches_filesystem --offline -- --exact --test-threads=1
```

Compiled successfully, one test executed and failed at `daemon.rs:609` because
`current_uid()` reads `/proc/self/status` at lines 219-220 on Darwin. Fixture
TMPDIR is restricted to a new approved scratch directory. No child sleep
fixture, server process, user DB or user secret was accessed.

- Duration: 2.60386 seconds, timeout 120 seconds, exit 101.
- Receipt SHA-256:
  `ee0d0f6a4c75bc67a960a0c25fb1418d4ab3cb977663cc5a5859395a1d8fa03a`.
- Receipt directory:
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/phase1-daemon-uid-red-20260930`.

This is an actual product RED, not a compile failure or placeholder assertion.
Repair must retain the effective-owner check. Skipping it on macOS is rejected.
The server forbids unsafe code and has no accepted native identity dependency;
the safe cross-platform API/dependency needs a separate authorized contract.

## Corrections to subagent recommendations

- Do not regenerate/re-freeze existing tests to match drift. The prior frozen
  artifacts remain evidence; disputed contracts require independent review.
- A content SHA-256 is not a Git object ID. A failed `git cat-file` lookup says
  nothing about test fabrication. APP-009's current content hash matches its
  note, but its real terminal/signal caller remains missing.
- A Rust `todo!()` test can compile. It still is not a behavioral RED contract
  and cannot be edited by implementers/orchestrators to obtain GREEN.
- Proposed graphs must derive edges from real contracts. Arbitrary sequential
  APP/DB/TUI numeric chains and false-completed prerequisites are rejected.
- Closure-owned permits plus a correctly closed admission gate can provide
  drain observability; a separate JoinSet is not automatically required. Define
  completion/error ownership and prove cancellation races before selecting the
  minimal implementation. No synchronous mutex guard may cross an await.
- Preserve the approved tool-output-before-pair-persistence order. Such output
  is not proof of durability. Emit terminal errors on pair failure and never
  dispatch the next effect or provider request afterward.
- Do not extend mandatory `AppState` fields blindly: existing frozen tests use
  struct literals. Shared runtime wiring needs a compatibility-preserving
  integration contract, not mass frozen-test edits.
- Do not stop a shared daemon when its launching UI exits. Shutdown is an
  explicit owner action and must account for all clients and background work.
- Do not skip macOS UID checks, substitute descriptor ownership for caller
  identity, or infer endpoint identity from a held lock alone.
- Installing by predictable `.bak` files, two renames with a missing-live-path
  window, or unchecked PowerShell process exit statuses is not an accepted
  rollback design. Same-filesystem publication and recovery need real tests.
- `tar -t` names alone do not enforce uncompressed byte budgets or safely
  classify all archive links. A macOS-only `timeout` command cannot be assumed
  present. Architecture checks must precede candidate execution.
- Cross-language normalization needs a protocol/fixture contract; extracting
  TypeScript into a module does not make Rust use that implementation.
- Do not silently omit required platform artifacts or pass signing credentials
  in command arguments. A runner label alone is not a reproducible image pin.
- Existing native builder/manifest candidates must be reviewed from their Git
  refs; absence from HEAD is not proof they do not exist anywhere.

## Proposed integration order and checkpoints

1. **Contract/ownership checkpoint:** independently resolve the immutable
   backlog/native test disputes and inaccurate legacy acceptance. Preserve all
   258 mandatory stories, original notes and frozen evidence. Register precise
   repair paths and RED-author ownership through the required review process.
2. **Platform/native prerequisites:** P11 caller identity/liveness and private
   publication; P1/P2 reviewed native builder/artifact/load contracts; P9 real
   bridge lifecycle. These unblock native compilation without weakening gates.
3. **Persistence spine:** S2 session adapter lifetime/typed APIs, then S3 real
   streaming persistence/replay. Header/pair failures and cancellation must be
   observed through real provider/tool/SQLite fixtures.
4. **Daemon and local app:** shared runtime contract, S1 startup/attach plus P12
   explicit drain; S4 native interaction. Serialize all `main.rs` and
   `server/lib.rs` writers, with tests at each integrated boundary.
5. **Transport and workspace:** durable event producer/transport owner, then P5
   browser client; P13 scope/membership callers; P6 real account authority; P7
   all live tool dispatches through the broker.
6. **Installation and release:** P3/P4 bounded transactional install; P10 actual
   signed bundles with post-sign proof. Run installed parent journey V1 and
   integrity/graph checks V2/V3 on the exact integrated revision.
7. **Full Phase 1 release:** P14 accounts for every mandatory requirement and
   remaining remote/mobile/provider/OS-platform gates. Actual device/signing/
   platform evidence cannot be supplied by read-only source reviews.

No implementation milestone or release is complete while a repair child,
unwired caller, missing acceptance or disputed frozen contract remains.

## Proposal gate result

The assignment table was mechanically checked: exactly 21 rows, 11 Muse,
5 Luna and 5 DeepSeek. `python3 tools/validate_repository.py` was run through
the bounded supervisor with a 30-second deadline on this review branch. It
returned exit 1 with the same 51 backlog-exhaustion errors; platform protection
readback remained unverified. Therefore this document is preserved on the
proposal branch and is not merged as an approved product plan.

Canonical receipt SHA-256:
`0baa1abdf7d4e6e24c99b566dd12d9021d4d5d9e7eb639a6ae0cefdf8f63372e`.
Receipt directory:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/phase1-plan-canonical-20260930`.
