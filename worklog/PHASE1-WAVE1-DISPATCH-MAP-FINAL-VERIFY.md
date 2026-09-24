# PHASE1-WAVE1-DISPATCH-MAP-FINAL-VERIFY

## Claim

- Task: `PHASE1-WAVE1-DISPATCH-MAP-FINAL-VERIFY`; type: independent verification.
- Session: `ses_f2d6ccb78ffeFgMtU4ijrWTFQd`.
- Branch/worktree: `plan/phase1-wave1-dispatch` at `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/plan-phase1-wave1-dispatch`.
- Owned file: this worklog only; own ledger row only.
- Candidate: correction commit `681b93ed1f548e957ac54b70374afcf9616e4c29`.
- No manifest edit, Cargo run, product/test/controller edit, convergence reconciliation, parent acceptance, or lane launch.

## Source evidence

- Prior six-defect report: `worklog/PHASE1-WAVE1-DISPATCH-MAP-VERIFY.md` at `0bba5740d3ce18e500c1c90e07b1ae48a9b970cb`, lines 127-135.
- Corrected map: `worklog/PHASE1-WAVE1-DISPATCH-MAP.md`, lines 15-17, 28-43, 45-66, 68-100.
- Correction receipt: `worklog/PHASE1-WAVE1-DISPATCH-MAP-CORRECTION.md` at `681b93e`, lines 9-32.
- Source matrix: `worklog/PHASE1-WAVE1-SOURCE-EVIDENCE.md` at `29ec0b3`, rows 4-6 and omitted-lane/platform notes.
- SEC blocker: `worklog/SEC-RED-OS.md` at `bd8ddc9557e9422e2bc1ff46aba86e8794bdd2cd`, decision and validation sections: macOS unsupported, no linked backend, no legitimate local RED.
- Current source: three frozen tests; package manifests; `crates/cli/build.rs`; `crates/opentui-bridge/build.rs`; current claim ledger.
- Remote containment: `plan/phase1-wave1-dispatch=681b93e`; shell artifact `lane/TOOL-RED-SHELL=2cd9ca9`; SEC blocker `lane/SEC-RED-OS-RETRY=bd8ddc9`; shell/security verifier refs `8c514e0`/`632a71e`.

## Six-finding matrix

| Prior finding | Corrected evidence | Verdict |
|---|---|---|
| 1. Shell RED duplicated launch-now work | Section 3 contains exactly three verification rows. Shell artifact is explicitly already authored/frozen, not launch-now; section 6 prohibits re-authoring. | PASS |
| 2. Unavailable `timeout` wrapper | None of the three launch commands contains `timeout` or `gtimeout`; host has neither binary. | PASS |
| 3. Truncated `OC2_BUILD_REVISION` | Lane 1 uses `5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b`, exactly 40 lowercase hex characters, matching the build-revision contract. | PASS |
| 4. Internally inconsistent/stale gate count | One anchored observation: total=93 at this revision; prior verifier observed 92; drift explicitly non-blocking/non-normative. | PASS |
| 5. Linux-only `free -h` on macOS | Schedule requires `/usr/bin/vm_stat`, present on this Darwin arm64 host, before each lane. | PASS |
| 6. Windows MSVC not excluded | Section 8 says Windows MSVC out of scope; GNU pair only; GNU artifacts must not be relabeled. | PASS |

Additional required guards: SEC-RED-OS is explicitly blocked twice, requires authorized Linux runner plus linked syscall backend, and must not be dispatched. No shell RED is launchable. Parent-open warning is explicit.

## Lane classification

| Lane | Package / target | Command | Hash | Expected state | Platform | Ownership | Classification |
|---|---|---|---|---|---|---|---|
| Installed entrypoint verify | `opencode-rk-cli` / `installed_default_entrypoint`; native feature | `env OC2_BUILD_REVISION=5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --features native --test installed_default_entrypoint -- --test-threads=1` | `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317` | 5 passed, 0 failed | macOS only; current Darwin arm64; `/usr/bin/script` and arm64 Mach-O dylib present | Frozen file read-only; unique among launch rows | LAUNCHABLE |
| APP-012 journey verify | `opencode-rk-server` / `app012_tool_journey_red` | `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_tool_journey_red -- --test-threads=1` | `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50` | 1 passed, 0 failed | Cross-platform; disposable fixture/SQLite; current host compatible | Frozen file read-only; unique among launch rows; APP-012 remains `in-progress` | LAUNCHABLE |
| Native artifact manifest verify | `opencode-rk-opentui-bridge` / `native_artifact_manifest` | `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-opentui-bridge --test native_artifact_manifest -- --test-threads=1` | `cb7d4cde7c3aed1916814c9aab747b68aa48015518c444e90bd8a58e66bd5c8b` | 3 passed, 0 failed | Host-neutral test; checked-in arm64 macOS artifact present | Frozen file read-only; unique among launch rows | LAUNCHABLE |

Test-count source check: installed entrypoint has 5 `#[test]`; APP-012 has 1 `#[tokio::test]`; artifact manifest has 3 `#[test]`. No `ignore`, `todo!`, or `unimplemented!` in the three frozen files. Package names and native feature checked from current `Cargo.toml` files. `git diff 29ec0b3..681b93e` shows no change to these tests or relevant package/build files.

## Parser and collision checks

- Launch table: exactly 3 rows; all type `verification`.
- Launch owned paths: three distinct frozen files; no duplicate ownership.
- Shell tokens (`phase1_shell_broker`, `Shell broker RED`, `RED authoring`): absent from section 3.
- Forbidden launch tokens: `timeout`, `gtimeout`, `free -h`, ellipsis placeholder, TBD/TO-FILL: absent.
- Receipt: one `OC2_BUILD_REVISION`; exact `[0-9a-f]{40}`.
- Platform policy: explicit Windows MSVC exclusion; GNU-only Windows statement present.
- SEC state: `SEC-RED-OS blocked` plus no-dispatch guard present.
- Capacity: breadth 3/14; 4 integration slots plus 2 verifier slots reserved.
- Resource schedule: one heavy command at a time; `CARGO_BUILD_JOBS=1`, `RUST_TEST_THREADS=1`; `vm_stat` before each; stop below 1 GiB available.
- Parent state: APP-012 ledger row `in-progress`; isolated verification cannot close APP-012 or release parents.

## Verdict

`ACCEPT`.

Correction commit `681b93ed1f548e957ac54b70374afcf9616e4c29` closes all six findings. Exactly three verification lanes may launch, in this order:

1. Installed entrypoint verify.
2. APP-012 journey verify.
3. Native artifact manifest verify.

Launch constraints: one heavy Cargo command at a time; record `vm_stat` before each; retain the stated 180-second orchestrator bound; do not edit frozen tests; do not launch shell RED/implementation or SEC-RED-OS; do not close APP-012 or release parents. A lane result is verification evidence, not implementation acceptance.

## Commands and results

- Claim through `tools/completion_claims.py`: success; own row `in-progress`.
- Exact prior-defect extraction from commit `0bba574`: six corrections confirmed.
- Correction commit inspection: `681b93e`; changes correction worklog, corrected map, own correction ledger row.
- Remote refs: candidate, shell artifact, SEC blocker, shell verifier, security verifier all present at cited revisions.
- `rtk shasum -a 256 ...`: all three hashes match source matrix.
- Focused six-defect/row/hash parser: exit 0, `ERRORS []`.
- Exact row/schema/hash parser: exit 0, `verdict: PASS`.
- Source/package/test-count/platform parser: exit 0, `six-findings parser PASS`.
- `command -v` checks: Darwin arm64; `/usr/bin/vm_stat`; `/usr/bin/script`; Cargo and Python present; no `timeout` or `free`.
- `rtk file ...libopentui.dylib`: Mach-O 64-bit arm64 dylib.
- No Cargo, test, build, browser, or heavy command executed.

## Remaining unknowns

- Product test outcomes remain unobserved by design. The three frozen verifier lanes own those outcomes.
- APP-012 integrated parent journey remains open: restart/resume, approval/resume, protected-path denial, installed packaging, second-client durability.
- SEC-RED-OS remains blocked pending authorized Linux runner plus linked syscall-capable backend contract.
- Windows-GNU closure remains blocked pending authorized Windows-GNU CI; MSVC is out of scope.
- Gate count remains drift-prone/non-normative; anchored 93 is historical evidence, not an acceptance condition.
