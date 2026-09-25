# PHASE1-VERTICAL-LINUX-MAP - vertical Phase 1 plan for the Linux product journey

Claim: session `ses_f295edd5fffeGej4z7teI3s0aI`, continued after lawful reclaim by
orchestrator session `ses_f3c4de578ffelQv59xDXmOs03B`; scratchpad
`worklog/PHASE1-VERTICAL-LINUX-MAP.md`.
Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/plan-phase1-vertical-linux`.
Baseline: `plan/PHASE1-VERTICAL-LINUX` at `8a91a7b49a5a1c948218ad8f176d44e015530dcb` (== `origin/main`) before this artifact and ledger edit.
Scope: planning artifact, scratchpad and own ledger row only. No Cargo/source/test/PLAN/ralph/task/controller edit. No acceptance claimed.
Status: proposal audited and read-only validated; candidate completion still
requires the ledger transition, commit, push, and remote equality check.

## 0. Branch and prior-art verification (before writing)

- Baseline `git status --short --branch` was clean, branch `plan/PHASE1-VERTICAL-LINUX`, HEAD == `origin/main` at `8a91a7b`. The current worktree intentionally contains only this artifact and the own ledger-row edit.
- This session holds the reclaimed `PHASE1-VERTICAL-LINUX-MAP` row. Findings below are source-verified in this artifact; chat-only claims are not treated as evidence.
- Sibling pattern exists: `PHASE1-VERTICAL-WINDOWS-GNU-MAP` and `PHASE1-VERTICAL-PRODUCT-MACOS-MAP` rows are `not-started`. This map proposes Linux lanes only and does not alter sibling rows.
- `python3 tools/validate_repository.py` = FAIL (`validate_backlog_exhaustion: 51 error(s)`, pre-existing, controller-owned). `python3 tools/convergence_gate.py` = CONVERGENCE BLOCKED, total=58 (pre-existing off-plan ledger rows). Both block commit-time repository guard, not this audit. Recorded, not fixed (out of lease).

## 1. Verified prior findings (file:line evidence)

Each row is what the tree actually says at baseline `8a91a7b`. "Prior finding" = the chat summary this task asked me to verify; verdict is mine.

| Prior finding | Verdict | Exact evidence at `8a91a7b` |
|---|---|---|
| Landlock detection-only / fail-closed backend | CONFIRMED | `crates/security/src/os_backend.rs:120` hardcodes `let enforcement_linked = false;`; `:171 pub fn engage` always returns `Err(Blocked)` with "no syscall backend linked"; `crates/security/src/platform_matrix.rs:155 pub fn enforce` always `Err(UnsupportedSandbox)`. Detection only: `os_backend.rs:338 fn detect_landlock` = `/proc/version` >= 5.13 AND `:363 fn filesystems_has_landlock` (`/proc/filesystems`). `crates/security/src/sandbox_real.rs:37` mirrors detection. `crates/security/src/lib.rs:2 #![forbid(unsafe_code)]` blocks syscall wrappers in-crate. No real kernel enforcement anywhere. |
| x64 native present | CONFIRMED | `crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so`, ELF 64-bit x86-64 shared object, 26,583,552 bytes, sha256 `e84ced36b9f0d77069067833f1dd0edcc6324abdd5e4be662e1dccd2994b5d9a`. |
| arm64 native absent | CONFIRMED | `ls crates/opentui-bridge/native/lib/` on this branch = only `x86_64-unknown-linux-gnu`. `git ls-tree -r origin/main -- crates/opentui-bridge/native/` = one `.so` only. No `aarch64-unknown-linux-gnu`. |
| POSIX installer | CONFIRMED (weaker than the 427-line variant) | `scripts/install-oc2.sh` on `main` = 144 lines. Has checksum pre-write (`:80-87`), legacy-name refusal exit 73 (`:90-93`), staged `--version`/`--help` identity gates exit 74 (`:102,:122`). **Lacks** native `libopentui` install, member allowlist, 128 MiB probe, rollback: `grep "MAX_ARCHIVE\|EXPECTED_NATIVE\|LIB_SUFFIX\|tar -t"` = empty. The bounded 427-line installer (member allowlist `MAX_ARCHIVE_MEMBERS`, `MAX_ARCHIVE_PAYLOAD=134217728`, symlink refusal, atomic mv + rollback) exists only on `plan/packaging-matrix` and `lane/CROSS-PLATFORM-RUNNER-CLOSURE-VERIFY`, **not integrated**. |
| daemon auth/TUI/web exist | CONFIRMED | `crates/server/src/daemon_auth.rs:45 pub fn mint` (32 random bytes, hex, `/dev/urandom`), `:72 verify_bearer` (constant-time), `:151 require_bearer` middleware. `crates/server/src/lib.rs:174` uses it; `:268-329` applies bearer to `/api/*`. `crates/cli/src/main.rs:222 None =>` calls `:229 discover_presence` and `:231 plan_default_launch`; `:645 async fn serve`, `:693 router_with_auth(... Some(credential))`. Web: `web/src/*.test.tsx` suites; `crates/cli/tests/web_entrypoint.rs`, `web_singleton_runtime.rs`. |
| APP012 | CONFIRMED OPEN, not a ledger row | `tasks/completion/local.json` story `APP-012` deps `[APP-008, APP-010, APP-011, TUI-010]` (all completed in ledger), `paths: [tests/e2e/local_application.rs]`. That path **does not exist** (`tests/` holds only `bootstrap/`, `fixtures/`). No `APP-012` row in `tasks/completion/claims.json`. Read-path RED (`app012_tool_journey_red.rs`) lives only on `lane/APP-012-TOOL-RED`. |
| resource cleanup | PARTIAL | Only `worklog/CLEANUP-TRIAGE.md` on this branch; no dedicated resource-lane artifact. `config/resource-targets.json` targets exist (`idleDaemonRssMiB: 64`, `maxOpenWorkspaceStores: 4`, whole-process-tree requirement). No Linux whole-tree measurement lane landed. |
| CI runner/SBOM gaps | CONFIRMED | `.github/workflows/ci.yml:33 matrix.os: [ubuntu-latest, windows-latest]`. No `macos`, no `--target x86_64-unknown-linux-gnu`/arm64 cross build, no artifact assembly, no provenance/SBOM step, no release job. Only two workflows: `ci.yml`, `completion.yml`. No `release.yml`, no `release/`, no `tests/release/`. `crates/opentui-bridge/native/sbom.json` and `artifacts.json` are **absent on `main`** (present only on `e4bbb11`/packaging-matrix). |

Additional integrity finding not in the prior summary, verified here:

- **Manifest/tree hash divergence.** `origin/main` tracks a Linux x64 `.so` whose content sha256 is `e84ced36...` (26,583,552 bytes). The native manifest on pushed candidate `origin/lane/CROSS-PLATFORM-RUNNER-CLOSURE-VERIFY` at `2d04c1c` records `x86_64-unknown-linux-gnu` sha256 `9f074adf1e3c67bb027433d44da05b9e285a37ae58cf0b2ed76504304450c79e` (26,627,832 bytes) and its `native/lib/.../libopentui.so` content matches that. So the checked-in `main` blob is **not** the manifest-verified artifact. Any Linux packaging lane must re-bind the artifact and manifest on the integrated revision; do not assume either candidate or main is the released one.

## 2. Hard external blocker (governs the whole vertical)

The Linux product journey needs a **real Linux kernel** runner. This host is `Darwin arm64` (`uname -a`), `rustup target list --installed` = `aarch64-apple-darwin` only, and `docker` is installed but its daemon is not running (`dial unix /Users/mymac/.docker/run/docker.sock: no such file or directory`). Consequences:

- Landlock kernel enforcement **cannot** be proven on this host. `engage()` currently returns `Blocked`, and there is no linked syscall backend. A unit test that never attaches a ruleset is explicitly **not** isolation evidence (`docs/SECURITY.md:49-56`).
- The Linux installed-entrypoint RED cannot execute here; `crates/cli/tests/installed_default_entrypoint.rs:1 #![cfg(target_os = "macos")]` compiles out on Linux and the macOS file is not Linux proof.
- CI runner/SBOM/release work is CODEOWNER-protected (`.github/CODEOWNERS` `* @rashidtvmr`), and `gh` is not installed, so no hosted run or PR proof may be fabricated.

Therefore every Linux slice below declares its **runner** and marks kernel-dependent execution as blocked-until-Linux-runner. No lane may claim sandbox isolation from a unit-only test.

## 3. Wave structure (20-worker harness: 4 integration-spine + 2 verifier reserved; <=14 breadth)

- **Wave 1 (pre-wire + build the enforcement and installed foundation).** First serialize workspace/dependency prewire, then author the Linux Landlock RED against the existing API. Implementation starts only after the compiling RED is frozen. Packaging inputs remain candidate refs until an integration lane binds them to the integrated tree.
- **Wave 2 (wire the Linux user journey on the shared engine).** Approval/resume channel, Linux installed APP-012 journey, whole-tree resource ledger. One shared-`lib.rs` serialized integration owner.
- **Wave 3 (release closure and independent verification).** Protected unsigned Linux closure workflow consuming SBOM/manifest, plus reserved verifier lanes re-running frozen suites on the exact integrated revision.

Reserved throughout: **INT-LINUX-1..INT-LINUX-4** integration capacity and
**VER-LINUX-1..VER-LINUX-2** independent verifier capacity. `INT-LINUX-1` is
the only concrete integration entry in this domain map. `INT-LINUX-2..4` are
capacity labels, not dependency nodes; the global synthesis must instantiate
their shared-file work as sequential one-file stages. Seven current breadth
entries plus any explicitly added packaging child remain below the 14-lane
breadth ceiling; each executable stage has one owned file.

## 4. Proposed task entries (JSON only)

Schema note: existing plan stories use `{id,title,deps,paths,journey,tests}` (`tasks/completion/local.json`). Linux verticals additionally need runner/owner/verifier/evidence, so each entry carries those keys. IDs `LINUX-0xx` are proposed lane ids for `tasks/completion/discovered.json`; `INT-LINUX-*` and `VER-LINUX-*` are reserved integration/verifier lanes. Nothing here edits `ralph.json` or a task card.

```json
{
  "wave": 1,
  "entries": [
    {
      "id": "LINUX-001",
      "title": "Land the manifest-bound Linux native artifact set and bounded POSIX installer on main",
      "kind": "integration",
      "deps": ["TUI-011"],
      "paths": ["crates/opentui-bridge/native/artifacts.json"],
      "journey": "A Linux user runs the POSIX installer and receives oc2 plus the matching libopentui.so for their architecture, with the archive bounded by member/byte limits and rollback on identity failure, and every shipped byte tied to a manifest hash.",
      "source_evidence": "origin/main tracks only lib/x86_64-unknown-linux-gnu/libopentui.so (sha256 e84ced36..., 26583552 bytes); artifacts.json/sbom.json/build_opentui.sh absent on main, present at pushed candidate origin/lane/CROSS-PLATFORM-RUNNER-CLOSURE-VERIFY @2d04c1c with x86_64 hash 9f074adf... (26627832 bytes); scripts/install-oc2.sh on main = 144 lines with no MAX_ARCHIVE/EXPECTED_NATIVE bounds versus the 427-line bounded variant on pushed candidate.",
      "owned_file": "crates/opentui-bridge/native/artifacts.json",
      "shared_serialized_owner": "INT-LINUX-1 binds this manifest to the exact integrated payload commit; SBOM, build script, notices, native blobs and installer are separate serialized inputs, not this lane's owned files",
      "runner": "linux x86_64 (build/verify) plus local macos for byte-presence checks only",
      "red_owner_file": "crates/opentui-bridge/tests/native_artifact_manifest.rs (test-author, read/refreeze)",
      "verifier": "VER-LINUX-1",
      "deps_notes": "TUI-011 completed per ledger; this lane is the integration of its artifacts to main",
      "negative_security_resource": ["manifest entry whose hash/size differs from the shipped payload is rejected", "x86_64 and aarch64 Linux entries cannot silently point at one another", "manifest must not claim signing or authenticity"],
      "commands": [
        "shasum -a 256 crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so",
        "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-opentui-bridge --test native_artifact_manifest -- --test-threads=1",
        "bash -n scripts/install-oc2.sh",
        "python3 tools/validate_repository.py"
      ],
      "evidence": [
        "content sha256 of each shipped lib matches artifacts.json entry on the integrated revision",
        "integrated installer member/byte/identity/rollback vectors pass against disposable fixtures",
        "no manifest hash equals the stale e84ced36 blob unless the manifest was re-bound deliberately"
      ],
      "external_blockers": ["repository guard currently FAIL (pre-existing backlog-exhaustion); .github untouched by this lane"]
    },
    {
      "id": "LINUX-002",
      "title": "Real Landlock kernel enforcement backend that attaches a ruleset and closes inherited capabilities",
      "kind": "test-author",
      "deps": [],
      "paths": ["crates/security/tests/phase1_landlock_enforcement.rs"],
      "journey": "A generated tool child on Linux runs inside an attached Landlock ruleset: allowed fixture access succeeds; access outside the grant is denied by the kernel with no file created; inherited descriptors and environment are closed before exec.",
      "source_evidence": "crates/security/src/os_backend.rs:120 enforcement_linked=false; :171 engage() always Err(Blocked); crates/security/src/platform_matrix.rs:155 enforce() always Err(UnsupportedSandbox); crates/security/src/lib.rs:2 #![forbid(unsafe_code)]; docs/SECURITY.md:49-56 requires a test that actually runs on the target platform.",
      "owned_file": "crates/security/tests/phase1_landlock_enforcement.rs (test-author only)",
      "shared_serialized_owner": "none (new file); the backend crate wiring is the sibling LINUX-003 impl lane",
      "runner": "linux >= 5.13 with Landlock in /proc/filesystems (REQUIRED; blocked on macos host)",
      "red_owner_file": "crates/security/tests/phase1_landlock_enforcement.rs",
      "verifier": "VER-LINUX-2",
      "deps_notes": "test-author defines the observable contract against an explicit integration seam; INT-LINUX-1 pre-wires the workspace/dependency shape before RED. RED must compile and fail for absent enforcement; never wait for or use an implementation stub.",
      "negative_security_resource": [
        "write outside the grant root leaves no file on disk (absence asserted, not just an error string)",
        "read of a secret path (/etc/passwd outside grant; a sentinel .env) is denied by the kernel",
        "inherited fd to an out-of-grant file cannot be read by the child after engage",
        "child killed and reaped on violation; /proc/self/fd count stable across 50 setup/teardown cycles",
        "kernel without Landlock returns explicit BLOCKED, never a silent allow"
      ],
      "commands": [
        "grep -i landlock /proc/filesystems",
        "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 300 cargo test -p opencode-rk-security --test phase1_landlock_enforcement -- --test-threads=1",
        "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-security --lib os_backend"
      ],
      "evidence": [
        "receipt says enforcement engaged (ruleset attached), not detection-only",
        "denied-write marker absent on disk",
        "fd count before/after teardown equal"
      ],
      "external_blockers": ["needs a real Linux kernel runner; this host is Darwin arm64 and the local docker daemon is not running"]
    },
    {
      "id": "LINUX-003",
      "title": "Linkable Landlock syscall backend crate consumed by os_backend engage()",
      "kind": "implementation",
      "deps": ["LINUX-002"],
      "paths": ["crates/security-landlock/src/lib.rs"],
      "journey": "The native daemon can actually engage kernel isolation: engage() attaches a Landlock ruleset for the declared grants and returns Ok only when the kernel accepted it, wiring the real backend into the existing fail-closed contract.",
      "source_evidence": "crates/security/src/os_backend.rs:151 require_supported and :171 engage are the callers; :249 run_confined and :283 restricted_spawn are the existing path/capability helpers; crates/security/src/lib.rs:2 forbids unsafe, so a syscall backend must live in a separate crate.",
      "owned_file": "crates/security-landlock/src/lib.rs (new crate; the only file with unsafe)",
      "shared_serialized_owner": "INT-LINUX-1 pre-wires workspace Cargo.toml member, crates/security/Cargo.toml dependency and any required safe API seam before RED; this lane edits only its new crate file",
      "runner": "linux >= 5.13 (integration of the LINUX-002 RED happens on Linux)",
      "red_owner_file": "crates/security/tests/phase1_landlock_enforcement.rs (frozen; do not edit)",
      "verifier": "VER-LINUX-2",
      "deps_notes": "LINUX-002 must be frozen RED first (compiling, failing for missing backend); no placeholder backend or compile-failing test is acceptable",
      "negative_security_resource": [
        "engage() with an empty grant list refuses rather than granting everything",
        "ABI/feature probe failure returns typed Blocked, never Ok",
        "no panic on kernel older than Landlock ABI v1",
        "no unbounded retained ruleset handle; one handle per engage, released on drop",
        "does not silently widen access: only declared roots are added to the ruleset"
      ],
      "commands": [
        "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 300 cargo test -p opencode-rk-security --test phase1_landlock_enforcement -- --test-threads=1",
        "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-security --lib",
        "CARGO_BUILD_JOBS=1 cargo check --workspace --all-targets"
      ],
      "evidence": [
        "phase1_landlock_enforcement green on Linux with ruleset-engaged receipt",
        "security lib suite no-regress",
        "workspace check clean"
      ],
      "external_blockers": ["crate addition touches workspace Cargo.toml (integrator); Linux runner required for GREEN"]
    }
  ]
}
```

```json
{
  "wave": 2,
  "entries": [
    {
      "id": "LINUX-004",
      "title": "Human approval and resume channel in the live turn path",
      "kind": "implementation",
      "deps": ["APP-012-APPROVAL-RESUME-CORRECTION"],
      "paths": ["crates/server/src/approval_resume.rs"],
      "journey": "When a tool authorization returns RequireHuman, the turn parks with a durable approval request, the user approves or denies, and the same turn resumes and emits the tool result instead of terminating with an error.",
      "source_evidence": "crates/server/src/lib.rs:1167-1201 invokes the real broker but converts RequireHuman directly into a terminal error; worklog/APP-012.md records no approval-resume channel in this turn caller. Any later line references must be refreshed against the integrated revision.",
      "owned_file": "crates/server/src/approval_resume.rs (new module)",
      "shared_serialized_owner": "integrator owns the single crates/server/src/lib.rs caller edit and one mod line; only one lane may hold lib.rs at a time",
      "runner": "any host (Tokio loopback + disposable SQLite); Linux runner additionally required for the LANE-008 installed variant",
      "red_owner_file": "crates/server/tests/approval_resume_threading.rs",
      "verifier": "VER-LINUX-1",
      "deps_notes": "APP-012 remains open. The approval/resume correction is a prerequisite child, not proof of the parent journey; parent closure still requires installed Linux execution, restart/resume, brokered tool path and independent verification.",
      "negative_security_resource": [
        "a denied approval resolves the turn to a stable denied state with no process or file side effect",
        "a policy version bump invalidates a pending approval; stale approval is rejected without side effect",
        "approval must not be satisfiable by the wildcard permission or by a prompt",
        "pending approval queue is bounded; overflow rejects with typed backpressure",
        "secrets in the approval payload are redacted from logs/history/errors"
      ],
      "commands": [
        "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 cargo test -p opencode-rk-server --test approval_resume_threading -- --test-threads=1",
        "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 cargo test -p opencode-rk-server --lib"
      ],
      "evidence": [
        "approve path resumes and persists a tool result row",
        "deny path leaves the fixture file byte-identical",
        "stale approval rejected after policy bump"
      ],
      "external_blockers": []
    },
    {
      "id": "LINUX-005",
      "title": "Installed Linux APP-012 golden journey from a clean HOME",
      "kind": "implementation",
      "deps": ["LINUX-001", "LINUX-004", "TUI-010"],
      "paths": ["tests/e2e/local_application.rs"],
      "journey": "Install oc2 on Linux, run it with no subcommand from a fresh HOME, use in-app setup, complete a coding turn with a brokered file tool plus approval, attach a second client to the same session, restart the daemon and resume the same history.",
      "source_evidence": "tasks/completion/local.json APP-012 paths [tests/e2e/local_application.rs] which does not exist; crates/cli/tests/installed_default_entrypoint.rs:1 is #![cfg(target_os = \"macos\")] so macOS-only; worklog/APP-012.md lists restart/resume, approval, protected-path denial and packaged proof as still open.",
      "owned_file": "tests/e2e/local_application.rs (test-author authors it; one file)",
      "shared_serialized_owner": "integrator owns the staging wrapper that invokes the installed binary; no product file edited by the test-author",
      "runner": "linux x86_64 with PTY (script(1) or equivalent); REQUIRED",
      "red_owner_file": "tests/e2e/local_application.rs",
      "verifier": "VER-LINUX-2",
      "deps_notes": "APP-012 is open, not a completed dependency. Run against the LINUX-001 installed layout, not a cargo-run binary; approval/resume, restart/resume and protected-path proof remain acceptance prerequisites.",
      "negative_security_resource": [
        "bare launch on piped stdin takes the documented headless path and never hangs in raw mode",
        "restart mid-turn restores truthful state, never a fabricated completed response",
        "protected-path read (.env) is denied with zero side effect",
        "only one store owner exists; a second launch attaches instead of forking a database",
        "process-tree RSS at idle stays within config/resource-targets.json idleDaemonAndNativeTuiRssMiB"
      ],
      "commands": [
        "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 300 cargo test -p opencode-rk-cli --features native --test local_application -- --test-threads=1",
        "readelf -d target/release/oc2 | grep NEEDED",
        "readelf -d target/release/native/lib/linux-x64/libopentui.so | grep NEEDED"
      ],
      "evidence": [
        "installed binary resolves libopentui.so without LD_LIBRARY_PATH",
        "tool output persisted and observed by the second client",
        "restart resumes the same session id and history"
      ],
      "external_blockers": ["needs Linux PTY runner; macOS host cannot execute (cfg target_os)"]
    },
    {
      "id": "LINUX-006",
      "title": "Linux whole-process-tree resource ledger and disabled-feature zero-cost proof",
      "kind": "implementation",
      "deps": ["OPS-001"],
      "paths": ["crates/foundation/src/resource_ledger_linux.rs"],
      "journey": "Operators get typed admission caps from config/resource-targets.json (queue counts and byte counts with reservations before admission) and an attestation that disabled optional subsystems start zero tasks, threads, watchers and stores, measured on the whole daemon process tree.",
      "source_evidence": "tasks/OPS-001.md (ledger + disabled-feature zero-cost, whole-tree measurement, no silent history deletion); config/resource-targets.json targets idleDaemonRssMiB 64, maxOpenWorkspaceStores 4; no Linux whole-tree lane landed (only worklog/CLEANUP-TRIAGE.md).",
      "owned_file": "crates/foundation/src/resource_ledger_linux.rs (new module)",
      "shared_serialized_owner": "integrator owns the foundation lib.rs mod line",
      "runner": "linux (reads /proc for the process tree and /proc/self/fd)",
      "red_owner_file": "crates/foundation/tests/resource_ledger_linux.rs",
      "verifier": "VER-LINUX-1",
      "deps_notes": "OPS-001 is accepted; this lane adds the Linux measurement path only",
      "negative_security_resource": [
        "a queue beyond its count or byte cap is rejected with typed backpressure, not unbounded growth",
        "a disabled subsystem construct attempt performs zero I/O and allocates zero retained bytes",
        "measurement covers the daemon process tree, never the daemon alone",
        "no user history is deleted to satisfy a byte target; pruning is reported, not silent",
        "fd and thread counts do not grow across repeated enable/disable cycles"
      ],
      "commands": [
        "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 cargo test -p opencode-rk-foundation --test resource_ledger_linux -- --test-threads=1",
        "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-foundation --lib"
      ],
      "evidence": [
        "measured whole-tree RSS recorded for idle and active profiles",
        "disabled-feature zero-cost attestation with zero task/thread/store counts",
        "queue overflow returns explicit backpressure error"
      ],
      "external_blockers": ["/proc-based measurement requires a Linux runner"]
    }
  ]
}
```

```json
{
  "wave": 3,
  "entries": [
    {
      "id": "LINUX-007",
      "title": "Unsigned Linux release closure workflow consuming manifest, SBOM and checksums",
      "kind": "integration",
      "deps": ["LINUX-001", "LINUX-005", "SHIP-001"],
      "paths": [".github/workflows/release.yml", "scripts/write-release-manifest.py"],
      "journey": "A protected, unsigned Linux x86_64 job builds the native app, packages oc2 plus libopentui.so, installs into a disposable HOME, runs the installed Linux APP-012, and attaches a bounded JSON receipt binding source revision, toolchain, artifact hashes, SBOM hash and test-log hash.",
      "source_evidence": ".github/workflows/ci.yml:33 matrix has only ubuntu-latest/windows-latest, no --target build, no artifact upload, no SBOM step; no release.yml or release/ dir exists; crates/opentui-bridge/native/sbom.json and artifacts.json are absent from main.",
      "owned_file": "scripts/write-release-manifest.py",
      "shared_serialized_owner": "INT-LINUX-3 owns the separate .github/workflows/release.yml CODEOWNER-gated commit; this lane does not lease that workflow file",
      "runner": "hosted ubuntu-latest (x86_64) unsigned; no signing identity",
      "red_owner_file": "tests/bootstrap/test_release_workflow_contract.py (new, bootstrap-style stdlib validator)",
      "verifier": "VER-LINUX-2",
      "deps_notes": "workflow definition can land before runner proof; runner proof cannot be fabricated",
      "negative_security_resource": [
        "workflow must upload an explicit artifact list bound to a revision, never a mutable 'latest'",
        "receipt marks unsigned true and signing not-run; no signing or notarization claim is made",
        "logs are bounded and redact environment",
        "no secret is introduced for the unsigned job",
        "a failed installed test fails the job; no continue-on-error"
      ],
      "commands": [
        "python3 -m unittest discover -s tests/bootstrap -p 'test_release_workflow_contract.py' -v",
        "python3 tools/validate_repository.py",
        "python3 tools/render_ruleset_import.py",
        "python3 tools/verify_ruleset_readback.py"
      ],
      "evidence": [
        "workflow contract validator green locally",
        "hosted run receipt with revision, target, artifact hashes, SBOM hash, unsigned true",
        "artifact download installs and runs on a clean runner"
      ],
      "external_blockers": [
        "CODEOWNER review and merged PR required for .github changes",
        "hosted runner access and branch protection readback; gh not installed locally",
        "signing/notarization identities unavailable; this Linux plan does not claim signing, notarization, hosted-runner success or release acceptance"
      ]
    },
    {
      "id": "INT-LINUX-1",
      "title": "Integration spine: main-tree artifact/installer landing and lib.rs wiring",
      "kind": "integration",
      "deps": ["LINUX-001", "LINUX-003", "LINUX-004", "LINUX-006"],
      "paths": ["crates/server/src/lib.rs", "Cargo.toml", "crates/foundation/src/lib.rs", "crates/security/src/lib.rs"],
      "journey": "The integration owner lands each lane's shared-file wiring exactly once (workspace members, one mod line per new module, one caller edit) and verifies the merged tree before dependents unlock.",
      "source_evidence": "AGENTS.md serialized-integration rule; docs/CONVERGENCE.md integration-before-breadth; crates/security/src/lib.rs mod list; multiple lanes would otherwise race on lib.rs.",
      "owned_file": "the shared lib.rs/mod lines (serialized; one holder at a time)",
      "shared_serialized_owner": "INT-LINUX-1",
      "runner": "any (cargo check) plus Linux for kernel lanes",
      "red_owner_file": "none (integration does not author tests)",
      "verifier": "VER-LINUX-1",
      "deps_notes": "runs after each wave; unlocks dependents only on a green merged tree",
      "negative_security_resource": [
        "no lane edits a frozen test to pass",
        "no second store owner is introduced by a wiring change",
        "no unbounded queue/retained output added by wiring"
      ],
      "commands": [
        "CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 300 cargo check --workspace --all-targets",
        "python3 tools/convergence_gate.py"
      ],
      "evidence": ["merged-tree check clean", "convergence gate movement recorded, not asserted"],
      "external_blockers": ["repository guard FAIL is pre-existing and controller-owned"]
    },
    {
      "id": "VER-LINUX-1",
      "title": "Independent verifier: frozen-suite rerun on exact integrated revision",
      "kind": "verifier",
      "deps": ["INT-LINUX-1"],
      "paths": [],
      "journey": "An independent verifier reruns every frozen Linux suite on the exact integrated commit and rejects any lane whose self-report disagrees with the on-disk artifact.",
      "source_evidence": "docs/TDD.md; AGENTS.md lane gating; tools/lane_gate.py re-checks artifacts rather than trusting self-reports.",
      "owned_file": "none",
      "shared_serialized_owner": "VER-LINUX-1",
      "runner": "linux x86_64 (one heavy command at a time, CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1)",
      "red_owner_file": "none",
      "verifier": "self (independent of implementers)",
      "deps_notes": "runs after each integration wave",
      "negative_security_resource": [
        "a green that needed a test edit is FAIL",
        "a sandbox claim from a unit-only test is FAIL",
        "resource measurements must cover the process tree"
      ],
      "commands": [
        "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 300 cargo test -p opencode-rk-security --test phase1_landlock_enforcement -- --test-threads=1",
        "CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 300 cargo test -p opencode-rk-cli --features native --test local_application -- --test-threads=1",
        "python3 tools/convergence_gate.py"
      ],
      "evidence": ["per-suite positive counts on the integrated commit", "frozen hash unchanged"],
      "external_blockers": ["needs Linux runner"]
    },
    {
      "id": "VER-LINUX-2",
      "title": "Independent verifier: artifact/manifest/installer integrity auditor",
      "kind": "verifier",
      "deps": ["LINUX-001", "LINUX-007"],
      "paths": [],
      "journey": "An independent auditor re-hashes the shipped Linux native bytes and installer vectors against artifacts.json/sbom.json and rejects any divergence between the checked-in blob and the manifest.",
      "source_evidence": "origin/main x86_64 .so sha256 e84ced36... versus manifest 9f074adf... at e4bbb11 (divergence verified in section 1); eq worklog/PHASE1-PACKAGING-MATRIX-MAP.md matrix.",
      "owned_file": "none",
      "shared_serialized_owner": "VER-LINUX-2",
      "runner": "any host for hashes; linux for installed run",
      "red_owner_file": "none",
      "verifier": "self",
      "deps_notes": "must run on the integrated revision, not a feature branch",
      "negative_security_resource": [
        "a manifest hash that does not match the shipped blob is FAIL",
        "an installer that installs oc2 without libopentui.so is FAIL",
        "an unsigned receipt that claims signing is FAIL"
      ],
      "commands": [
        "shasum -a 256 crates/opentui-bridge/native/lib/*/libopentui.so",
        "python3 -c \"import json;json.load(open('crates/opentui-bridge/native/artifacts.json'))\"",
        "python3 tools/lane_gate.py"
      ],
      "evidence": ["artifact hashes match manifest on the integrated commit", "installer vectors pass"],
      "external_blockers": []
    }
  ]
}
```

## 5. Dependency and ordering summary

- Wave 1: `INT-LINUX-1` pre-wires the workspace/dependency seam. Then `LINUX-002` authors a compiling RED against that seam; its frozen test unlocks `LINUX-003`. `LINUX-001` can prepare packaging inputs, but no candidate ref is release truth until integrated.
- Wave 2: `LINUX-004` and `LINUX-006` can run in parallel on distinct owned files; `LINUX-005` depends on the approval/resume child and the installed layout. The integrator owns every shared `lib.rs` edit, one holder at a time.
- Wave 3: `LINUX-007` after the installed journey contract is complete. The
  synthesis assigns reserved integration capacity `INT-LINUX-2` and
  `INT-LINUX-3` to shared wiring and the protected workflow, then
  `INT-LINUX-4` to exact-revision artifact binding. These labels are not
  executable dependency IDs in this domain map. `VER-LINUX-1` and
  `VER-LINUX-2` remain independent.
- Reservation: four integration slots (`INT-LINUX-1..4`) and two verifier slots held throughout; breadth lanes fill the remaining slots, never above 14.

## 6. One-file collision matrix

| File | Lanes | Risk |
|---|---|---|
| `crates/server/src/lib.rs` | LINUX-004/005 (via INT-LINUX-2 only) | serialized; integrator owns the edit |
| `Cargo.toml` (workspace) | LINUX-003 (via INT-LINUX-1 only) | serialized pre-wire before RED |
| `crates/security/Cargo.toml` | LINUX-003 (via INT-LINUX-1 only) | serialized dependency pre-wire before RED |
| `crates/security/tests/phase1_landlock_enforcement.rs` | LINUX-002 (author) then frozen | frozen after RED |
| `tests/e2e/local_application.rs` | LINUX-005 (author) | new file, no collision |
| `crates/opentui-bridge/native/artifacts.json` | LINUX-001 | integration-only |
| `scripts/install-oc2.sh` | LINUX-001 | integration-only; CODEOWNER not required (not in CODEOWNERS) |
| `.github/workflows/release.yml` | LINUX-007 | CODEOWNER-gated, separate commit |
| `tasks/completion/claims.json` | all | only via `tools/completion_claims.py` |

## 7. Verification performed in this lane

```
git status --short --branch            -> clean, plan/PHASE1-VERTICAL-LINUX @ 8a91a7b
git rev-parse HEAD origin/main         -> both 8a91a7b
grep PHASE1-VERTICAL-LINUX claims.json -> empty (no prior row)
ls worklog/PHASE1-VERTICAL-LINUX*      -> no matches (no prior artifact)
uname -a                               -> Darwin arm64 (no Linux kernel)
rustup target list --installed         -> aarch64-apple-darwin only
docker run --rm alpine uname -r        -> daemon not running (no Landlock probe possible)
shasum -a 256 crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so
                                       -> e84ced36... (stale vs manifest 9f074adf... at e4bbb11)
file crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so
                                       -> ELF 64-bit LSB shared object, x86-64
python3 tools/validate_repository.py   -> FAIL backlog-exhaustion 51 (pre-existing)
python3 tools/convergence_gate.py      -> CONVERGENCE BLOCKED total=58 (pre-existing)
```

No test, build, or product command run as evidence; this is a read-only planning lane. No frozen test touched.

Continuation validation:

```
extract and json.loads all fenced JSON -> 3 blocks; 10 unique entries; no
                                         unknown dependencies; no duplicates
git diff --check                      -> PASS
python3 tools/validate_repository.py -> FAIL: 51 inherited backlog-exhaustion
                                         reconciliation errors
python3 tools/convergence_gate.py    -> BLOCKED: 58 inherited ledger findings
```

The guard and convergence failures reproduce the baseline authority blockers;
this proposal does not edit their controller-owned inputs.

## 8. Remaining unknowns and explicit non-claims

- Kernel Landlock enforcement is **not proven** by anything in this repository today; `engage()` returns `Blocked` and this host cannot run a Linux kernel test. The vertical plan requires a Linux runner; no execution happened here.
- The checked-in `main` Linux `.so` and the native manifest disagree (section 1). Whether this is a deliberate stale blob or an unmerged artifact refresh is unknown; `LINUX-001`/`VER-LINUX-2` must resolve it, not assume.
- Windows and macOS verticals are out of scope for this map (sibling `PHASE1-VERTICAL-WINDOWS-GNU-MAP` / `PHASE1-VERTICAL-PRODUCT-MACOS-MAP`).
- No acceptance or product completion is claimed. APP-012 remains open; this artifact proposes lanes only. Candidate refs `origin/lane/CROSS-PLATFORM-RUNNER-CLOSURE-VERIFY @2d04c1c` and `origin/plan/PHASE1-VERTICAL-PACKAGING @e8da066` are not `origin/main` and are not release truth. No signing, notarization, hosted-runner result or release acceptance is claimed.
