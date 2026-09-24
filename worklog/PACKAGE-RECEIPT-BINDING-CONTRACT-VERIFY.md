# PACKAGE-RECEIPT-BINDING-CONTRACT-VERIFY

## Claim

- Task: `PACKAGE-RECEIPT-BINDING-CONTRACT-VERIFY`
- Session: `ses_f2cd1b633ffethHotQ6IlxnF2i`
- Branch: `plan/PACKAGE-RECEIPT-BINDING`
- Base under review: `d989cb61f5350ee4f04bde480f2f4e786cd532f9`
- Owned file: `worklog/PACKAGE-RECEIPT-BINDING-CONTRACT-VERIFY.md`
- Scope: evidence verification only; no product, workflow, test, controller, or
  release acceptance edits.

## Verdict

**CONDITIONAL / BLOCKED.** The research is source-grounded and correctly proves
that this repository has no release archive producer, that the shell installer
consumes archives, that the source receipt writer exists, and that the runtime
reader is missing. It is not yet precise enough to freeze packaged-binding RED
or authorize implementation integration unchanged.

RED authoring is feasible after the corrections below. Integration remains
blocked until one producer owner and one exact manifest transport/schema are
chosen. No signing or platform proof is inferred.

## Evidence matrix

| Claim in `d989cb6` | Verdict | Source evidence / correction |
|---|---|---|
| No release archive producer exists in this tree; producer is external/unknown | PASS, scoped | `git grep` finds no release pack job/script. `.github/workflows/ci.yml:15-60` is planning/check/test/clippy/fmt only. `scripts/install-oc2.sh:194-291` consumes an archive; `scripts/install-oc2.ps1:44-59` consumes one. `crates/opentui-bridge/native/build_opentui.sh:824-860` extracts/builds a dependency, not a release archive. External producer existence cannot be proven from this tree, so the artifact's `ABSENT / EXTERNAL` wording is valid only with that scope. |
| Source-built receipt exists | PASS | `crates/cli/build.rs:23-48` reads `OC2_BUILD_REVISION`, validates exactly 40 lowercase hex, otherwise uses bounded `git rev-parse`; `:171-214` bounds git output and emits `cargo:rustc-env=GIT_COMMIT`. Explicit invalid input exits at `:50-60`. |
| Missing git/env receipt is honest, never fabricated | PASS | `build.rs:40-48` emits no `GIT_COMMIT` when fallback is absent/invalid. It does not synthesize a revision. |
| Existing installed test reads a receipt | PASS, non-runtime | `crates/cli/tests/installed_default_entrypoint.rs:356-361` reads `OC2_E2E_REVISION` before `option_env!("GIT_COMMIT")`; `:530-571` asserts only nonempty and deterministic behavior. The env override is a test harness substitute, not packaged proof. Artifact correctly labels this gap. |
| Runtime receipt reader is absent | PASS | `crates/cli/src/main.rs:61-64` defines the clap version surface; `:376-400` doctor reports only `CARGO_PKG_VERSION` and capabilities. No source reader of `GIT_COMMIT` exists outside `build.rs` and the installed test. |
| POSIX installer is consumer-only and fail-closed before target write | PASS | `scripts/install-oc2.sh:194-210` checks archive checksum and legacy adjacency before staging; `:215-325` validates names/types and 128 MiB member/aggregate payloads; `:327-364` checks staged identity; target moves begin at `:366-423`. It does not verify a revision receipt. |
| PowerShell installer is weaker and needs parity | PASS | `scripts/install-oc2.ps1:44-51` has only archive hash and legacy refusal; `:54-67` extracts, checks only `oc2.exe`, creates install dir, copies, then runs `--version`. No member allowlist, payload cap, native DLL check, staged identity gate, or receipt verification. |
| Native SBOM/manifest are not release-level metadata | PASS | `crates/opentui-bridge/tests/native_artifact_manifest.rs:38-98,156-203` validates bounded native `artifacts.json`, `NOTICES`, and `sbom.json`; `crates/opentui-bridge/build.rs:17-63` wires target-native library paths. These describe vendored OpenTUI artifacts, not a release archive. |
| Linux/macOS platform set | PASS, path correction required | `install-oc2.sh:73-88,185-191` supports linux/macOS x64/arm64 and computes `native/lib/$PLATFORM/$NATIVE_NAME`. The artifact's matrix says `native/lib/<triple>`; that is inconsistent with the installer, while `crates/opentui-bridge/build.rs:17-20` uses Rust triples internally. Producer/archive layout must choose and state the installer slug path, or installer must change. |
| Windows GNU only; MSVC out | PASS | `crates/opentui-bridge/native/build_opentui.sh:19-31,101-111,136-158,696-701` supports `x86_64-pc-windows-gnu` and rejects MSVC. No MSVC claim is justified. |
| Windows archive contains executable plus native pair | PARTIAL | `build_opentui.sh:24-29` and `crates/opentui-bridge/native/artifacts.json:85-111` prove the GNU DLL/import pair exists. `install-oc2.ps1:57-67` consumes/copies only `oc2.exe`; the matrix's `(+ DLL pair)` is not an installer contract. Producer and PS1 must define exact member paths and install/copy/verify both, or explicitly exclude native Windows packaging. |
| Unsigned Phase 1 | PASS | No signing/notarization implementation is claimed in the artifact or relevant workflow. The contract must remain checksum-based and explicitly non-authenticating. |
| 128 MiB archive bounds | PARTIAL | POSIX bounds are proven at `install-oc2.sh:15-20,263-325`; PS1 has none. The future contract says members retain caps but does not assign an equivalent Windows check or define compressed archive-size handling. RED must cover both installers and distinguish expanded member/aggregate limits from archive-byte limits. |
| Bounded manifest metadata | PARTIAL | `64 KiB` is stated in the artifact and matches native test convention, but no exact JSON grammar, key types, canonicalization, unknown-key policy, or per-field/hash format is specified. |
| Full hash binding | PARTIAL | The source receipt width is exact 40 lowercase hex. The proposed archive/member SHA-256 values are named but not constrained to exact lowercase 64 hex, and no rule says the manifest archive hash must equal the installer-supplied `--checksum`. Add both rules. |
| Missing/mismatch/tamper fail before write | PARTIAL | POSIX transaction boundary is evidenced, but the proposed sidecar location, input flag, trust anchor, and tamper cases are unspecified. A manifest inside the archive makes `archive sha256` self-referential unless excluded from the hashed bytes. State that the manifest is external, or define a canonical hash exclusion. Define whether temp-stage creation is allowed and assert existing target bytes remain unchanged. |
| Manifest revision compared to staged runtime receipt | FAIL as an executable contract | `--version` currently has no revision, and `doctor` has no revision field. “`--version` and/or `doctor`” leaves the reader and parse grammar ambiguous. Select one machine-readable field/format and require exact 40-hex parsing; reject missing, malformed, or duplicate values. |
| Three-owner split (producer, installer, runtime) | PASS as decomposition; blocked as integration plan | Producer owner is explicitly absent; installer files are shared source owners; runtime owner is `crates/cli/src/main.rs`. Exact producer path, manifest transport, and serialized integration owner remain unassigned. |

## Contract corrections required before RED freeze

1. **Manifest transport and trust:** choose an external sidecar path/argument
   (recommended: `--manifest`) and require its `archive_sha256` to equal the
   trusted installer checksum argument. Do not put a self-hashed manifest inside
   the archive unless the hash-exclusion rule is explicit. State unsigned Phase
   1's trust boundary: checksum source is external input, not authenticity.
2. **Exact schema:** define canonical UTF-8 JSON keys and types, e.g. `schema`,
   `revision` (lowercase 40-hex), `archive_sha256` (lowercase 64-hex),
   `members` (bounded object mapping exact archive member names to lowercase
   64-hex), and `sbom` (bounded relative path/reference). Define 64 KiB total
   bound, duplicate/unknown key handling, and deterministic serialization.
3. **Archive layout:** replace `<triple>` in the matrix with the exact names
   consumed by `install-oc2.sh` (`native/lib/linux-x64/...`, etc.), or change
   the installer as a separately owned contract. Define the Windows GNU member
   and installed paths for `opentui.dll` and `libopentui.dll.a`; require PS1
   parity rather than merely noting that parity is needed.
4. **Runtime machine interface:** select `--version` or `doctor --json` as the
   required receipt surface. Specify a stable field and parser, full 40-hex
   validation, and no env fallback. Installer compares the staged executable's
   value to manifest `revision` before any install-target mutation.
5. **Bound and negative matrix:** require POSIX and PowerShell tests for missing,
   malformed, mismatch, manifest tamper, binary/member tamper, >128 MiB member,
   >128 MiB aggregate, unsafe/extra members, and pre-write target preservation.
   Define whether compressed archive bytes have a separate cap.

## RED/source ownership feasibility

- RED can be authored in a new std-only test lane once the five corrections are
  accepted. It can use disposable fixture archives, a fixture runtime receipt,
  both installer scripts, fixed archive/member hashes, and no network.
- Producer owner is not currently viable as a concrete source path. Integration
  must choose either a new `scripts/make-release-archive.sh` plus workflow owner
  or an explicitly recorded external producer contract. A future verifier must
  reject “producer exists” until a checked-in producer or external artifact
  receipt is supplied.
- Installer owner: `scripts/install-oc2.sh` and `scripts/install-oc2.ps1`, with
  PS1 native/member parity explicitly added to the contract.
- Runtime owner: `crates/cli/src/main.rs` plus a small native module if needed;
  exact output field must be integrated before installer verification can be
  proven.
- `crates/cli/tests/installed_default_entrypoint.rs` remains frozen; its
  `OC2_E2E_REVISION` override cannot be reused as packaged proof.

## Validation

- Requested grep: PASS for source inventory; hits are `build.rs`, the frozen
  installed test, installer consumers, native SBOM/artifact metadata, and the
  dependency builder. No release archive producer hit.
- `shasum -a 256 crates/cli/tests/installed_default_entrypoint.rs` ->
  `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317`.
- `git diff --check` -> clean after this worklog is staged in the lane.
- `python3 tools/convergence_gate.py` -> exit 1, `CONVERGENCE BLOCKED`,
  `total=91`; unrelated off-plan/acceptance blockers remain. This verifier does
  not convert the contract into release acceptance.
- No product RED/GREEN applies to this verification lane. No build, network,
  archive extraction, or database command run. Resource scope: text/Git only.

## Decisions and unknowns

- Decision: do not authorize unchanged contract as RED-frozen or integration-ready;
  report conditional blocker rather than infer producer, signing, transport, or
  platform behavior.
- Unknowns: producer ownership; external sidecar delivery/checksum trust; exact
  runtime output format; Windows archive/native installation layout; compressed
  archive cap policy. These are contract blockers, not implementation gaps to
  silently fill.
