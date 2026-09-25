# Phase 1 vertical packaging map

Status: proposed decomposition. This map is planning evidence, not product
implementation or release acceptance.

Task: `PHASE1-VERTICAL-PACKAGING-MAP`
Branch: `plan/PHASE1-VERTICAL-PACKAGING`
Base: `8a91a7b`
Owned artifact: this file

## Boundary and claim discipline

The package contract has three independent product owners and one serialized
integration owner:

1. Runtime receipt: the CLI emits a compile-time source revision.
2. Producer: a deterministic archive and external release manifest are made
   from one source revision and one target payload.
3. Consumers: POSIX and Windows-GNU installers validate the same manifest,
   archive closure, payload hashes, and runtime receipt before mutation.
4. Integration: release matrix, workflow wiring, installed journey, rollback,
   and unsigned Phase 1 evidence join the three owners without changing their
   contracts.

No producer, release archive, release SBOM, runtime receipt, Windows parity, or
release workflow is claimed to exist. Installer GREEN can prove only consumer
behavior. It cannot prove archive production, SBOM provenance, source/runtime
binding, deterministic output, or matrix completeness. Signing, notarization,
publisher authenticity, and hosted-runner results remain external and
unclaimed.

## Source ledger

| Behavior | Evidence | Classification |
| --- | --- | --- |
| Current CLI uses Clap-generated version and currently names the parser `opencode-rk` | `crates/cli/src/main.rs:55-60` at `8a91a7b` | Existing CLI surface; runtime receipt gap |
| Current POSIX installer accepts `--version`, `--archive`, `--checksum`, `--install-dir`, and `--uninstall` only | `scripts/install-oc2.sh:9-30` at `8a91a7b` | Existing legacy consumer |
| Current POSIX installer checks only the archive checksum, extracts with `tar -xzf`, stages `oc2`, then mutates the destination | `scripts/install-oc2.sh:65-98` at `8a91a7b` | Existing partial consumer |
| Current POSIX staged identity checks require `oc2` and reject `opencode-rk`, but do not parse a revision | `scripts/install-oc2.sh:99-141` at `8a91a7b` | Existing identity gate; binding gap |
| Current Windows installer accepts no manifest, uses unbounded `Expand-Archive`, checks only `oc2.exe`, and copies before a receipt check | `scripts/install-oc2.ps1:5-70` at `8a91a7b` | Existing weak consumer; parity gap |
| Compile-time revision accepts exactly 40 lowercase hex, then emits `GIT_COMMIT`; no runtime environment/git fallback is intended | `crates/cli/build.rs:35-47,55-60,171-214` at historical RED commit `ca7a2df` | Existing historical build receipt; runtime owner must expose it |
| Frozen RED uses external `--manifest`, exact POSIX member closure, staged receipt, and sentinel preservation | `ca7a2df:crates/cli/tests/packaged_revision_binding.rs:184-319` | Frozen consumer contract |
| Frozen RED test hash | `d0a800ed7bda8b623aa427604c6ad2c28acdd37633d391079d651246ad0a8858` | Immutable test evidence |
| Installed default entrypoint journey hash | `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317` | Immutable installed-journey evidence |
| Native artifact test hash | `cb7d4cde7c3aed1916814c9aab747b68aa48015518c444e90bd8a58e66bd5c8b` | Immutable native evidence |
| Windows GNU runtime DLL and import archive are a required pair; MSVC is unsupported by the pinned native build | `ca7a2df:crates/opentui-bridge/native/artifacts.json:85-116,171-175`; `ca7a2df:crates/opentui-bridge/native/build_opentui.sh:19-31` | Existing native matrix evidence; not release archive proof |
| Existing native SBOM describes vendored OpenTUI artifacts, not the CLI release archive | `ca7a2df:crates/opentui-bridge/native/sbom.json:1-22`; `ca7a2df:crates/opentui-bridge/tests/native_artifact_manifest.rs:156-204` | Existing dependency evidence; release SBOM gap |
| CI runs planning and Rust check/test/clippy/fmt only; no release packaging job | `.github/workflows/ci.yml:15-60` at `8a91a7b` | Existing workflow; integration gap |
| No release producer exists in the current tree | `git ls-tree -r --name-only 8a91a7b` has no `scripts/make-release-archive.py` | Missing product surface |

The corrected contract is recorded in historical planning evidence at
`a6e1d82:worklog/PACKAGE-RECEIPT-SCHEMA-CORRECTION.md`. Its schema and ownership
decisions are used below as proposed requirements, not as source proof.

## Canonical package contract

### Transport and trust

Each release has two independently delivered files:

- archive: `oc2-<platform>.tar.gz` for POSIX, or `oc2-windows-x64.zip` for
  Windows;
- external sidecar: `oc2-release-manifest.json`.

The sidecar is never an archive member. The installer requires
`--archive FILE --checksum SHA256 --manifest FILE` and performs these checks
before destination mutation:

```text
sha256(archive) == --checksum
manifest.archive_sha256 == --checksum
```

The checksum is an unsigned Phase 1 integrity input, not an authenticity or
publisher-attestation claim. Replacing both archive and checksum remains outside
the threat model until a separately authorized signing lane exists.

Archive and sidecar must both be regular files. Archive size is at most
134,217,728 compressed bytes. Sidecar size is at most 65,536 bytes. Neither
bound is environment-configurable.

### Manifest schema

Schema identifier: `oc2-release-receipt/v1`.

The sidecar is UTF-8 without BOM, one canonical JSON object, no final newline,
no whitespace, no duplicate or unknown keys, and exactly these six top-level
keys:

```json
{
  "archive_sha256": "<lowercase 64-hex>",
  "members": {
    "<exact archive member>": "<lowercase 64-hex>"
  },
  "platform": "<one supported platform>",
  "revision": "<lowercase 40-hex>",
  "sbom": {
    "path": "sbom.json",
    "sha256": "<same hash as members.sbom.json>"
  },
  "schema": "oc2-release-receipt/v1"
}
```

Canonical serialization follows the correction contract's RFC 8785-style
ordering and escaping rule. Arrays are absent. Parser budgets are 128 JSON
tokens, 256 UTF-8 bytes per key/path, 256 UTF-8 bytes per string, and 65,536
total bytes. Invalid UTF-8, BOM, NUL/control characters, trailing bytes, wrong
types, duplicate keys, unknown keys, uppercase hashes, and incomplete member
sets fail closed.

### Exact platform closure

| Platform | Rust target | Format | Exact members in lexical order | Runtime install |
| --- | --- | --- | --- | --- |
| `linux-x64` | `x86_64-unknown-linux-gnu` | tar.gz | `native/lib/linux-x64/libopentui.so`, `oc2`, `sbom.json` | `oc2` in install bin; `.so` in adjacent `lib` |
| `linux-arm64` | `aarch64-unknown-linux-gnu` | tar.gz | `native/lib/linux-arm64/libopentui.so`, `oc2`, `sbom.json` | `oc2` in install bin; `.so` in adjacent `lib` |
| `macos-x64` | `x86_64-apple-darwin` | tar.gz | `native/lib/macos-x64/libopentui.dylib`, `oc2`, `sbom.json` | `oc2` in install bin; dylib in adjacent `lib` |
| `macos-arm64` | `aarch64-apple-darwin` | tar.gz | `native/lib/macos-arm64/libopentui.dylib`, `oc2`, `sbom.json` | `oc2` in install bin; dylib in adjacent `lib` |
| `windows-x64` | `x86_64-pc-windows-gnu` | ZIP | `libopentui.dll.a`, `oc2.exe`, `opentui.dll`, `sbom.json` | `oc2.exe` and DLL beside each other; import archive not copied |

All members are regular files. No directory entry, symlink, hardlink, device,
FIFO, reparse point, PAX/GNU metadata member, duplicate, extra, missing,
absolute, traversal, backslash, empty, control-byte, or overlong path is
accepted. POSIX tar uses deterministic `ustar` metadata, mode `0755` only for
the executable, mode `0644` for payloads, uid/gid `0`, empty owner/group,
mtime `0`, sorted names, and no extended headers. ZIP uses sorted UTF-8 names,
fixed DOS timestamp `1980-01-01 00:00:00`, no extra fields/comments, regular
files, and stored method.

Each member is at most 134,217,728 uncompressed bytes. Aggregate regular
payload is at most 134,217,728 bytes. `sbom.json` is also capped at 65,536
bytes. Bounds are enforced while reading, before a destination path is opened.

### Runtime receipt

`oc2 --version` is the machine interface:

```text
oc2 <package-version> revision=<40-lowercase-hex>\n
```

Exactly one line is accepted. No CR, second line, stderr text, duplicate
`revision=`, uppercase revision, version-only fallback, process-environment
fallback, or runtime git lookup is accepted. The value comes only from the
compile-time `GIT_COMMIT` emitted by the validated build receipt. Missing
compile-time revision is an explicit runtime error.

Installer child execution is bounded to 4,096 combined output bytes, 5 seconds,
8 grammar tokens, and one 40-byte revision. Timeout kills and waits for the
child. Staged and installed receipts must both equal manifest `revision`.

### Transaction and codes

All archive, manifest, member, SBOM, hash, and staged-runtime checks happen in
an isolated temporary stage. Existing binary/native bytes remain unchanged on
every pre-write failure. Destination mutation uses temporary files and atomic
swap with backups; post-install receipt failure restores prior bytes. Temporary,
backup, and stage paths are removed on success, failure, signal, and timeout.

| Code | Meaning |
| ---: | --- |
| 0 | Valid package installed and post-install receipt equals manifest |
| 64 | Usage or unsupported platform/target |
| 65 | Archive, manifest, member, SBOM, size, or hash integrity failure |
| 66 | Archive or manifest absent |
| 73 | Adjacent legacy `opencode` or `opencode.exe` conflict |
| 74 | Runtime identity, destination safety, copy, swap, rollback, or timeout failure |

## Independent vertical chains

The chains below are proposed task objects. `implementation_file` identifies one
product file owned by the lane. A shared path appears only in serialized tasks;
it must never be edited concurrently. `test_owner` is independent from the
implementation owner. `verifier` decides acceptance.

```json
[
  {
    "id": "PKG-RT-001",
    "kind": "implementation",
    "outcome": "Expose the exact compile-time revision receipt through oc2 --version.",
    "schema": "oc2 <package-version> revision=<40-lowercase-hex>\\n",
    "inputs": ["CARGO_PKG_VERSION", "compile-time GIT_COMMIT"],
    "outputs": ["one stdout line", "nonzero explicit error when compile-time revision is absent"],
    "source_evidence": ["crates/cli/src/main.rs:55-60 at 8a91a7b", "ca7a2df:crates/cli/build.rs:35-47,55-60"],
    "implementation_file": "crates/cli/src/main.rs",
    "test_owner": "independent packaging RED verifier",
    "verifier": "Run frozen packaged_revision_binding receipt assertions and a clean binary invocation; reject env/git fallback.",
    "dependencies": ["existing validated compile-time revision receipt"],
    "runner": ["linux-x64", "linux-arm64", "macos-x64", "macos-arm64", "windows-x64"],
    "negative_security_resource": ["uppercase or short revision", "OC2_E2E_REVISION and GIT_COMMIT process env must not affect output", "bounded one-line output"],
    "commands": ["CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 cargo test -p opencode-rk-cli --test packaged_revision_binding -- --test-threads=1"],
    "receipts": ["frozen test sha256 d0a800ed7bda8b623aa427604c6ad2c28acdd37633d391079d651246ad0a8858", "captured stdout/stderr and exact integrated revision"],
    "done_criteria": ["all valid receipts equal manifest revision", "malformed or absent compile-time receipt never becomes a valid version result", "no test edits"]
  },
  {
    "id": "PKG-PROD-002",
    "kind": "implementation",
    "outcome": "Build deterministic archives and the external canonical manifest for every supported target.",
    "schema": "exact platform member sets, lowercase hashes, external oc2-release-manifest.json, unsigned checksum input",
    "inputs": ["one source revision", "target build outputs", "release SBOM bytes", "package version"],
    "outputs": ["oc2-<platform>.tar.gz or oc2-windows-x64.zip", "canonical oc2-release-manifest.json", "archive and member SHA-256 values"],
    "source_evidence": [".github/workflows/ci.yml:15-60 at 8a91a7b has no release producer", "ca7a2df:crates/opentui-bridge/native/artifacts.json:85-116"],
    "implementation_file": "scripts/make-release-archive.py",
    "test_owner": "independent producer determinism verifier",
    "verifier": "Build twice in disposable roots for each target; compare archive bytes, manifest bytes, exact names, hashes, and revision.",
    "dependencies": ["PKG-RT-001", "PKG-SBOM-005", "native GNU pair evidence"],
    "runner": ["one job per linux-x64/linux-arm64/macos-x64/macos-arm64/windows-x64 target", "x86_64-pc-windows-msvc must fail closed"],
    "negative_security_resource": ["missing or wrong-target native payload", "duplicate or extra member", "archive/member/manifest size ceilings", "no shell-string concatenation or ambient git fallback"],
    "commands": ["OC2_BUILD_REVISION=$(git rev-parse HEAD) python3 scripts/make-release-archive.py --platform <platform> --out <disposable-root>", "repeat in a second disposable root and compare with cmp"],
    "receipts": ["two archive SHA-256 values equal", "two manifest SHA-256 values equal", "manifest archive_sha256 equals complete archive bytes", "source revision captured"],
    "done_criteria": ["all five archives are byte-identical on rerun", "manifest is not embedded in archive", "no archive is emitted with a missing or relabeled native artifact", "producer never claims signing or authenticity"]
  },
  {
    "id": "PKG-POSIX-003",
    "kind": "implementation",
    "outcome": "Consume the canonical tar.gz package on Linux and macOS with pre-write validation and atomic rollback.",
    "schema": "--archive FILE --checksum SHA256 --manifest FILE; exact three-member POSIX closure",
    "inputs": ["archive", "trusted external checksum", "canonical sidecar", "install directory"],
    "outputs": ["oc2 plus native library installed in the specified layout", "exit code 0 or defined fail-closed code"],
    "source_evidence": ["scripts/install-oc2.sh:9-30,65-98,99-141 at 8a91a7b", "ca7a2df:crates/cli/tests/packaged_revision_binding.rs:184-319"],
    "implementation_file": "scripts/install-oc2.sh",
    "test_owner": "independent POSIX installer verifier",
    "verifier": "Run frozen T01-T05, then deferred T06-T13, against disposable roots and sentinel binary/native files.",
    "dependencies": ["PKG-RT-001", "PKG-PROD-002", "PKG-SBOM-005"],
    "runner": ["linux-x64", "linux-arm64", "macos-x64", "macos-arm64"],
    "negative_security_resource": ["manifest canonicality and duplicate keys", "tar links/devices/PAX/traversal", "wrong member hashes and native suffix", "128 MiB compressed/member/aggregate bounds", "destination symlink and legacy conflict", "bounded child output and timeout"],
    "commands": ["CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 cargo test -p opencode-rk-cli --test packaged_revision_binding -- --test-threads=1", "sh -n scripts/install-oc2.sh"],
    "receipts": ["frozen test sha256 d0a800ed7bda8b623aa427604c6ad2c28acdd37633d391079d651246ad0a8858", "sentinel hashes before and after every failure", "staged and installed receipt capture"],
    "done_criteria": ["T01-T13 pass without test edits", "all pre-write failures preserve old bytes", "post-install failure restores old bytes", "temporary files are cleaned"],
    "serialization": "Shares no concurrent edit with PKG-BIND-006 or PKG-ROLLBACK-008; integrate sequentially."
  },
  {
    "id": "PKG-WIN-004",
    "kind": "implementation",
    "outcome": "Consume the canonical Windows GNU ZIP package with the same checks and runtime behavior as POSIX.",
    "schema": "windows-x64 exact four-member ZIP: libopentui.dll.a, oc2.exe, opentui.dll, sbom.json",
    "inputs": ["ZIP archive", "trusted external checksum", "canonical sidecar", "install directory"],
    "outputs": ["oc2.exe and opentui.dll beside each other", "verified but not installed libopentui.dll.a", "defined exit code"],
    "source_evidence": ["scripts/install-oc2.ps1:5-70 at 8a91a7b", "ca7a2df:crates/opentui-bridge/native/artifacts.json:85-116,171-175"],
    "implementation_file": "scripts/install-oc2.ps1",
    "test_owner": "independent Windows-GNU verifier",
    "verifier": "Run PowerShell parity tests on windows-x64 GNU fixture runner; compare failure matrix and sentinel preservation with POSIX.",
    "dependencies": ["PKG-RT-001", "PKG-PROD-002", "PKG-SBOM-005"],
    "runner": ["windows-x64 GNU only"],
    "negative_security_resource": ["replace unbounded Expand-Archive", "reject ZIP directory/symlink/reparse/duplicate/traversal entries", "reject missing DLL or import archive", "enforce compressed/member/aggregate/SBOM limits", "reject destination symlink and restore on post-install failure"],
    "commands": ["pwsh -NoProfile -File tests/packaged_revision_binding.ps1", "pwsh -NoProfile -Command '& { [System.Management.Automation.Language.Parser]::ParseFile(\"scripts/install-oc2.ps1\", [ref]$null, [ref]$null) }'"],
    "receipts": ["Windows fixture archive/member hashes", "staged and installed exact receipt", "sentinel hashes and cleanup listing"],
    "done_criteria": ["T02, T06-T13 Windows cases pass", "GNU import pair is required and hashed", "MSVC is rejected rather than relabeled", "no installer mutation precedes staged validation"]
  },
  {
    "id": "PKG-SBOM-005",
    "kind": "serialized-companion",
    "outcome": "Generate a bounded release SBOM tied to the packaged CLI, native payload, source revision, and archive member hashes.",
    "schema": "sbom.json is UTF-8 JSON, SPDX-2.3, <=65536 bytes, exact archive member, manifest sbom.path=sbom.json and matching hash",
    "inputs": ["CLI package version", "source revision", "target native payload metadata", "member checksums", "license/notices data"],
    "outputs": ["release sbom.json bytes", "SPDX package/file records covering CLI and packaged native payload"],
    "source_evidence": ["ca7a2df:crates/opentui-bridge/native/sbom.json:1-22", "ca7a2df:crates/opentui-bridge/tests/native_artifact_manifest.rs:156-204"],
    "implementation_file": "scripts/make-release-archive.py",
    "test_owner": "independent SBOM linkage verifier",
    "verifier": "Reject native-only SBOM substitution; hash archive member bytes; validate SPDX-2.3, size, revision, CLI, native files, and manifest linkage.",
    "dependencies": ["PKG-PROD-002 native payload collection"],
    "runner": ["all five producer target jobs"],
    "negative_security_resource": ["missing CLI or native record", "SBOM >64 KiB", "invalid UTF-8/non-JSON", "manifest path/hash mismatch", "secret or host path leakage"],
    "commands": ["python3 -m json.tool < staged/sbom.json >/dev/null", "shasum -a 256 staged/sbom.json", "compare staged hash with members.sbom.json and manifest.sbom.sha256"],
    "receipts": ["SBOM SHA-256", "SPDX-2.3 parse result", "source revision and target record", "archive member hash equality"],
    "done_criteria": ["SBOM is release-specific, not copied from crates/opentui-bridge/native/sbom.json", "all archive consumers reject SBOM tamper or omission", "producer rerun preserves SBOM bytes"],
    "serialization": "Runs inside PKG-PROD-002; no second worker edits the producer file."
  },
  {
    "id": "PKG-BIND-006",
    "kind": "serialized-integration",
    "outcome": "Prove one source revision survives build injection, manifest revision, staged receipt, and installed receipt.",
    "schema": "manifest.revision == staged oc2 --version revision == installed oc2 --version revision == captured source revision",
    "inputs": ["producer archive and manifest", "compile-time OC2_BUILD_REVISION", "staged and installed executable paths"],
    "outputs": ["binding receipt containing all four equal revisions", "fail-closed code without destination mutation on any mismatch"],
    "source_evidence": ["ca7a2df:crates/cli/build.rs:35-47", "ca7a2df:crates/cli/tests/packaged_revision_binding.rs:202-319", "scripts/install-oc2.sh:99-141 at 8a91a7b"],
    "implementation_file": "scripts/install-oc2.sh",
    "test_owner": "independent frozen RED verifier",
    "verifier": "Inject deterministic wrong archive/member/runtime revisions; inspect bytes, output grammar, exit code, and no environment fallback.",
    "dependencies": ["PKG-RT-001", "PKG-PROD-002", "PKG-POSIX-003", "PKG-WIN-004"],
    "runner": ["one POSIX host and one Windows GNU host, then integrated matrix"],
    "negative_security_resource": ["wrong manifest revision", "runtime mismatch", "uppercase/duplicate/extra-line receipt", "OC2_E2E_REVISION and process GIT_COMMIT ignored", "4,096-byte output and 5-second child bound"],
    "commands": ["CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 cargo test -p opencode-rk-cli --test packaged_revision_binding -- --test-threads=1", "capture git rev-parse HEAD and all receipt lines"],
    "receipts": ["frozen RED hash d0a800ed7bda8b623aa427604c6ad2c28acdd37633d391079d651246ad0a8858", "revision equality table", "sentinel digest table"],
    "done_criteria": ["T02, T03, T05 pass on integrated source", "installed receipt is observed after atomic install, not inferred from staged output", "no fallback or replay of an ambiguous package"],
    "serialization": "Must follow both consumer implementations; cannot be accepted from installer-only GREEN."
  },
  {
    "id": "PKG-E2E-007",
    "kind": "integration-verification",
    "outcome": "Prove a produced package installs from a fresh disposable root and launches the real default entrypoint journey.",
    "schema": "archive -> manifest/hash checks -> installed oc2 -> no-subcommand authenticated daemon/native entrypoint -> receipt",
    "inputs": ["exact integrated revision", "one producer archive and sidecar", "fresh disposable HOME and install root", "fixture provider"],
    "outputs": ["installed journey log", "runtime receipt", "daemon/native frame evidence", "cleanup evidence"],
    "source_evidence": ["crates/cli/tests/installed_default_entrypoint.rs:1-107 at 8a91a7b", "crates/cli/tests/native_launch.rs:212-320 at 8a91a7b"],
    "implementation_file": "crates/cli/tests/installed_default_entrypoint.rs",
    "test_owner": "independent installed-journey verifier; frozen test source is immutable",
    "verifier": "Run the frozen installed test on the exact integrated revision; reject a compile-only or mocked journey.",
    "dependencies": ["PKG-BIND-006", "PKG-POSIX-003 or PKG-WIN-004", "native artifact for selected host", "provider fixture"],
    "runner": ["hosted POSIX packaging runner", "Windows GNU runner when available", "macOS native runner when required by frozen test"],
    "negative_security_resource": ["fresh HOME only", "no manual serve/browser/database setup", "one authenticated daemon", "bounded child/process cleanup", "missing credentials opens setup rather than leaking secrets"],
    "commands": ["CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 240 cargo test -p opencode-rk-cli --test installed_default_entrypoint -- --test-threads=1", "record exact git rev-parse HEAD and test source hash"],
    "receipts": ["installed test hash fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317", "stdout/stderr bounded capture", "process and disposable-root cleanup listing"],
    "done_criteria": ["journey runs from the installed package, not a development binary", "receipt and native runtime evidence are captured", "rerun passes on the same integrated revision"]
  },
  {
    "id": "PKG-ROLLBACK-008",
    "kind": "serialized-integration",
    "outcome": "Prove upgrade, failure rollback, and receipt equality preserve prior installed bytes and native payloads.",
    "schema": "preflight all checks -> temporary copy -> atomic swap -> post-install receipt -> remove backups on success or restore on failure",
    "inputs": ["old binary/native sentinel", "valid upgrade package", "tampered or failing package", "destination paths including symlinks"],
    "outputs": ["successful upgraded files and equal receipt", "unchanged/restored old files on failure", "cleanup receipt"],
    "source_evidence": ["scripts/install-oc2.sh:88-98,142 at 8a91a7b", "ca7a2df:worklog/PACKAGE-RECEIPT-BINDING-RED.md deferred T12-T13"],
    "implementation_file": "scripts/install-oc2.sh",
    "test_owner": "independent transaction verifier",
    "verifier": "Run successful upgrade, injected copy/swap failure, timeout, signal, symlink destination, and legacy conflict cases with byte sentinels.",
    "dependencies": ["PKG-POSIX-003", "PKG-WIN-004", "PKG-BIND-006"],
    "runner": ["POSIX matrix", "Windows GNU parity runner"],
    "negative_security_resource": ["no preflight deletion", "backup restoration after post-install receipt failure", "no symlink destination traversal", "bounded temporary space and cleanup"],
    "commands": ["run deferred T12 and T13 fixtures", "hash target/native before and after each failure", "list stage/tmp/backup paths after exit"],
    "receipts": ["old/new target SHA-256", "rollback code and stderr", "zero retained stage/backup files"],
    "done_criteria": ["T12 and T13 pass without test edits", "old bytes restored after every induced failure", "upgrade leaves user data untouched"]
  },
  {
    "id": "PKG-MATRIX-009",
    "kind": "release-integration",
    "outcome": "Wire the unsigned release matrix and prove target closure, deterministic archives, and explicit unsupported MSVC behavior.",
    "schema": "five target jobs, one exact source revision, one archive/manifest pair per platform, no MSVC relabeling",
    "inputs": ["source revision", "native build evidence", "producer script", "release SBOM", "runner capabilities"],
    "outputs": ["matrix archive set", "hash and size receipts", "explicit unsupported-target receipt for MSVC"],
    "source_evidence": ["ca7a2df:crates/opentui-bridge/native/artifacts.json:85-116,143-175", ".github/workflows/ci.yml:15-60 at 8a91a7b"],
    "implementation_file": ".github/workflows/release.yml",
    "test_owner": "independent release matrix verifier",
    "verifier": "Check workflow target mapping, artifact collection, deterministic rerun, exact sidecar hashes, and no signing claim.",
    "dependencies": ["PKG-PROD-002", "PKG-SBOM-005", "PKG-BIND-006", "PKG-E2E-007"],
    "runner": ["linux-x64", "linux-arm64", "macos-x64", "macos-arm64", "windows-x64 GNU"],
    "negative_security_resource": ["missing hosted capability fails closed", "wrong Rust/native target fails closed", "MSVC absent and explicitly unsupported", "archive/download/local limits remain 128 MiB", "no signing or notarization fabricated"],
    "commands": ["python3 tools/validate_repository.py", "workflow parser/lint check", "run producer twice per matrix target", "record runner/toolchain and artifact hashes"],
    "receipts": ["matrix mapping receipt", "per-target archive and manifest hashes", "source tree/revision hash", "unsupported MSVC receipt"],
    "done_criteria": ["T14 passes for all five targets", "release workflow consumes producer output rather than inventing metadata", "unsigned limitation is visible in release evidence"]
  }
]
```

## Frozen RED boundary

The historical RED commit `ca7a2df` is the test authority. Its focused command
compiled successfully and ran five behavioral tests, all failing because the
current consumer has no `--manifest` path and still enforces its legacy archive
closure. The test source is frozen at hash
`d0a800ed7bda8b623aa427604c6ad2c28acdd37633d391079d651246ad0a8858`.

### T01-T05 in the first vertical slice

| ID | Frozen scenario | Expected observable | Required owner |
| --- | --- | --- | --- |
| T01 | Missing `--manifest` with complete three-member POSIX archive | Exit 64; binary and native sentinels unchanged | POSIX consumer |
| T02 | Canonical good POSIX manifest and archive | Exit 0; exact layout; installed receipt equals manifest revision | Runtime, producer, POSIX consumer |
| T03 | Archive digest, member digest, and manifest/runtime revision mismatches | Exit 65 for archive/member integrity; exit 74 for runtime mismatch; sentinels unchanged | Runtime and POSIX consumer |
| T04 | Extra, duplicate, and traversal members | Exit 65 before destination write | POSIX consumer |
| T05 | Staged and installed exact one-line receipt grammar | Exit 0 only for exact `oc2 <version> revision=<40hex>\\n`; no stderr | Runtime and consumer |

The first slice is not complete when the installer merely returns the expected
code for a fixture. It must demonstrate archive hash, member closure, runtime
binding, post-install observation, and preserved sentinel bytes.

### T06-T14 deferred hardening and producer proof

| ID | Deferred scenario | Why deferred from first RED | Owner/chain |
| --- | --- | --- | --- |
| T06 | Wrong binary/native/DLL/import/SBOM member hashes | Requires complete producer and all member linkage | PKG-PROD-002, PKG-SBOM-005, PKG-POSIX-003, PKG-WIN-004 |
| T07 | Missing, extra, duplicate, absolute, traversal, backslash, overlong names | Requires exact archive validators in both consumers | PKG-POSIX-003, PKG-WIN-004 |
| T08 | Tar links/devices/PAX and ZIP directory/symlink/reparse members | Requires format-native type inspection, not extraction behavior | PKG-POSIX-003, PKG-WIN-004 |
| T09 | Per-member, aggregate, compressed archive, and SBOM size caps | Requires bounded streaming and Windows parity | PKG-POSIX-003, PKG-WIN-004, PKG-SBOM-005 |
| T10 | Missing, malformed, uppercase, duplicate, wrong, extra-line, timeout receipts | Requires real staged child parser and kill/wait ownership | PKG-RT-001, PKG-BIND-006 |
| T11 | Wrong native suffix/path and missing Windows DLL/import pair | Requires five-platform matrix artifacts | PKG-PROD-002, PKG-WIN-004, PKG-MATRIX-009 |
| T12 | Legacy adjacency, destination symlink, injected copy failure, rollback | Requires atomic transaction implementation | PKG-ROLLBACK-008 |
| T13 | Successful upgrade and post-install receipt equality | Requires installed invocation after swap | PKG-BIND-006, PKG-ROLLBACK-008 |
| T14 | Producer rerun byte identity for all five platforms | Cannot be proven by installer GREEN | PKG-PROD-002, PKG-SBOM-005, PKG-MATRIX-009 |

T06-T14 are not waived. They are deferred implementation and verifier work. A
passing T01-T05 consumer lane cannot close the producer, release SBOM, Windows
parity, rollback, matrix, or release parent.

## Wave schedule

Three waves preserve integration and independent verification capacity. Each
wave reserves at least two integration-spine lanes and two independent verifier
lanes; additional slots are bounded by the 8 GiB interactive budget.

### Wave 1: contract spine and producer inputs

| Lane | Work | Exit evidence |
| --- | --- | --- |
| I1 | Freeze schema, exact member table, exit codes, and owner boundaries | This map plus serialized contract review |
| I2 | PKG-RT-001 runtime receipt | Clean compile-time revision receipt; no env/git fallback |
| I3 | PKG-PROD-002 and PKG-SBOM-005 producer fixture seam | Deterministic archive/manifest/SBOM fixtures in disposable roots |
| V1 | Re-run historical RED T01-T05 and verify frozen hash | Five compiling behavioral failures at `ca7a2df` contract |
| V2 | Negative/security review of path, type, hash, size, and unsigned assumptions | Failure matrix with no authenticity overclaim |

No release workflow or acceptance is claimed at this wave. Producer and runtime
may be developed independently after the schema is fixed, but their integrated
receipt is still pending.

### Wave 2: platform consumers and binding

| Lane | Work | Exit evidence |
| --- | --- | --- |
| I1 | PKG-POSIX-003 tar.gz consumer | T01-T05, then POSIX T06-T13 on Linux/macOS fixtures |
| I2 | PKG-WIN-004 ZIP consumer | Windows GNU parity, DLL/import closure, bounded extraction |
| I3 | PKG-BIND-006 staged-to-installed revision binding | Four-way revision equality receipt on integrated tree |
| I4 | PKG-ROLLBACK-008 transaction and upgrade wiring | Sentinel restoration, cleanup, post-install failure evidence |
| V1 | Independent POSIX verifier | Frozen hash, exact command output, no test edits |
| V2 | Independent Windows/security verifier | PowerShell source and hosted-runner receipts; no `Expand-Archive` trust shortcut |

Wave 2 is not release-ready until producer output, not hand-built fixtures,
drives both consumers.

### Wave 3: installed journey, matrix, and unsigned release evidence

| Lane | Work | Exit evidence |
| --- | --- | --- |
| I1 | PKG-E2E-007 installed default journey | Fresh HOME, authenticated singleton daemon, native frame/fixture, receipt, cleanup |
| I2 | PKG-MATRIX-009 release workflow and target matrix | Five target archive/manifest pairs, explicit GNU-only Windows result |
| I3 | Final serialized integration | Exact commit rerun of frozen tests and all package receipts |
| V1 | Release verifier | Independent matrix, archive, SBOM, transaction, and source hashes |
| V2 | Unsigned Phase 1 boundary review | External checksum caveat, signing/notarization/MSVC blockers visible |

Wave 3 may report a candidate package set only after every applicable T01-T14
case passes on the exact integrated revision. It may not report authenticity,
signing, notarization, or hosted-runner success where those receipts are absent.

## Verification and landing receipts

Required focused commands, all bounded and single-slot:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 cargo test -p opencode-rk-cli --test packaged_revision_binding -- --test-threads=1
shasum -a 256 crates/cli/tests/packaged_revision_binding.rs
shasum -a 256 crates/cli/tests/installed_default_entrypoint.rs
shasum -a 256 crates/opentui-bridge/tests/native_artifact_manifest.rs
sh -n scripts/install-oc2.sh
python3 tools/validate_repository.py
python3 tools/convergence_gate.py
git diff --check
```

The historical RED command result is valid evidence only for the RED revision:
compile success, five behavioral failures, no fixture panic, no product edits,
and frozen test hash `d0a800ed7bda8b623aa427604c6ad2c28acdd37633d391079d651246ad0a8858`.
The current convergence gate is independently blocked by pre-existing ledger
off-plan tasks and acceptance notes; that gate failure must remain visible and
must not be bypassed by this planning artifact.

Acceptance requires all of the following on one integrated revision:

- producer and release SBOM exist as real outputs;
- runtime receipt is compile-time bound;
- POSIX and Windows-GNU consumers enforce the same contract;
- T01-T14 applicable cases pass with zero test edits;
- installed E2E passes from a package, not a development binary;
- archive, manifest, SBOM, source, test, and runner receipts are hashable;
- rollback and cleanup evidence exists;
- unsigned limitations and unsupported MSVC remain explicit;
- independent verifier reruns the frozen tests on the exact integrated commit.
