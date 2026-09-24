# PACKAGE-RECEIPT-SCHEMA-CORRECTION-VERIFY

## Claim and boundary

- Task: `PACKAGE-RECEIPT-SCHEMA-CORRECTION-VERIFY`
- Session: `ses_verify_7a3b`
- Branch: `plan/PACKAGE-RECEIPT-SCHEMA-CORRECTION`
- Base commit under review: `302f736f077b673c0390a0d88b8a159809508b6e`
- Verifier predecessor commit: `9de9ee7e8bffc7b3cce16d1d2951adeb43735a7b`
- Owned file: `worklog/PACKAGE-RECEIPT-SCHEMA-CORRECTION-VERIFY.md`
- Other permitted file: own ledger row in `tasks/completion/claims.json` only.
- Scope: evidence verification only; no product, test, workflow, installer, signing, or release acceptance edits. No RED, no implementation, no repair.
- Role: release archive provenance verifier.

## Verdict

**REVISE.** Commit `302f736` is a source-grounded research contract document that **defines** a future schema and ownership plan. It does not modify any product source. The verifier `9de9ee` asked whether existing source resolves the blockers. The correction does not resolve them in source; it specifies them for a future RED author. Several critical runtime/installer blockers remain FAIL-level as executable contracts. The schema and ownership plan are ACCEPT-ready for RED authoring, but GREEN cannot be claimed until producer, installer, and runtime are actually wired.

## Blocker-by-blocker source verification

| Verifier gap (9de9ee7) | Source authority (commit 302f736 tree) | Correction document | Resolution status |
|---|---|---|---|
| External sidecar transport/trust in unsigned mode | `install-oc2.sh:57-60` accepts NO `--manifest`; only `--archive/--checksum/--install-dir/--uninstall` | Defines external `--manifest`, `archive_sha256 == --checksum` equality rule | NOT RESOLVED IN SOURCE: only defined for future RED |
| Canonical JSON byte/schema/version/duplicate handling | No JSON manifest parser exists anywhere in `scripts/`, `crates/` | Defines `oc2-release-receipt/v1`, canonical UTF-8, 6 keys, 64 KiB cap, token limits, duplicate/unknown rejection | NOT RESOLVED IN SOURCE: schema fully defined but unimplemented |
| Exact POSIX tar.gz and Windows ZIP member/layout | `install-oc2.sh:191` expects `native/lib/<platform>/libopentui.so`; NO sbom.json member. `install-oc2.ps1:57-67` uses `Expand-Archive`, copies only `oc2.exe` | Defines 3 POSIX members (binary + native + sbom), 4 Windows members (oc2.exe + opentui.dll + libopentui.dll.a + sbom.json) | NOT RESOLVED IN SOURCE: contract **changes** existing POSIX member set by adding sbom.json; Windows installer unchanged |
| Windows GNU DLL/import/runtime requirements | `artifacts.json:85-116` proves DLL+import archive exists for `x86_64-pc-windows-gnu`; MSVC rejected `artifacts.json:171-175`, `build_opentui.sh:19-31` | Same GNU-only evidence | RESOLVED: Windows GNU only confirmed; no MSVC |
| Runtime `--version` grammar/parser | `main.rs:63` uses Clap `version` derive; outputs `oc2 <Cargo version>` with NO revision. `main.rs:376-414` doctor JSON has version/capabilities but no revision | Defines exact grammar `oc2 <version> revision=<40hex>` | NOT RESOLVED IN SOURCE: runtime reader absent; FAIL-level as executable contract per verifier |
| Full 40-lowerhex equality | `build.rs:55-60` validates exactly 40 lowercase hex | Same; defines manifest `revision` = lowercase 40-hex | PARTIALLY RESOLVED: source receipt width exact; runtime `--version` lacks revision entirely |
| Compressed download, manifest, member count/path, 128 MiB extraction caps | `install-oc2.sh:15-20,263-325` has POSIX bounds; `install-oc2.ps1` has NO bounds | Defines 128 MiB compressed archive cap, 128 MiB per-member/aggregate, 64 KiB SBOM | NOT RESOLVED IN SOURCE: PS1 parity absent |
| Traversal/symlink/duplicate/extra/tamper/revision failures before destination writes | `install-oc2.sh:15-203` has POSIX pre-write validation; `install-oc2.ps1` has none | Defines full negative matrix T01-T14 | PARTIALLY RESOLVED: POSIX yes, PS1 no |
| Temp cleanup | `install-oc2.sh:104-153` has cleanup trap/rollback | Same | RESOLVED in POSIX; not in PS1 |
| Exit-code mapping | `install-oc2.sh:64,65,66,69,73,74` exists | Defines 0/64/65/66/73/74 matrix | POSIX RESOLVED; PS1 uses different codes |
| SBOM linkage | No release SBOM exists; `crates/opentui-bridge/native/sbom.json` is vendored OpenTUI only | Defines release `sbom.json` as exact archive member, SPDX 2.3, bounded | NOT RESOLVED IN SOURCE: no release SBOM; native SBOM cannot substitute |
| Concrete producer job/owner matrix | No release archive producer, no release workflow; CI only checks/tests `ci.yml:15-60` | Defines `REL-PACKAGE-PRODUCER`, `make-release-archive.py`, job matrix | DEFINED AS PLAN: producer path is a future owner, not existing source |
| Three-owner DAG | No producer exists; installer is shared source owner; runtime is `main.rs` | Defines producer/sidecar, installer POSIX+PS1, runtime module owner | RESOLVED AS PLAN: decomposition documented, but runtime/installer/sidecar not implemented |
| Deterministic RED fixtures | No `packaged_revision_binding.rs` or PS1 test exists | Defines exact command manifest and T01-T14 cases | DEFINED FOR FUTURE RED AUTHOR: no tests exist yet |

## Required validation commands and results

```sh
git grep -n 'OC2_BUILD_REVISION\|GIT_COMMIT\|Expand-Archive\|tar -x\|artifacts.json\|sbom.json' -- crates scripts .github
shasum -a 256 crates/cli/tests/installed_default_entrypoint.rs
git diff --check
python3 tools/validate_repository.py
```

Results:

- Source inventory grep: no `make-release-archive` or `--manifest` flag exists in `scripts/install-oc2.sh` or `install-oc2.ps1`. `Expand-Archive` only in `install-oc2.ps1:57` (and stale `install-opencode2.ps1:57`). `tar -x` in `install-oc2.sh:270,291` (and stale `install-opencode2.sh:89`). `artifacts.json` and `sbom.json` only in `crates/opentui-bridge/native/`. No release archive producer, no CI release job.
- Frozen installed test SHA-256: `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317` (unchanged).
- `git diff --check`: clean on commit `302f736` tree.
- `python3 tools/validate_repository.py`: FAIL (pre-existing convergence block: `CONVERGENCE BLOCKED` for off-plan/unresolved tasks). This is pre-existing and unrelated to the schema correction lane.

## Root-cause: correction resolves gaps by specification, not implementation

Commit `302f736` adds only `worklog/PACKAGE-RECEIPT-SCHEMA-CORRECTION.md` and a `claims.json` row. It changes zero product source. Every verifier blocker that was FAIL or PARTIAL remains FAIL or PARTIAL in the **source tree**. The correction document correctly:

1. Names exact owners (producer/installer/runtime).
2. Defines exact schema, ABNF, member sets, exit codes.
3. Explicitly states RED cannot be frozen and GREEN cannot be claimed until implementation.

But the **verifier contract** asks: does the **existing source** resolve the blockers? The answer is NO for:

- `--manifest` flag / sidecar parsing (POSIX source has none).
- Manifest schema parser (no parser exists in any script or crate).
- `revision` field in runtime `--version` output (Clap `version` derive only; `main.rs:63`).
- SBOM as release archive member (POSIX installer expects 2 members, no sbom; PS1 expects 1 member).
- Windows PS1 parity (`Expand-Archive`, no member/hash/size validation, no DLL/import/SBOM checks).
- PS1 128 MiB bounds (absent).
- Release SBOM (only vendored OpenTUI `sbom.json` exists, explicitly not a release manifest per correction doc line 36).

## Trust and authenticity

The correction correctly and explicitly states this is unsigned Phase 1. `--checksum` is an external integrity input, not an authenticated publisher signature. No text in the correction calls the digest authenticity, attestation, signature, or notarization. This matches verifier 9de9ee7's `PARTIAL/FAIL` finding. The installer remains a consumer only.

## Frozen test integrity

- `crates/cli/tests/installed_default_entrypoint.rs` SHA-256: `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317` (confirmed unchanged).
- No frozen tests were touched, renamed, moved, skipped, or weakened.
- `packaging_identity.rs` tests reference `install-oc2.sh` but assert alias behavior, not manifest/revision binding.

## Decision: REVISE (not ACCEPT)

The correction document is ACCEPTABLE as a **specifications contract** for a future RED author. It resolves the verifier's *information gaps* (ownership, schema, layout, failure matrix). However, it does NOT resolve the **executable source blockers**:

- The runtime `--version` has no revision field (FAIL as executable contract).
- The installer scripts have no `--manifest` input or JSON parser (FAIL as executable contract).
- The Windows PS1 installer has no member/hash/SBOM/bounds validation (FAIL as executable contract).
- No release SBOM or archive producer exists (FAIL as executable contract).

The correction honestly states "GREEN cannot be claimed until producer, installer, and runtime are wired." This verification confirms that state. The document is ready for separate RED authoring because every prior blocker now has an exact schema, owner, failure mode, and RED boundary. But the **commit itself** does not resolve the blockers in source -- it defines them for implementation.

## Remaining gaps (unresolved by correction commit)

1. **Runtime `--version` grammar**: `main.rs:63` Clap `version` derive must be replaced with custom handler emitting `oc2 <version> revision=<40hex>`. Owned by `REL-PACKAGE-RUNTIME-RECEIPT`. NOT implemented.
2. **`--manifest` sidecar flag**: `install-oc2.sh:58` usage and arg parser must add `--manifest`. NOT implemented.
3. **JSON manifest parser**: No canonical JSON parser exists anywhere in scripts or crates. NOT implemented.
4. **SBOM archive member**: POSIX installer `install-oc2.sh:212-235` member allowlist must add `sbom.json`; current `MAX_ARCHIVE_MEMBERS=4` and allowlist checks `oc2` + native only. NOT implemented.
5. **Windows PS1 parity**: `install-oc2.ps1:57` `Expand-Archive` must be replaced with bounded `ZipArchive` validation. NOT implemented.
6. **Producer**: No `scripts/make-release-archive.py` exists. NOT implemented.

## Resource observations

- No build, test, network, or archive-extraction commands were run.
- Text/Git only; no child processes spawned.
- Memory/CPU/disk negligible: grep, read, sha256sum only.

## Conclusion

Commit `302f736f077b673c0390a0d88b8a159809508b6e` resolves **all prior verifier informational gaps** by defining exact schema, ownership, failure matrix, and RED boundary. It is ready for **separate RED authoring** as evidenced by the explicit `T01-T14` fixture matrix and command manifest in the correction document. However, it does **not resolve every executable source blocker** -- those remain pending implementation by the defined owners (producer, installer, runtime). Status: **REVISE** for implementation gate; **ACCEPT** for RED-authoring gate.
