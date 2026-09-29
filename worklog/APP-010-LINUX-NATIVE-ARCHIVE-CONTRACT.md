# APP-010-LINUX-NATIVE-ARCHIVE-CONTRACT — blocked test-author scratchpad

## Claim and exact base

- Owner session: `ses_f134383b8ffezMTq9WFzZu6ACR` (assigned route verified as `tokenharbor/gpt-6-luna`; no user allowlist supplied).
- Task: `APP-010-LINUX-NATIVE-ARCHIVE-CONTRACT`, test-author, one owned test path: `crates/cli/tests/linux_native_installer_contract.rs`.
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/LINUX-archive-contract-author`.
- Exact base/revision: `1c1a64f677b537319a110063ef6289f8225d41b9`; tree `0bfc45ad04f17bb1e4e8b9cbb09964ad473c16a9`.
- Confirmed published `origin/wave/1-staging` points to that exact revision. Base publication receipt: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/LINUX-REGISTRATION-PUBLICATION-RESUME.md`; independent review: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/LINUX-REGISTRATION-INDEPENDENT-RESUME.md`.
- Claim taken through `tools/completion_claims.py`; subsequent exact-schema blocker was set via `cc.update(..., 'blocked', ...)`. No foreign claim collision.

## Contract and source evidence

- Published card in `tasks/completion/local.json`, `APP-010-LINUX-NATIVE-ARCHIVE-CONTRACT`: test real Linux installer scripts with archive-byte fixtures; dynamic archives require a safe sibling library, static archives do not; reject wrong target, duplicates, unsafe paths and invalid linkage declaration before destination mutation; preserve outer checksum and non-destructive guarantees. It expressly says outer archive SHA-256 stays primary and no extra per-library digest without integrator threat-model approval.
- `crates/opentui-bridge/build.rs:26-44`: native build selects `libopentui.a` for Linux when present, else `libopentui.so`. This is build-time selection only; it does not define a release manifest.
- `crates/cli/build.rs:20-34`: static `.a` returns without runtime search path; Linux dynamic `.so` uses `$ORIGIN/../lib`. This implies expected installed sibling layout for dynamic loading, but does not specify archive metadata or archive member naming/encoding.
- `crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so` is tracked. `crates/cli/tests/native_launch.rs:261-320` only checks a local artifact path in one native launch test; it is not a release archive schema or installer test contract.
- `scripts/install-oc2.sh:72-98` and `scripts/install-opencode2.sh:72-99`: both validate the complete archive checksum before extraction, unpack to a private `mktemp -d` staging location, require only the executable, then create/copy into install directory. Neither validates archive linkage metadata or installs a library. Existing `install-oc2.sh:82-86, 93-98, 102-141` has legacy-binary refusal, path quoting, copy/chmod and post-copy identity/help checks; stale compatibility script has no corresponding identity gate. These are current behaviors, not the requested new shared contract.
- `LINUX-REGISTRATION-INDEPENDENT-RESUME.md:63-90` confirms conditional linkage and explicitly leaves manifest/linkage schema integrator-owned; per-component digest is not approved. `LINUX-REGISTRATION-PUBLICATION-RESUME.md:50-54` requires compiling real behavioral RED and controller freeze before implementation, with no implementation/acceptance claim from publication.
- The registered `LINUX-RED-FIXTURE-BLUEPRINT.md` was searched under the approved `/private/var/folders/.../T/opencode` root and is not yet available.

## Blocker / safe boundary

The contract calls for tests constructing valid and invalid linkage declarations, but the published source has no archive manifest file path, encoding/schema version, target field, linkage enum/literals, library member naming/location, duplicate-definition semantics, or safe-member grammar. The current scripts consume flat tar.gz files containing `oc2`; the existing release source has no producer/validator contract. A test that invents such a declaration would become authority for an unapproved packaging schema and could freeze behavior contrary to the integrator's approved design. Therefore no owned Rust test file was created, and no test was run or frozen.

Before authoring, obtain an integrator-approved schema receipt covering at minimum:

1. Manifest member path, serialization, schema/version policy, required/optional state and unknown-field behavior.
2. Canonical target triple/platform representation and dynamic/static declarations.
3. Dynamic library member path/name and installed destination (`bin/../lib` relationship), including whether static archives may contain any native member.
4. Duplicate manifest/member, wrong-target, unsupported linkage, symlink/hardlink/device, absolute path, `..`, and nested member rejection semantics; which cases are rejected by tar extraction vs explicit validation.
5. Expected failure exit/status and exact pre-mutation boundary for both installers.
6. Confirmation that archive SHA-256 is the only integrity authority unless an independent threat-model approval authorizes another digest.

## Safe test harness proposal (not executable source; schema gated)

Once the above is approved, implement one std-only Rust integration target that drives **both actual scripts** via `Command` using argv (never a shell command string), with per-test unique directories under `std::env::temp_dir`, disposable `HOME`, isolated `OC2_INSTALL_DIR`/`OPENCODE2_INSTALL_DIR`, cleared environment plus only an explicit minimal `PATH`/`HOME`, and no network. Build archive bytes locally using deterministic fixtures and compute the checksum from the archive file as production scripts require. Use executable fixture bytes that are actual tiny shell scripts, not mock success of the installer; dynamic execution fixture should use a native sibling library only if required to exercise that code path, while packaging-only tests assert exact installed library bytes and do not imply ELF runtime proof. On Unix, use `tar` as an argv process to create archive fixtures and an RAII `Drop` guard for temp cleanup; require bounded child completion with `try_wait` polling/kill+wait under a fixed deadline and retain at most 64 KiB combined stdout/stderr. Avoid invoking unrelated binaries through global PATH where possible; provide only system utilities scripts need (`uname`, `tar`, `sha256sum`/`shasum`, `cut`, `mktemp`, `mkdir`, `cp`, `chmod`, `rm`). Verify no destination mutation by snapshotting the isolated install tree and sentinel bytes before rejection.

Concrete case map after schema approval:

- Dynamic positive, both installers: fixture archive with schema-approved dynamic declaration and sibling library; install under paths containing spaces and non-ASCII; assert executable and library bytes/modes at specified locations, legacy `opencode` sentinel and unrelated user marker byte-identical, and no stage residue.
- Dynamic missing library: valid dynamic declaration but no library member; assert rejection and entire destination snapshot unchanged (expected RED against current script, which currently only checks executable).
- Static positive: schema-approved static declaration, no sibling `.so`; assert successful install. Do not assert universally-required `.so` behavior.
- Before-mutation invalid matrix: wrong target, duplicate metadata/member, unsafe/traversal/link member, invalid declaration, missing manifest as defined by schema; assert nonzero contract-specific exit and no created/replaced destination files.
- Existing guarantees: altered outer checksum rejected before destination mutation; pre-existing `opencode` causes refusal and remains byte-identical; upgrade marker preserved; paths with spaces/non-ASCII; uninstall removes only binary/library owned by installer as agreed, retains marker/user data; safe cleanup on every failure.
- Run script cases by script path and argv; no installer is run in this authoring session because the assigned verification budget is SOURCE ONLY.

## Observable resource/security bounds

- Fixture and installed bytes generated solely inside one unique disposable temp root; do not read or alter user HOME/database. No secrets or network. Only the test-created inputs are read.
- Subprocess lifetime deadline and output cap above; archive fixtures small (e.g. below 64 KiB each), constant test matrix and bounded file counts (e.g. <=32 members). Stage/install paths are under temp root; no host `/usr/local`, real `~/.local`, or repository release destination is used.
- Real script invocation with explicit environment and positional arguments avoids shell interpolation/inherited credentials. `tar` path security assertions require schema/installer contract; do not presume a generic tar hardening policy without integration approval.

## Tests, guards, and status

- No Cargo, rustc, installer, runtime, listener, browser or VM command executed, per SOURCE ONLY grant.
- Lightweight source-only observations: `rtk git diff --check` passed before worklog creation; current required base and published ref were verified. `rtk python3 tools/convergence_gate.py` returned `CONVERGENCE BLOCKED total=2` with AUD-017 and AUD-020 notes admitting no acceptance; do not repeat or attribute to this lane.
- No test SHA/frozen RED exists. The prerequisite for resumption is approved archive schema/fixture blueprint and a later heavy-slot grant for independent compile+real-script RED, then controller freeze. This is not completed and makes no acceptance claim.
- Remaining tasks after schema: author only the registered test path, compile and independently run the real scripts in a disposable fixture, capture expected behavioral RED (including missing dynamic library), hash source and command manifest, submit to controller for freeze. Only after that checkpoint may script implementation lanes begin.
