# TUI-011-FROZEN-CONTRACT-REVIEW

Status: blocked; frozen-test contract review required. No product, test, policy,
plan, verifier, manifest, or artifact edits.

## Claim and revision

- Task: `TUI-011-FROZEN-CONTRACT-REVIEW`
- Session: `ses_f2ec822f1ffejtupLfO5SURFAW`
- Branch: `lane/TUI-011-FROZEN-REVIEW`
- Candidate revision audited: `a83a56327dfd15e96dd2e9c5faf6bf231548746f`
- Frozen test: `crates/opentui-bridge/tests/native_artifact_manifest.rs`
- Frozen test SHA-256: `efb642364b6a4dc559fff019d21c14f778c170f34f96bf5e7183b4aeb536aa40`

## Source evidence

- `crates/opentui-bridge/tests/native_artifact_manifest.rs:10` defines one
  `MAX_BYTES` value of `64 * 1024`.
- `crates/opentui-bridge/tests/native_artifact_manifest.rs:88-96` validates the
  manifest `sha256`; line 93 requires every character to be both an ASCII hex
  digit and an ASCII lowercase character. ASCII digits are hex digits but are
  not lowercase characters.
- `crates/opentui-bridge/tests/native_artifact_manifest.rs:115-125` applies the
  same 64 KiB value to the native binary. This is a metadata/sidecar bound used
  against a multi-megabyte Mach-O artifact.
- `crates/opentui-bridge/tests/native_artifact_manifest.rs:149-196` applies the
  64 KiB bound to `NOTICES` and `sbom.json`; this sidecar use is internally
  coherent.
- `crates/opentui-bridge/native/artifacts.json:3-14` records the pinned fork,
  target, dylib path, SHA-256, and declared size. The arm64 dylib declaration
  is `6863648` bytes with SHA-256
  `798f30dd7f4fbe36d52c8834652ed7bcd7f20dfd2a1203d09cc24880eeb13a91`.
- `crates/opentui-bridge/build.rs:26-44` accepts the macOS static or dynamic
  native artifact and does not impose a 64 KiB artifact limit.
- `crates/opentui-bridge/native/build_opentui.sh:71-75` defines the production
  artifact bound as `MAX_ARTIFACT_BYTES=134217728` (128 MiB), distinct from
  metadata bounds.

## Reproduction

Executed at the exact candidate revision with one Cargo build job, one Rust test
thread, and a 180-second Python subprocess timeout:

`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-opentui-bridge --test native_artifact_manifest -- --test-threads=1`

Result: exit `101`; `running 3 tests`; `1 passed, 2 failed`.

1. `test_artifact_exists_and_is_macho_arm64` failed at frozen test line 122:
   `TUI-011 RED: artifact is 6863648 bytes, exceeds 64 KiB`.
   This is an internally incorrect assertion, not an implementation failure:
   the manifest declares the same size at `artifacts.json:14`, and the file is
   a valid `Mach-O 64-bit dynamically linked shared library arm64`.
2. `test_manifest_exists_bounded_and_records_contract` failed at frozen test
   line 95:
   `TUI-011 RED: sha256 must be lowercase 64-hex, found: 798f30dd7f4fbe36d52c8834652ed7bcd7f20dfd2a1203d09cc24880eeb13a91`.
   This is an internally incorrect assertion: the value is exactly 64 chars,
   contains only `0-9a-f`, and is the verified artifact digest. No production
   behavior is implicated.
3. `test_license_and_sbom_sidecars_exist_and_bounded` passed. It found nonempty
   bounded sidecars and the pinned commit.

## Artifact and sidecar evidence

Independent SHA-256 and byte checks matched every manifest entry:

| Target / file | Bytes | SHA-256 |
|---|---:|---|
| `aarch64-apple-darwin/libopentui.dylib` | 6863648 | `798f30dd7f4fbe36d52c8834652ed7bcd7f20dfd2a1203d09cc24880eeb13a91` |
| `aarch64-apple-darwin/libopentui_static.a` | 7344888 | `a3c80e598f3808c9c600ec90203035065fa81fb87f5beb6086ab0f5383842956` |
| `aarch64-unknown-linux-gnu/libopentui.so` | 26598960 | `e85a45710e9e181b3eb7cca877a1d9022f2210bfa1e06b7c159e734506da3b89` |
| `x86_64-apple-darwin/libopentui.dylib` | 7190318 | `a7756f27becf5e0b9f25e553ad0141b76de2274c57fee6edb156c9b0ca5c9ef5` |
| `x86_64-unknown-linux-gnu/libopentui.so` | 26627832 | `9f074adf1e3c67bb027433d44da05b9e285a37ae58cf0b2ed76504304450c79e` |
| `x86_64-pc-windows-gnu/opentui.dll` | 6800384 | `0be2efd8a77330ba23b0f5eeb094a548f5fee4b7ad7b4c6f83e0e786dadd6b15` |
| `x86_64-pc-windows-gnu/libopentui.dll.a` | 101524 | `1dce093459b94a4d6bd0538c4fdd1681f50534930f3d59918b765bc87a982019` |

Sidecar bounds (`64 KiB`):

- `artifacts.json`: `6624` bytes; SHA-256
  `4820e519c65a44af9c814f79224d3cf47103df3eb39f2afdffc3fdff212a7711`.
- `sbom.json`: `35942` bytes; SHA-256
  `da5a67f38283e49abebac9c572af9cdff7f6363a1258066a5634e4e557052523`.
- `NOTICES`: `18635` bytes; SHA-256
  `5932e51716a4771b9d3b23eaab454be9a67ccfce83f4c9b2bac7ad983ca8a1bf`.

All sidecars are nonempty and below `65536` bytes. `artifacts.json` and
`sbom.json` parse as JSON. The builder script SHA-256 is
`8d1861cb688ed7f09b9b167e2e412fc88c154dfd6c78d6c551a80f10a67b6536`.

## Controller-facing contract-review proposal

Do not edit the frozen test in this lane. Authorize a new test revision only if
the controller accepts these minimal semantic corrections, then freeze and hash
that revision again:

1. Keep the manifest digest requirement at exactly 64 characters, with each
   character restricted to the lowercase hexadecimal alphabet `0-9` and `a-f`.
   The predicate must not reject decimal digits as “not lowercase”.
2. Keep the 64 KiB bound for bounded metadata: `artifacts.json`, `NOTICES`, and
   `sbom.json`. Remove that bound from the native artifact byte length.
3. Validate the native artifact using its declared manifest `size` and SHA-256,
   then apply a separately named artifact ceiling consistent with the production
   builder (`128 MiB`), followed by the existing Mach-O 64-bit arm64 checks.
   This preserves resource bounding without treating a binary as metadata.
4. Preserve the frozen test's no-network, std-only behavior and retain the
   sidecar nonempty/UTF-8/commit checks. A corrected test must receive a new
   SHA-256 and verifier freeze record; the current hash remains immutable.

## Production verification still required

After authorized test correction, rerun the corrected test on the exact
integrated revision, independently recompute every manifest hash and size, and
run the pinned builder `--verify-only` checks for each target and the Windows
GNU pair. Re-run native bridge, native parity, PTY, static reproducibility,
release packaging/install, clean-target dependency closure, SBOM/license
relationship, and supported-platform runtime checks. Resolve or explicitly
record the existing MSVC unsupported/header limitation and signing/notarization
authority gap. The frozen-test correction alone cannot establish TUI-011 or
parent convergence acceptance.

## Decision

Blocked pending controller contract review. No acceptance claimed. No frozen test
or production source changed.
