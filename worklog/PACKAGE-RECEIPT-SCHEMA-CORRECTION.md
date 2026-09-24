# PACKAGE-RECEIPT-SCHEMA-CORRECTION

Status: correction research complete. No product, test, workflow, producer, or
installer edits. This document freezes one receipt-binding contract for a fresh
RED author and independent verifier. Phase 1 remains unsigned.

## Claim and boundary

- Task: `PACKAGE-RECEIPT-SCHEMA-CORRECTION`
- Session: `ses_f2cc572c3ffejuH27YPGJY5dFV`
- Branch: `plan/PACKAGE-RECEIPT-SCHEMA-CORRECTION`
- Base: `9de9ee7e8bffc7b3cce16d1d2951adeb43735a7b`
- Owned file: `worklog/PACKAGE-RECEIPT-SCHEMA-CORRECTION.md`
- Other edits: own row in `tasks/completion/claims.json` only.
- Product producer is absent at current HEAD. Future producer paths below are
  ownership decisions, not claims that those paths already exist.
- No signing, notarization, authenticity, hosted-runner result, archive bytes,
  or release acceptance is claimed.

## Source evidence and current-state classification

Authoritative evidence is checked-in source and verifier findings. Earlier
contract prose, model output, issue text, and task artifacts are untrusted until
reconciled here.

| Current behavior | Evidence | Classification |
|---|---|---|---|
| Explicit `OC2_BUILD_REVISION` accepts exactly 40 lowercase hex; fallback is bounded `git rev-parse`; absent fallback emits no receipt | `crates/cli/build.rs:35-47,50-60,171-214` | Existing source receipt; preserve |
| Frozen installed test reads `OC2_E2E_REVISION` before compile-time `GIT_COMMIT` | `crates/cli/tests/installed_default_entrypoint.rs:356-361` | Test-only override; never package proof |
| CLI version surface is Clap-generated; doctor JSON has version/capabilities but no revision | `crates/cli/src/main.rs:60-65,348-413` | Runtime gap; runtime owner must add exact receipt |
| POSIX consumer verifies checksum, names/types, 128 MiB expanded limits, staged identity, then writes | `scripts/install-oc2.sh:15-20,194-235,237-325,327-426` | Existing partial consumer; extend |
| POSIX consumer currently expects `native/lib/<platform-slug>/...` | `scripts/install-oc2.sh:183-191` | Canonical archive path below |
| PowerShell consumer verifies checksum and `oc2.exe`, but uses unbounded `Expand-Archive` and has no member, DLL, receipt, or size gate | `scripts/install-oc2.ps1:44-70` | Weak consumer; parity is mandatory |
| OpenTUI Rust build gate uses native source triple paths and requires Windows DLL plus GNU import library; MSVC wording is conditional in code | `crates/opentui-bridge/build.rs:17-63` | Native build evidence |
| Pinned native matrix has `opentui.dll` plus `libopentui.dll.a` for `x86_64-pc-windows-gnu`; explicitly rejects MSVC | `crates/opentui-bridge/native/artifacts.json:85-116,171-175`; `crates/opentui-bridge/native/build_opentui.sh:19-31,101-111` | Windows GNU only |
| Existing native `artifacts.json`, `sbom.json`, and `NOTICES` describe vendored OpenTUI, not a release archive | `crates/opentui-bridge/tests/native_artifact_manifest.rs:38-98,156-204` | Do not relabel as release manifest |
| No release archive producer or release job is present | verifier `worklog/PACKAGE-RECEIPT-BINDING-CONTRACT-VERIFY.md:27-29,74-83` | Future producer owner required |

## Canonical receipt schema

### Transport and trust

The release producer emits two independent files per archive:

1. archive, named by platform below;
2. external sidecar `oc2-release-manifest.json` delivered beside it.

The installer requires `--archive FILE --checksum SHA256 --manifest FILE`. The
manifest is never an archive member. This avoids a self-hash cycle. The
installer first checks the archive byte size, computes its SHA-256, and requires
both equality conditions before extraction:

```
actual_archive_sha256 == --checksum
manifest.archive_sha256 == --checksum
```

`--checksum` is the expected digest supplied by a trusted release channel. In
unsigned Phase 1 it is an external integrity input, not an authenticated
publisher signature. A user who can replace both archive and checksum can
replace the package. No text in this contract calls the digest authenticity,
attestation, signature, or notarization.

The sidecar is read before destination mutation. It is a regular UTF-8 file,
not a symlink, at most 65,536 bytes. The archive is a regular file, not a
symlink, at most 134,217,728 compressed bytes. Neither value is configurable by
environment variables.

### Schema and canonical bytes

Schema is `oc2-release-receipt/v1`. Encoding is UTF-8 without BOM. The complete
manifest is RFC 8785-style canonical JSON: no whitespace, object keys sorted by
UTF-16 lexical order, arrays absent, deterministic string escaping, and a final
newline forbidden. Parsers must reject noncanonical bytes rather than parse and
re-serialize them. Duplicate object keys, unknown keys, missing keys, wrong
types, invalid UTF-8, NUL/control characters, and trailing bytes fail closed.

Top-level keys, with exact types and bounds:

| Key | Type and rule |
|---|---|
| `archive_sha256` | lowercase ASCII 64-hex; exact digest of complete archive bytes |
| `members` | object with exactly the platform member names, each value lowercase ASCII 64-hex; 3 entries POSIX, 4 Windows |
| `platform` | one of `linux-x64`, `linux-arm64`, `macos-x64`, `macos-arm64`, `windows-x64` |
| `revision` | lowercase ASCII 40-hex; exact source revision passed to the build |
| `sbom` | object with exactly `path` and `sha256` |
| `schema` | exact string `oc2-release-receipt/v1` |

`sbom.path` is exactly `sbom.json` and `sbom.sha256` must equal
`members["sbom.json"]`. The SBOM is therefore a hashed archive member, not an
unbound URL or an ambient local file. SBOM bytes are valid UTF-8 JSON,
`spdxVersion` is `SPDX-2.3`, and its size is at most 65,536 bytes. The producer
must generate a release SBOM describing the packaged CLI and native payload;
the existing OpenTUI-only `crates/opentui-bridge/native/sbom.json` cannot be
substituted without this release linkage.

The parser budgets at most 128 JSON tokens, 256 UTF-8 bytes per key/path, 256
UTF-8 bytes per string, and 65,536 total bytes. `members` is compared as an
exact set, not a subset. All member hashes are uncompressed payload hashes.

Canonical Linux example, with deterministic fixture values:

```json
{"archive_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","members":{"native/lib/linux-x64/libopentui.so":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","oc2":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","sbom.json":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"},"platform":"linux-x64","revision":"0123456789abcdef0123456789abcdef01234567","sbom":{"path":"sbom.json","sha256":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"},"schema":"oc2-release-receipt/v1"}
```

The example is illustrative only. `a` through `d` are fixture hashes, not
release evidence.

## Archive contract

### Common rules

- Archive names are `oc2-<platform>.tar.gz` for POSIX and
  `oc2-windows-x64.zip` for Windows.
- Archive member names use `/` only. No backslash, absolute path, empty path,
  `.` component, `..` component, control byte, NUL, or path longer than 256
  UTF-8 bytes is accepted.
- No directory entries are emitted. Extraction may create parent directories
  implicitly. Every archive member is a regular file. Tar symlink, hardlink,
  device, FIFO, PAX/GNU metadata member, and ZIP directory/symlink/reparse
  member are rejected.
- Duplicate names, missing names, and extra names fail before extraction.
- POSIX tar entries are sorted lexically by member name, format `ustar`, mode
  `0755` only for `oc2`, mode `0644` for native and SBOM, uid/gid `0`, empty
  owner/group names, mtime `0`, and no extended headers. Gzip has mtime `0`, no
  filename/comment, fixed deflate level `9`; producer rerun must be byte
  identical.
- ZIP entries are sorted lexically, UTF-8 names with the UTF-8 flag, regular
  files only, DOS timestamp `1980-01-01 00:00:00`, no extra fields/comments,
  and method `stored` (method 0) for byte-deterministic output. The producer
  rerun must be byte identical.
- `compressed archive bytes` means complete `.tar.gz` or `.zip` file bytes,
  including headers. Limit is 134,217,728 bytes. Downloaders must enforce the
  same limit from Content-Length and while streaming; installers enforce it on
  the local file before reading members.
- Each regular member is at most 134,217,728 uncompressed bytes. The sum of all
  regular members is at most 134,217,728 bytes. SBOM also has its 65,536-byte
  specific cap. Bounds are checked during bounded streaming, before any target
  path is opened.

### Exact platform members

| Platform | Source Rust target | Format and exact member set, lexical order | Installed native behavior |
|---|---|---|---|
| `linux-x64` | `x86_64-unknown-linux-gnu` | tar.gz: `native/lib/linux-x64/libopentui.so`, `oc2`, `sbom.json` | Copy binary to `$INSTALL_DIR/oc2`; copy SO to `$INSTALL_DIR/../lib/libopentui.so` |
| `linux-arm64` | `aarch64-unknown-linux-gnu` | tar.gz: `native/lib/linux-arm64/libopentui.so`, `oc2`, `sbom.json` | Same layout, arm64 selected by platform |
| `macos-x64` | `x86_64-apple-darwin` | tar.gz: `native/lib/macos-x64/libopentui.dylib`, `oc2`, `sbom.json` | Copy binary to `$INSTALL_DIR/oc2`; copy dylib to `$INSTALL_DIR/../lib/libopentui.dylib` |
| `macos-arm64` | `aarch64-apple-darwin` | tar.gz: `native/lib/macos-arm64/libopentui.dylib`, `oc2`, `sbom.json` | Same layout, arm64 selected by platform |
| `windows-x64` | `x86_64-pc-windows-gnu` | ZIP: `libopentui.dll.a`, `oc2.exe`, `opentui.dll`, `sbom.json` | Copy `oc2.exe` and runtime `opentui.dll` beside each other in `$InstallDir`; verify but do not install GNU import archive |

The POSIX binary member is exactly `oc2`, never `bin/oc2`, `opencode`, or
`opencode2`. The Windows executable is exactly `oc2.exe`. The Windows GNU
import archive is a required build/package member and is hashed, even though it
is link-time input and is not copied into the runtime install directory. The
runtime DLL is copied beside `oc2.exe`, avoiding a PATH-dependent nested DLL.
No MSVC archive, `.lib`, or MSVC support is in scope. Current source evidence
for the pair is `artifacts.json:85-116`; current source evidence for MSVC
rejection is `artifacts.json:171-175` and `build_opentui.sh:29-31`.

## Runtime receipt grammar

The required machine interface is `oc2 --version`, not an optional doctor field.
It is simultaneously human-readable and machine-parseable:

```
oc2 <package-version> revision=<40-lowercase-hex>\n
```

ABNF:

```
version-output = "oc2 " package-version " revision=" revision LF
revision      = 40(LOWER-HEX)
package-version = 1*(ALPHA / DIGIT / "." / "-" / "+")
LF            = %x0A
```

Exactly one line is accepted. No CR, second line, prefix, suffix, stderr text,
duplicate `revision=`, uppercase hex, environment fallback, or version-only
fallback is accepted. The runtime owner reads compile-time `option_env!("GIT_COMMIT")`
written by `crates/cli/build.rs`; absent compile-time receipt is an explicit
runtime error and produces no valid receipt. It never reads `OC2_E2E_REVISION`,
`OC2_BUILD_REVISION`, `GIT_COMMIT` process environment, or git at runtime.

Installer parser limits are 4,096 combined stdout/stderr bytes, 5 seconds wall
time, 8 grammar tokens, and 40 revision bytes. The child is killed and waited
on at timeout; no detached process is allowed. The staged binary's parsed
revision must equal manifest `revision`. After atomic install, the installed
binary is invoked again with the same bounded parser and must produce the same
revision. This final observation binds the installed runtime receipt, not just
the staged copy.

## Installer transaction and error contract

All archive, sidecar, member, SBOM, and staged-runtime checks happen before any
existing target or native target is renamed, removed, or overwritten. Creating
an isolated temporary stage is allowed. Destination directory creation and
temporary destination files occur only after staged checks pass. Existing
target bytes must remain unchanged on every pre-write failure.

The installer rejects a destination or native directory that is a symlink, and
rejects a legacy adjacent `opencode`/`opencode.exe` with existing exit code 73.
After all checks, it writes temporary destination files, atomically swaps with
backup files, runs the installed receipt check, and restores backups on any
failure. Temporary stage, temporary destination, and backup files are removed
on success, failure, signal, timeout, or parser error.

| Code | Meaning | Examples |
|---:|---|---|
| 0 | success | all exact members, hashes, receipt checks, and transaction checks pass |
| 64 | usage/platform | missing or malformed flags, non-64-bit unsupported platform, malformed `--checksum` syntax, unsupported MSVC target |
| 65 | archive/manifest integrity | archive digest mismatch, manifest mismatch/noncanonical JSON, missing/duplicate/extra/unsafe member, type/hash/SBOM/size failure |
| 66 | input absent | archive or manifest path not found |
| 73 | legacy conflict | adjacent legacy `opencode` binary exists |
| 74 | identity/safety/transaction | runtime receipt missing/malformed/mismatch, staged command failure/timeout, destination symlink, copy/swap/rollback failure |

The code family is intentionally compatible with the current POSIX consumer at
`install-oc2.sh:177-207,327-364`. A malformed manifest is integrity code 65,
not usage code 64. A missing manifest file is input code 66. No error path may
fall back to version-only identity or an environment-provided revision.

## Ownership DAG and producer matrix

### Three implementation owners

1. **Producer owner `REL-PACKAGE-PRODUCER`**: new
   `scripts/make-release-archive.py` (stdlib only), plus its serialized release
   job wiring. Sets `OC2_BUILD_REVISION`, builds each target, collects exact
   members, computes uncompressed member hashes and complete archive hash, emits
   canonical sidecar, and fails closed on any missing native payload. It never
   signs or silently falls back to a different target.
2. **Installer owner `REL-PACKAGE-INSTALLER`**:
   `scripts/install-oc2.sh` and `scripts/install-oc2.ps1`. Adds sidecar input,
   exact schema parser, archive format validators, bounds, member hash checks,
   staged runtime parser, Windows DLL/import parity, transaction rollback, and
   post-install receipt check. It consumes producer output only; it never
   generates or edits a manifest.
3. **Runtime owner `REL-PACKAGE-RUNTIME-RECEIPT`**:
   `crates/cli/src/main.rs` (or an additive runtime module registered there).
   Makes `--version` emit the exact grammar from compile-time `GIT_COMMIT`, with
   no environment or git fallback. It does not own archive or installer code.

The release workflow/integration owner is serialized and owns only workflow
wiring and integration assembly, not the three implementation contracts. No
worker edits frozen tests, `build.rs`, repository policy, or controller state
without an explicit new task.

### Dependency DAG

```
existing build.rs receipt
        |
        +--> runtime receipt owner (exact --version grammar)
        |             |
        +--> producer owner (revision injected into each build)
                      |
              canonical archive + sidecar
                      |
               installer owner (POSIX + PS1)
                      |
                independent RED author
                      |
            serialized integration + verifier
```

Producer and runtime both depend on the existing source-receipt behavior.
Installer depends on the frozen schema and runtime grammar, not on an
environment override. RED authoring may proceed from this document using
deterministic fixture bytes, but GREEN cannot be claimed until producer,
installer, and runtime are wired in one integrated revision.

### Planned release job matrix

| Job label | Runner/target contract | Archive |
|---|---|---|---|
| `linux-x64` | target `x86_64-unknown-linux-gnu`, native `libopentui.so` | `oc2-linux-x64.tar.gz` |
| `linux-arm64` | target `aarch64-unknown-linux-gnu`, native `libopentui.so` | `oc2-linux-arm64.tar.gz` |
| `macos-x64` | target `x86_64-apple-darwin`, native `libopentui.dylib` | `oc2-macos-x64.tar.gz` |
| `macos-arm64` | target `aarch64-apple-darwin`, native `libopentui.dylib` | `oc2-macos-arm64.tar.gz` |
| `windows-x64` | target `x86_64-pc-windows-gnu`, `opentui.dll` plus `libopentui.dll.a` | `oc2-windows-x64.zip` |

Each job checks out one exact source revision, exports
`OC2_BUILD_REVISION=$(git rev-parse HEAD)`, validates 40 lowercase hex, builds
with the target, and runs the deterministic archive twice in disposable roots.
The two archive bytes and both canonical manifests must match. The matrix is a
future producer contract, not evidence that these hosted runners currently
exist. `x86_64-pc-windows-msvc` is deliberately absent and must remain a
fail-closed unsupported target.

## Trust and failure matrix

| Fixture/input | Detection phase | Required result | Target side effect |
|---|---|---|---|
| Good archive + canonical sidecar + expected checksum | pre-write, then post-install | exit 0; staged and installed receipts equal manifest revision | new files atomically installed |
| Missing archive/sidecar | argument/input | exit 66 | old target unchanged |
| Archive over 128 MiB compressed | file-size gate | exit 65 | old target unchanged |
| Manifest over 64 KiB, invalid UTF-8, BOM, whitespace, noncanonical, unknown/duplicate key | sidecar parser | exit 65 | old target unchanged |
| Bad checksum syntax, missing required flags | argument parser | exit 64 | old target unchanged |
| Archive digest differs from `--checksum`, or sidecar archive digest differs | digest gate | exit 65 | old target unchanged |
| Missing, duplicate, extra, absolute, traversal, backslash, overlong member | archive listing | exit 65 | old target unchanged |
| Tar symlink/hardlink/device/PAX, ZIP directory/symlink/reparse, nonregular member | type gate | exit 65 | old target unchanged |
| Missing or extra POSIX native/SBOM, Windows DLL/import/SBOM | exact member set | exit 65 | old target unchanged |
| Member hash mismatch, binary/native/DLL/import/SBOM tamper | bounded staged hash | exit 65 | old target unchanged |
| Member over 128 MiB or aggregate over 128 MiB; SBOM over 64 KiB | bounded stream | exit 65 | old target unchanged |
| Runtime output missing, malformed, duplicate revision, wrong revision, extra line, timeout | staged parser | exit 74 | old target unchanged |
| Legacy adjacent binary or destination symlink | destination preflight | exit 73 or 74 | old target unchanged |
| Copy/swap/post-install receipt failure | transaction rollback | exit 74 | prior target/native bytes restored |
| Uppercase digest/revision, manifest `members` subset, env-only receipt | parser/binding | exit 65 or 74 as above | no fallback, no write |

## Future RED fixture matrix and exact commands

The independent RED author creates `crates/cli/tests/packaged_revision_binding.rs`
and disposable fixtures only. Tests are std-only, network-free, and do not edit
the frozen `installed_default_entrypoint.rs`. A fixture builder must set tar
`ustar` fields explicitly, gzip mtime 0/no name, ZIP stored entries with the
fixed timestamp, sorted names, fixed modes, and fixed payload bytes. It must
compute all expected hashes from bytes rather than embed claimed success logs.
Runtime helper binaries emit either the exact one-line grammar or one selected
malformation; no helper may read the ambient environment for revision.

Required RED command manifest:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 cargo test -p opencode-rk-cli --test packaged_revision_binding -- --test-threads=1
shasum -a 256 crates/cli/tests/packaged_revision_binding.rs
```

PowerShell parity command, on a Windows GNU fixture runner:

```powershell
pwsh -NoProfile -File tests/packaged_revision_binding.ps1
```

The controller freezes both test hashes before implementation. Required cases:

| ID | Fixture | Expected |
|---|---|---|
| T01 | deterministic good POSIX archive and sidecar | 0; exact installed revision; target receipt observed |
| T02 | deterministic good Windows ZIP with `oc2.exe`, DLL, import archive, SBOM | 0; DLL beside executable; import member verified |
| T03 | absent manifest, absent archive | 66; target sentinel byte unchanged |
| T04 | malformed/noncanonical/oversized/duplicate/unknown manifest | 65; target unchanged |
| T05 | archive digest mismatch and sidecar/archive digest mismatch | 65; target unchanged |
| T06 | wrong member hash: binary, native, DLL, import, SBOM | 65; target unchanged |
| T07 | missing, duplicate, extra, traversal, absolute, backslash, overlong member | 65; target unchanged |
| T08 | tar link/device/PAX and ZIP directory/symlink/reparse member | 65; target unchanged |
| T09 | per-member over 128 MiB, aggregate over 128 MiB, compressed archive over 128 MiB, SBOM over 64 KiB | 65 before target write |
| T10 | runtime missing/malformed/uppercase/duplicate/wrong revision/extra-line/timeout | 74; target unchanged |
| T11 | POSIX native wrong suffix/path and Windows missing DLL/import pair | 65; target unchanged |
| T12 | legacy adjacent binary, destination symlink, injected copy failure | 73/74; old bytes restored |
| T13 | successful upgrade then post-install receipt check | 0; installed receipt exactly equals manifest revision |
| T14 | producer rerun in two disposable roots for all five platforms | byte-identical archives and canonical manifests |

The test author must capture compiling RED before producer/runtime/installer
implementation. Current expected RED evidence remains verifier commit
`9de9ee7e8bffc7b3cce16d1d2951adeb43735a7b`, not a fabricated product result.

## Explicit resolution of verifier gaps

| Verifier gap | Owner and file boundary | Final decision | PASS-ready observable |
|---|---|---|---|
| Manifest transport/trust | Producer: `scripts/make-release-archive.py`; consumers: `scripts/install-oc2.sh`, `scripts/install-oc2.ps1` | External `--manifest`; `archive_sha256` must equal trusted `--checksum`; unsigned caveat explicit | Sidecar is not self-hashed; mismatch exits 65 before write |
| Exact schema/canonicalization | Producer sidecar writer plus installer parsers in both scripts | Six top-level keys, JCS-style canonical UTF-8, exact types, duplicate/unknown rejection, 64 KiB/128-token bounds | Example above parses only in canonical byte form |
| Full hash format | Producer hash writer; installer parsers in both scripts | Revision exactly lowercase 40-hex; archive/member/SBOM exactly lowercase 64-hex | Uppercase, short, nonhex rejected |
| Sidecar self-reference | Producer sidecar writer; archive assembly in `scripts/make-release-archive.py` | Sidecar external; SBOM is archive member and hashed in `members` | No circular archive hash |
| POSIX path mismatch | Producer archive assembly; POSIX consumer `scripts/install-oc2.sh` | Archive uses installer slugs `linux-x64`, `linux-arm64`, `macos-x64`, `macos-arm64`; binary member exactly `oc2` | Table and exact member set are unambiguous |
| Windows payload | Producer archive assembly; Windows consumer `scripts/install-oc2.ps1` | ZIP has `oc2.exe`, `opentui.dll`, `libopentui.dll.a`, `sbom.json`; DLL copied beside executable; import verified but not runtime-installed | PS1 parity test T02/T11 |
| Windows safety parity | Windows consumer `scripts/install-oc2.ps1` | Replace `Expand-Archive` with bounded `ZipArchive` validation, exact set/type/path/hash checks, staged receipt, rollback | Same failure matrix and codes as POSIX |
| 128 MiB limits | Both consumers; producer preflight in `scripts/make-release-archive.py` | Per-member and aggregate uncompressed caps remain 128 MiB; archive compressed cap 128 MiB; SBOM/manifest 64 KiB | T09 covers both formats and all limits |
| Runtime interface | Runtime owner `crates/cli/src/main.rs` or additive module registered there | Exact one-line `oc2 <version> revision=<40hex>` from compile-time receipt; no env fallback | T10 rejects parser ambiguity; T13 observes installed value |
| Producer owner | Serialized integration owner plus `scripts/make-release-archive.py` and release job wiring | `REL-PACKAGE-PRODUCER`, `scripts/make-release-archive.py`, serialized release matrix | Producer absence is no longer an unspecified owner; implementation still pending |
| Three-owner integration | Serialized integrator; boundaries are the producer script, two installer scripts, and runtime module | Producer, installer, runtime owners and explicit DAG above; verifier remains independent | No owner may claim another surface or acceptance |
| SBOM linkage | Producer archive/sidecar writer; both installer consumers | Release `sbom.json` is exact member, manifest path/hash must match member, SPDX 2.3 bounded JSON | Tamper/missing SBOM exits 65 |
| Determinism | Producer `scripts/make-release-archive.py` | Fixed paths/order/modes/timestamps/format/compression; producer rerun T14 | Identical bytes, not merely equal metadata |
| Missing/extra/path traversal/symlink | Both installer scripts' archive validators | Exact member set and regular-file-only validators for tar and ZIP | T07/T08 before extraction/write |
| Download bounds | Release downloader job plus both installer scripts | Downloader and local installer enforce 128 MiB compressed bytes, including streaming count | Oversize rejected before archive processing |
| Installer write boundary | Both installer scripts' stage/transaction functions | All checks in temp stage; destination mutation only after staged receipt; post-write failure restores backups | Sentinel target unchanged for every pre-write case; T12 rollback |
| Signing/notarization/MSVC | Release matrix owner; no implementation file in this Phase 1 lane | Explicitly out of Phase 1; GNU Windows only; no authenticity claim | Verifier must not infer any of these from hashes |

## Validation performed

```sh
git grep -n 'OC2_BUILD_REVISION\|GIT_COMMIT\|Expand-Archive\|tar -x\|artifacts.json\|sbom.json' -- crates scripts .github
shasum -a 256 crates/cli/tests/installed_default_entrypoint.rs
git diff --check
```

Results:

- Source inventory: producer absent; existing build receipt and both installer
  consumers identified; native SBOM/artifact surfaces identified.
- Frozen installed test SHA-256 unchanged:
  `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317`.
- `git diff --check`: clean before this worklog is staged.
- No product RED/GREEN, build, network, archive extraction, or database command
  applies to this research lane. Verifier commit `9de9ee7...` is the captured
  failing contract fixture.
- Resource observation: text/Git only; no child process, archive, or network
  workload.

## Remaining status

The schema and ownership blockers are resolved as decisions. Producer,
installer, runtime, RED authoring, and independent verification remain future
tasks. This artifact does not claim those implementations or release evidence.
