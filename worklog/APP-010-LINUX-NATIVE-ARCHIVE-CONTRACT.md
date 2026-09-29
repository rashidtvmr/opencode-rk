# APP-010-LINUX-NATIVE-ARCHIVE-CONTRACT — source candidate scratchpad

## Claim and exact base

- Owner session: `ses_f134383b8ffezMTq9WFzZu6ACR` (assigned route verified as `tokenharbor/gpt-6-luna`; no user allowlist supplied).
- Task: `APP-010-LINUX-NATIVE-ARCHIVE-CONTRACT`, test-author, one owned test path: `crates/cli/tests/linux_native_installer_contract.rs`.
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/LINUX-archive-contract-author`.
- Exact parent candidate: `a16b50daf9ea7ecbe0a9e0088174d9704f8e3299`; parent `1c1a64f677b537319a110063ef6289f8225d41b9`; parent tree `0bfc45ad04f17bb1e4e8b9cbb09964ad473c16a9`.
- Confirmed published `origin/wave/1-staging` points to that exact revision. Base publication receipt: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/LINUX-REGISTRATION-PUBLICATION-RESUME.md`; independent review: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/LINUX-REGISTRATION-INDEPENDENT-RESUME.md`.
- Blueprint corrections input verified SHA-256 `8d74a5e1dc545428c284ea858aa746ea299f9e330cb6e62e0a8fed1583e9b0b2`; original blueprint SHA-256 `0910a0a7d7d59885c110545fc30a3b347d7f145a56bbd0a3c59fa435db8816a3`.
- Existing claim was `blocked` for absent schema. The corrections explicitly authorize a contract-independent subset and defer linkage-dependent cases, so transitioned only this owned claim to `in-progress` via `tools/completion_claims.py`. No foreign claim collision.

## Contract and source evidence

- Published card in `tasks/completion/local.json`, `APP-010-LINUX-NATIVE-ARCHIVE-CONTRACT`: test real Linux installer scripts with archive-byte fixtures; dynamic archives require a safe sibling library, static archives do not; reject wrong target, duplicates, unsafe paths and invalid linkage declaration before destination mutation; preserve outer checksum and non-destructive guarantees. It expressly says outer archive SHA-256 stays primary and no extra per-library digest without integrator threat-model approval.
- `crates/opentui-bridge/build.rs:26-44`: native build selects `libopentui.a` for Linux when present, else `libopentui.so`. This is build-time selection only; it does not define a release manifest.
- `crates/cli/build.rs:20-34`: static `.a` returns without runtime search path; Linux dynamic `.so` uses `$ORIGIN/../lib`. This implies expected installed sibling layout for dynamic loading, but does not specify archive metadata or archive member naming/encoding.
- `crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so` is tracked. `crates/cli/tests/native_launch.rs:261-320` only checks a local artifact path in one native launch test; it is not a release archive schema or installer test contract.
- `scripts/install-oc2.sh:72-98` and `scripts/install-opencode2.sh:72-99`: both validate the complete archive checksum before extraction, unpack to a private `mktemp -d` staging location, require only the executable, then create/copy into install directory. Neither validates archive linkage metadata or installs a library. Existing `install-oc2.sh:82-86, 93-98, 102-141` has legacy-binary refusal, path quoting, copy/chmod and post-copy identity/help checks; stale compatibility script has no corresponding identity gate. These are current behaviors, not the requested new shared contract.
- `LINUX-REGISTRATION-INDEPENDENT-RESUME.md:63-90` confirms conditional linkage and explicitly leaves manifest/linkage schema integrator-owned; per-component digest is not approved. `LINUX-REGISTRATION-PUBLICATION-RESUME.md:50-54` requires compiling real behavioral RED and controller freeze before implementation, with no implementation/acceptance claim from publication.
- The registered `LINUX-RED-FIXTURE-BLUEPRINT.md` was searched under the approved `/private/var/folders/.../T/opencode` root and is not yet available.

## Implemented contract-independent test source

Created `crates/cli/tests/linux_native_installer_contract.rs`. It invokes both real installers with argv, cleared environment and disposable HOME/TMPDIR/minimal PATH; generates bounded ustar+gzip bytes without a new crate dependency; computes outer SHA-256 with host utilities; drains output with a 64 KiB cap per stream; applies a 30-second child deadline; and removes only its unique temp root. The shell executable fixture is packaging-only, not native application, ELF, or loader evidence.

Assertions authored for both scripts: byte-identical install under paths with spaces and non-ASCII; mode 0755; preserve an `opencode` sentinel outside the install directory and a disposable user-data marker; upgrade replaces executable while retaining markers; uninstall removes only executable; existing in-directory `opencode` refusal preserves full destination snapshot; bad outer checksum preserves destination and leaves no TMPDIR residue; argument errors preserve destination; and traversal, absolute path, symlink, FIFO, and duplicate executable archive members are rejected without observed escape or destination mutation.

These are authored but uncompiled/unexecuted. Nothing is reported as executed RED or frozen. Against the registered task base, unsafe-member tests are source-predicted to fail only; no execution evidence exists.

## Deferred linkage-dependent coverage

No linkage metadata/schema is invented. Still deferred: dynamic library member placement and byte identity; dynamic-missing-library rejection (requires deciding structural versus staged-launch predicate); static archive positive; wrong-target detection; shared library replacement/uninstall ownership; and actual ELF loader/rpath proof. Genuine loader proof belongs to independent Ubuntu acceptance. No component digest, `readelf`, `ldd`, native-library mock, or fake-native launch is introduced.

Before authoring, obtain an integrator-approved schema receipt covering at minimum:

1. Manifest member path, serialization, schema/version policy, required/optional state and unknown-field behavior.
2. Canonical target triple/platform representation and dynamic/static declarations.
3. Dynamic library member path/name and installed destination (`bin/../lib` relationship), including whether static archives may contain any native member.
4. Duplicate manifest/member, wrong-target, unsupported linkage, symlink/hardlink/device, absolute path, `..`, and nested member rejection semantics; which cases are rejected by tar extraction vs explicit validation.
5. Expected failure exit/status and exact pre-mutation boundary for both installers.
6. Confirmation that archive SHA-256 is the only integrity authority unless an independent threat-model approval authorizes another digest.

## Safe test harness proposal (not executable source; schema gated)

The harness design is materialized in the test file. It writes deterministic ustar entries in-process and invokes gzip, digest tools, and real scripts only. Child stdout/stderr are concurrently drained into capped buffers. Each sandbox owns a unique temp root and Drop cleanup. The integration test target is Unix-only because both installer scripts are POSIX sh. No network, user HOME, database, or real installation path is used.

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

### Resume from the approved author-ready subset

- The contract-independent subset is authorized by `LINUX-RED-FIXTURE-BLUEPRINT-CORRECTIONS.md:140-165`; its source-only constraints and itemized coverage are incorporated above. This supersedes the earlier original-blocker conclusion for the safe subset only, not the remaining linkage questions.
- Actual authored test functions: `checksum_mismatch_preserves_existing_destination_for_both_installers`; `positive_packaging_install_preserves_existing_data_and_handles_unicode_paths`; `upgrade_replaces_only_packaged_executable_and_preserves_user_markers`; `uninstall_removes_only_installed_binary_and_preserves_other_files`; `legacy_opencode_refusal_preserves_destination_for_both_installers`; `invalid_invocation_and_checksum_fail_without_destination_mutation`; `unsafe_archive_members_are_rejected_without_destination_mutation_or_escape`.
- Fixture architecture: in-process bounded ustar writer covers regular, symlink and FIFO entries; gzip makes real `.tar.gz` archive bytes; outer archive checksum computed from fixture file; scripts invoked as real `sh <script> argv...` children. Every suite case creates its own disposable temp root and captures pre/post snapshots where rejection/non-destructive behavior is contractual.
- Source-level correction: test target is `#![cfg(unix)]`; no platform-specific ELF assertion. Assertions on 64/65/66/73 are only existing argument/checksum/legacy gates documented in the correction appendix, not invented codes for new archive contract rejections. Unsafe-member tests require rejection and no side effects, not a newly assigned status code.
- No link library member is tested, copied, or expected by this subset. Tests do not claim static/dynamic classification, safe sibling library placement, native ELF launch or Ubuntu proof.

- No Cargo, rustc, installer, runtime, listener, browser or VM command executed, per SOURCE ONLY grant. No RED was demonstrated and no freeze hash/command manifest was produced.
- Lightweight source-only observations: `rtk git diff --check` passed before worklog creation; current required base and published ref were verified. `rtk python3 tools/convergence_gate.py` returned `CONVERGENCE BLOCKED total=2` with AUD-017 and AUD-020 notes admitting no acceptance; do not repeat or attribute to this lane.
- Lightweight follow-up checks are limited to diff/check/hash/status. The prior convergence count was two unrelated AUD-017/AUD-020 ledger blockers; the corrections receipt records later staging advancement.
- Test source is uncompiled; lane remains in-progress, not completed. Heavy-slot grant is required for focused compile plus real-script RED against a known script revision and controller freeze.
- Future focused command: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 cargo test -p opencode-rk-cli --test linux_native_installer_contract -- --test-threads=1`. Execute only after heavy-slot grant. Capture command, output/log, source hash and exact script revision before freeze. Linkage-dependent missing-library/static/wrong-target/native-loader cases remain deferred regardless.
