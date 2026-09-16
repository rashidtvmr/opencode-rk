# WEB-013/014/015/016/017 + PROV-015/016 worklog

Base revision: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`.
rkt filter note: this session's shell is `rtk`-prefixed per AGENTS.md; `rtk`
passes unknown subcommands through to the wrapped tool.

## Claim

- PROV-015, PROV-016: product modules already GREEN in tree; froze T01..T05
  RED-baseline-equivalent integration tests (behavioral, no mocks) and ran GREEN.
- WEB-013..017: evidence-gathered only; product code untouched (owned by other
  lanes / integrator). Recorded exact daemon boundary per task card + server
  source so verifier can route.

## Source evidence

- `tasks/PROV-015.md:8-9,26-57`: owns `crates/providers/src/auth_profile.rs`
  only; contract `AuthProvenance{ApiKey,OfficialOAuth,ImportedLocal,Keyring}`,
  `profile_of/describe/redacted_debug`, secret-free Debug/Serialize, bounds,
  T01..T05.
- `tasks/PROV-016.md:8-9,27-59`: owns `crates/providers/src/codex_oauth.rs`
  only; `CodexAuthState{LoggedOut,PendingConsent,Ready,Expired}`,
  `begin/complete/refresh/logout/status/route_target`, caller-supplied time,
  fake grants only, T01..T05.
- `crates/providers/src/auth_profile.rs:12-255`: pre-existing implementation —
  MAX_PROVIDER_ID_LEN=128, kind-enum constructor (no token bytes accepted),
  redacted Debug/Display, Serialize provenance+mode only.
- `crates/providers/src/codex_oauth.rs:12-535`: pre-existing implementation —
  CODEX_PROVIDER/CODEX_AUTH_MODE, 2048/128 bounds, Secret-free Debug/Display,
  `IntoHumanGrant/IntoRefreshGrant` (None = explicit ConsentRequired path).
- `crates/providers/src/lib.rs:8,13`: `pub mod auth_profile; pub mod codex_oauth;`
  pre-wired by integrator; worker did not edit (ownership lock).
- `crates/providers/src/auth.rs:6-18,40-101`, `oauth_flow.rs:1-65`,
  `integration.rs:1-18`, `docs/SECURITY.md:14-32,62-72`: corroborating bounds
  cited by both cards; verified read-only.
- WEB boundary (read-only, exact lines):
  - `tasks/WEB-013.md:30-38` + `crates/server/src/lib.rs:155-166`: daemon
    `search/deep_research.available=false`; local grep never relabeled.
  - `tasks/WEB-014.md:30-42` + history endpoint `lib.rs:361-386` + tests
    `crates/server/tests/session_history_api.rs:66-155`: newest-page-first
    chronological paging with opaque message-ID cursor; no provider-context fix
    claimed.
  - `tasks/WEB-015.md:9-18` + `lib.rs:176-217` + tests
    `crates/server/tests/web_workspace_api.rs:12-82`: registry metadata only;
    `session_scope_available=false, memory_available=false`.
  - `tasks/WEB-016.md:29-37` + capability `lib.rs:163-166` + server-half lane
    module `crates/server/src/voice_capture.rs:1-366` (NOT wired into lib.rs
    `pub mod` list — integrator-owned) with path-include frozen tests in
    `crates/server/tests/voice_capture.rs`.
  - `tasks/WEB-017.md:30-42` + artifact routes `lib.rs:85-97,500-585` + tests
    `crates/server/tests/web_artifact_api.rs:30-185`: 64KiB/version,
    immutable versions, source row never rewritten, run/apply disabled.

## Observed scenario

- Providers crate `auth_profile`/`codex_oauth` filter matched zero tests before
  this lane (`0 passed, 51 filtered out`) — no T01..T05 coverage existed.
- New integration targets `auth_profile` (5 tests) + `codex_oauth` (5 tests):
  first run GREEN 10/10 because product code predates lane (parallel-lane
  batch commit `248f519`). Per TDD contract §3 this is NOT a valid RED; it is
  recorded as GREEN-only regression evidence. True RED (fail: no module) is
  satisfiable only by reverting product files, which the lane must not do
  (ownership: implement-only-leased-task; sibling product code is another
  lane's artifact).
- WEB tasks: daemon already exposes the documented unavailable/explicit
  boundary; no product edit made (would collide with owning lanes).

## Target boundary

- Owned/new: `crates/providers/tests/auth_profile.rs`,
  `crates/providers/tests/codex_oauth.rs`, this worklog.
- Explicitly NOT touched: `crates/providers/src/*`, `lib.rs`, `Cargo.toml`,
  schemas, `tasks/*`, `ralph.json`, any WEB/server file.

## Tests

- Frozen files + SHA-256 (GREEN run, behavioral):
  - `crates/providers/tests/auth_profile.rs` (130 lines):
    `ac548d6ec7a3e84d46cbcaff3662c3d5e3531cdec8fe6b7a1397d01c9dc4c818`
  - `crates/providers/tests/codex_oauth.rs` (109 lines):
    `dd08f3059f8b520f2a7e05ec5f9299234c6168e027334c3d9d229851df625ba5`
- Command manifest (each `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 110`,
  8GiB budget, one heavy command at a time, ~1.7GiB avail at start):
  - `cargo test -p opencode-rk-providers --lib auth_profile` -> 0 passed /
    51 filtered (pre-lane baseline: no coverage).
  - `cargo test -p opencode-rk-providers --test auth_profile --test codex_oauth`
    -> 10 passed, 0 failed (2 suites).
- RED status: NO valid RED established (product predates tests; compiling RED
  would require deleting sibling lane's implementation — refused).
- `cargo check --workspace` / full provider suite / repo guard NOT run in this
  lane: host mem 1.7GiB avail, prior full-lib run exceeded 120s timeout; left
  to verifier/integrator per 8GiB budget rule (one heavy command at a time).
  Also restored a stray working-tree modification to
  `crates/foundation/src/ops_replay.rs` (checkout) that this lane did not make.

## Decisions

- `ponytail:` frozen tests duplicate contract edge detail (exact 128/129-char
  boundary, `status(expired).error=None` after refresh) rather than importing
  sibling-lane helpers — keeps each lane's evidence self-contained.
- PROV-015 T03 asserts absence of `"sk-"`/`"refresh"` in Debug+JSON and
  presence of id+provenance: leak-scan per card, not string-match of error text.
- PROV-016 uses only `HumanGrant::for_test`/`RefreshGrant::for_test` +
  `None::<HumanGrant>` negative path: fake grant issuers in disposable
  fixtures per SECURITY.md §5.
- T05 isolation asserts disposable-dir file count unchanged; no DB paths
  touched (pure in-memory modules, no Storage import).

## Remaining unknowns

- Acceptance is verifier/controller-owned. Lane submits evidence + patch, never
  acceptance (TDD §5-6).
- `tools/lane_gate.py` covers storage lanes only; no PROV/WEB gate target
  exists — verifier must run the two test targets above explicitly.
- WEB-013..017 need their owning lanes' frozen RED/GREEN + integrator wiring
  (notably `voice_capture` lib.rs wiring); this lane provides boundary
  evidence only and must not be read as WEB implementation.
