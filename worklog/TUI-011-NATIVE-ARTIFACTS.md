# TUI-011 native artifacts RED — frozen test

Status: RED frozen; blocked pending manifest/artifact/sidecar provision.

Frozen test SHA-256: `efb642364b6a4dc559fff019d21c14f778c170f34f96bf5e7183b4aeb536aa40`

## Claim
- Task: TUI-011
- Session: `ses_f30a77c92ffeDl6rNfFwafsKQh`
- Base commit: `ff4a409296d1ddb347cf8c1697d85e785bcc1962`
- Scratchpad owner file: `worklog/TUI-011-NATIVE-ARTIFACTS.md`
- Sole owned test: `crates/opentui-bridge/tests/native_artifact_manifest.rs`

## Source evidence
- `crates/opentui-bridge/build.rs:17-55`: link gate; for macOS triple expects
  `native/lib/<triple>/libopentui.a` (static) or `libopentui.dylib` (dylib).
- `crates/opentui-bridge/build.rs:52-55`: expected artifact names per OS.
- `docs/architecture/COMPLETION_NATIVE_TUI.md:14-20`: OpenTUI fork pin
  `c01292fd0837bafd07ce458c74416b2b375a41ab`, native tree = packages/native,
  build defined in `packages/native/build.zig`.
- `worklog/TUI-011-PROD-AUDIT.md:76-116`: current artifact matrix shows
  aarch64-apple-darwin ABSENT from repo; no SBOM, no checksum manifest, no
  license sidecars.
- `tasks/completion/tui.json:14`: TUI-011 requires reproducible native
  distribution, SBOM/license/checksum artifacts, exact source/toolchain/input
  identification.

## Host evidence (arm64 macOS)
- `uname -m` = arm64
- Target triple: `aarch64-apple-darwin`
- `cargo/rustc` available: rustc 1.98.1

## Observable contract (RED scope)
A bounded `crates/opentui-bridge/native/artifacts.json` (<=64KiB) records:
- `fork`: trusted repo origin and exact commit `c01292fd0837bafd07ce458c74416b2b375a41ab`
- `zig_version`: pinned Zig version string
- `target`: `aarch64-apple-darwin`
- `artifact`: exact relative path under `crates/opentui-bridge/native/` (e.g.
  `lib/aarch64-apple-darwin/libopentui.dylib`) and lowercase 64-hex SHA-256

The artifact file exists at the path declared in the manifest and its bytes
are a valid Mach-O 64-bit shared library or static archive for arm64 macOS
(minimal deterministic header parsing; no shell/network). Required license and
SBOM sidecars exist alongside the manifest and are bounded in size.

## RED test
`crates/opentui-bridge/tests/native_artifact_manifest.rs` (std only, no dep
changes). Compiles and fails because:
- `artifacts.json` does not exist (manifest-missing failure)
- no `aarch64-apple-darwin` artifact exists (artifact-missing failure)
- no license/SBOM sidecars exist (sidecar-missing failure)

Test asserts absence with actionable error messages.

## Run
```
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-opentui-bridge --test native_artifact_manifest -- --test-threads=1
```

## RED evidence
- Compiles: OK (no errors after rustfmt)
- Run result: 3 tests, 0 passed, 3 failed (expected RED)
- Failures:
  - test_manifest_exists_bounded_and_records_contract: manifest not found at native/artifacts.json
  - test_artifact_exists_and_is_macho_arm64: manifest not found (cascading)
  - test_license_and_sbom_sidecars_exist_and_bounded: NOTICES not found at native/NOTICE
- Frozen test SHA-256: `efb642364b6a4dc559fff019d21c14f778c170f34f96bf5e7183b4aeb536aa40`

## Remaining
- Mark TUI-011 `blocked` with RED evidence via completion_claims.
- Commit only: test file, this scratchpad, claims.json row; push
  `lane/TUI-011-native-artifacts` (never force).
- Fetch and rebase onto origin/lane/PHASE1-product-spine-20260923, re-run RED, push.
