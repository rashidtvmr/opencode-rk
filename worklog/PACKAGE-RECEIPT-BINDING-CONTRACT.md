# PACKAGE-RECEIPT-BINDING-CONTRACT

Status: research complete. No product/test/workflow edits. Separate verifier follows; no implementation/RED in this lane.

## Claim
- Task: `PACKAGE-RECEIPT-BINDING-CONTRACT`
- Session: `ses_f2cd8fdc1ffeFytgFHCfmIlhOK`
- Branch: `plan/PACKAGE-RECEIPT-BINDING`
- Owned file: `worklog/PACKAGE-RECEIPT-BINDING-CONTRACT.md` (this file)
- Base HEAD: `5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b`
- Prior session exhausted steps after analysis with worklog unwritten; this session writes and lands that analysis unchanged in substance.

## Verdict
- Archive producer: ABSENT / EXTERNAL. No archive producer exists in tree. The installer is a consumer, never the producer. Do not mislabel installer ownership.
- Source-built receipt: EXISTS at HEAD (`crates/cli/build.rs`, landed `c4325e4`, integrated `5d66683`).
- Packaged binding (producer/archive/manifest/installer/runtime receipt chain): ABSENT. Future RED + source ownership specified below. No signing claimed; unsigned Phase 1 preserved.

## Producer-absence proof
- Producer search (`git grep -n 'archive|tar|zip|sbom|checksum' -- .github scripts crates`, filtered): zero archive producers.
  - `.github/workflows/ci.yml`: check/test/clippy/fmt only. No release/pack job.
  - `.github/workflows/completion.yml`: plan checks only.
  - `scripts/install-oc2.sh:215,239,270,291`: CONSUMES archives (`tar -tzf` allowlist probe, `tar -tvzf` type probe, `tar -xOf` per-member bounded probe, `tar -xzf` staged extraction).
  - `scripts/install-oc2.ps1`: CONSUMES archives (`Expand-Archive` to temp stage).
  - `crates/opentui-bridge/native/build_opentui.sh`: builds the pinned native lib only (zig fork pin `c01292fd0837bafd07ce458c74416b2b375a41ab`, zig 0.16.0). Not a release-archive producer.
  - No `release.yml`, no `make-release-archive.sh`, no pack script anywhere in tree.
- SBOM/manifest in tree are NATIVE-LIB ONLY, not release-level: `crates/opentui-bridge/native/{artifacts.json,sbom.json,NOTICES}` plus refrozen `crates/opentui-bridge/tests/native_artifact_manifest.rs` (refrozen SHA-256 `cb7d4cde7c3aed1916814c9aab747b68aa48015518c444e90bd8a58e66bd5c8b`). No release manifest/checksum producer exists.

## Source receipt (exists, base of future binding)
- `crates/cli/build.rs:35-48`: `OC2_BUILD_REVISION` env (exactly 40 lowercase hex, else `exit 1` with fixed diagnostic, value never echoed) takes precedence; else bounded `git rev-parse --verify HEAD` (env_clear, stdin null, 128 B output cap, fail-absent = NO `GIT_COMMIT` emitted, never fabricated). Emits `cargo:rustc-env=GIT_COMMIT=<40-hex>`.
- Test reader `crates/cli/tests/installed_default_entrypoint.rs:356-361`: `OC2_E2E_REVISION` env else `option_env!("GIT_COMMIT")`; absent = honest panic (`revision receipt missing: set OC2_E2E_REVISION in packaging or provide compile-time GIT_COMMIT`).
- Runtime surface GAP: no `src/` reader of `GIT_COMMIT` exists. `crates/cli/src/main.rs:379` doctor/`--version` surface `CARGO_PKG_VERSION` (`0.1.0-alpha.1`) only. `GIT_COMMIT` is compiled in but unread at runtime. Only two `GIT_COMMIT` references in tree: `build.rs` (writer) + frozen test (reader).
- Landed evidence: `worklog/APP-010-REVISION-RECEIPT-INTEGRATION.md` (post-push GREEN 5/5 on fresh target, frozen hash unchanged); `worklog/INSTALLED-DEFAULT-CONTRACT-INTEGRATION.md` (4/5 RED without receipt, 5/5 GREEN with `OC2_E2E_REVISION=$(git rev-parse HEAD)`; packaging must inject truthful receipt).

## Installer consumer gaps (consumer, NOT producer)
- `scripts/install-oc2.sh` gates (verified by read): checksum-before-write (sha256sum/shasum, mismatch exit 65); 4-member tar allowlist (`oc2` + `native/lib/<platform>/libopentui.so|dylib`, names relative/safe, duplicates rejected); per-member + aggregate 128 MiB caps (`MAX_ARCHIVE_MEMBER_PAYLOAD=134217728`, `MAX_ARCHIVE_PAYLOAD=134217728`, probe cap 134217729); staged `--version`/`--help` identity (must contain `oc2`, reject `opencode-rk`, fail exit 74, rollback); legacy `opencode` adjacency refusal (exit 73); upgrade rollback transaction.
- GAPS: sh verifies identity but NOT revision receipt (no `OC2_BUILD_REVISION`/`GIT_COMMIT` check anywhere in script); no sidecar manifest consumption (no producer emits one); PS1 strictly weaker (hash gate + legacy refusal only; no allowlist, no size caps, no staged identity gate).
- `install-oc2.sh` vs `install-opencode2.sh`: legacy twin retained (`BIN=opencode2`, no member caps); frozen `packaging_identity.rs` pins no-stale-`opencode2` rule for the oc2 script.

## Receipt/manifest/runtime contract (future, unsigned Phase 1)
- Revision source: producer MUST build with `OC2_BUILD_REVISION=<40 lowercase hex source rev>`; `build.rs` fail-closes on malformed explicit value; absent env + absent git = no receipt, never fabricated.
- Build-time injection: existing `build.rs` path reused unchanged; producer sets the env per platform build.
- Manifest binding: producer MUST ship a bounded sidecar manifest per archive: `revision` (40-hex), archive `sha256`, per-member `sha256`, `sbom.json` reference. Metadata bound 64 KiB (matches native manifest convention); members keep the installer 128 MiB per-member + aggregate caps.
- Installer verification: installer MUST compare manifest `revision` against staged binary runtime receipt BEFORE install; missing/mismatch/tamper aborts pre-write (existing exit-code family 64 usage / 65 integrity / 74 identity preserved). Env-only receipt is never packaged proof.
- Runtime receipt: installed `oc2` MUST expose the full 40-hex receipt (`--version` and/or `doctor` revision field reading compile-time `GIT_COMMIT`). Currently absent; owned by a future runtime-surface lane.
- Tamper/mismatch/fallback: fail closed in all three states; no fallback to env, no silent downgrade to version-string-only identity.

## Platform matrix
| Platform | Installer | Archive expectation | Status |
|---|---|---|---|
| linux-x64 / linux-arm64 | install-oc2.sh | tar.gz: `oc2` + `native/lib/<triple>/libopentui.so` | consumer gates exist; receipt verify absent |
| macos-x64 / macos-arm64 | install-oc2.sh | tar.gz: `oc2` + `native/lib/<triple>/libopentui.dylib` | same as linux |
| windows-x64 (GNU) | install-oc2.ps1 | zip: `oc2.exe` (+ DLL pair per bridge build.rs) | weaker gates; parity work required |
| windows MSVC | none | none | OUT: pinned `build.zig` SUPPORTED_TARGETS lists only `*-windows-gnu`; builder fail-closes (TUI-011 evidence) |
| signing/notarization | none | none | OUT for unsigned Phase 1; never claimed |

## RED / source ownership plan (one RED, three source owners)
- Future RED owner: new lane `crates/cli/tests/packaged_revision_binding.rs` (std-only): archive-to-staged-to-installed receipt equality; missing/mismatch/tampered-manifest RED; deterministic rerun. Independent verifier + implementer, not this lane.
- Source owner (a) producer: new integration-owned CI release job or `scripts/make-release-archive.sh` (matrix ubuntu/macos/windows-gnu runners; sets `OC2_BUILD_REVISION`, emits archive + sha256 + sidecar manifest). Exact runner/owner is the integration blocker if producer stays external: report required, never infer.
- Source owner (b) installer receipt-verify extension: `scripts/install-oc2.sh` + PS1 parity (manifest-vs-staged-binary check pre-write).
- Source owner (c) runtime receipt surface: `crates/cli/src/main.rs` `--version`/`doctor` revision field.
- Determinism/metadata: deterministic tar metadata (mtime/owner/sort); no network/signing in any lane.

## Commands and results (this landing session)
- `shasum -a 256 crates/cli/tests/installed_default_entrypoint.rs` → `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317` (frozen, unchanged).
- `git diff --check` → clean (recorded pre-commit; rerun at landing).
- `git grep -n 'OC2_BUILD_REVISION|GIT_COMMIT|Expand-Archive|tar -x' -- crates scripts` → writer `crates/cli/build.rs`, test reader `installed_default_entrypoint.rs:360`, consumers `install-oc2.sh` tar lines + `install-oc2.ps1` Expand-Archive. No producer hit.
- Resource observation: text/Git only; no builds, no network.

## Blockers and non-goals honored
- No packaging/source/workflow/test edits; no signing claim; no runner fabrication; no release acceptance.
- Producer external/absent reported as ownership blocker for integration (owner (a) above), not resolved by inference.
