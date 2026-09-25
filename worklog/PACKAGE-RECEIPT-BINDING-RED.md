# PACKAGE-RECEIPT-BINDING-RED

Status: valid RED captured. Owned test/scratchpad/ledger only. No product,
installer, runtime, dependency, workflow, verifier, controller, or frozen-test
edits. No implementation or acceptance claimed.

## Claim and boundary

- Task: `PACKAGE-RECEIPT-BINDING-RED`
- Session: `ses_f2b929c84ffep7149gP0A5cm5r`
- Branch: `red/PACKAGE-RECEIPT-BINDING`
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/red-package-receipt-binding`
- Owned test: `crates/cli/tests/packaged_revision_binding.rs`
- Owned scratchpad: `worklog/PACKAGE-RECEIPT-BINDING-RED.md`
- Ledger row: own row only, claimed through `tools/completion_claims.py` after
  initial `git status` showed only the pre-existing ledger modification.
- No installer, source, dependency, workflow, verifier, controller, or frozen
  test edits.

## Source evidence

- `scripts/install-oc2.sh:57-71` current parser accepts only
  `--version`, `--archive`, `--checksum`, `--install-dir`, `--uninstall`; no
  `--manifest`. Missing required archive/checksum is rejected at lines 177-181.
- `scripts/install-oc2.sh:194-202` verifies archive checksum before writes;
  lines 215-261 validate current two-member tar layout; lines 327-364 run
  staged `--version`/`--help` and only require substring `oc2`; destination
  mutation starts at lines 366-426.
- `scripts/install-oc2.sh:190-192` selects host platform and expected native
  path; current platform on this macOS arm64 worktree is `macos-arm64`.
- `scripts/install-oc2.ps1:5-12,36-67` has no manifest parameter, uses
  `Expand-Archive`, checks only `oc2.exe`, and copies before runtime validation.
- `crates/cli/build.rs:35-47,55-60,171-214` is the existing exact lowercase
  40-hex compile-time revision source. `crates/cli/src/main.rs:60-65,376-414`
  exposes Clap/doctor version information without the required runtime receipt.
- Correction contract `worklog/PACKAGE-RECEIPT-SCHEMA-CORRECTION.md:39-105,110-192,311-356`
  fixes the external canonical sidecar, exact member/revision grammar, failure
  semantics, and T01-T14 RED matrix used here.
- Verifier `worklog/PACKAGE-RECEIPT-SCHEMA-CORRECTION-VERIFY.md:21-36,82-100`
  confirms manifest flag/parser/runtime revision/Windows parity/producer are
  source blockers, while RED authoring is accepted as the next lane.

## Observable RED contract

- T01: omitted `--manifest` is usage code 64; both sentinel target files remain
  byte-identical.
- T02: canonical sidecar + exact POSIX member closure + matching archive hash
  installs successfully; installed `oc2 --version` is exactly
  `oc2 <package-version> revision=<40 lowercase hex>\n`.
- T03: archive digest mismatch and member digest mismatch fail 65; manifest
  revision/runtime mismatch fails 74; all pre-write failures preserve both
  sentinel files.
- T04: extra, duplicate, and traversal members fail 65 before writes.
- T05: staged and installed runtime version grammar is exact, with no stderr.
- Windows: source-only contract requires manifest transport, bounded ZIP
  handling, SBOM, DLL/import member names, and revision receipt. It does not
  claim PowerShell execution on this host.

## Test design

`crates/cli/tests/packaged_revision_binding.rs` uses only std APIs. It creates
unique disposable roots, env-clears installer/runtime children, fixes revision
and package version, writes USTAR fixtures with explicit metadata, compresses
with `gzip -n -9`, computes SHA-256 through available system tools, and invokes
`/bin/sh scripts/install-oc2.sh` with proposed `--manifest`. No network,
signing, user DB, Cargo, or product changes.

## Validation receipt

Focused command, harness timeout 240 seconds (Cargo slot 1):

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --test packaged_revision_binding -- --test-threads=1
```

Result: compile succeeded; `running 5 tests`; `0 passed; 5 failed`; all five
failures are behavioral assertions, not fixture panics, malformed archives,
unavailable commands, or compile errors.

| Test | Expected | Observed cause | Side effects |
|---|---:|---|---|
| T01 missing manifest | 64 | Current installer rejects the canonical three-member archive at its legacy two-member allowlist before argument handling; no `--manifest` exists. | Sentinels unchanged. |
| T02 canonical good | 0 | Current parser rejects `--manifest` as unknown, exit 64. | Sentinels unchanged. |
| T03 archive/member digest mismatch | 65 | Current parser rejects `--manifest` as unknown, exit 64. | Sentinels unchanged. |
| T03 runtime revision mismatch | 74 | Current parser rejects `--manifest` as unknown, exit 64. | Sentinels unchanged. |
| T04 extra/traversal/duplicate | 65 | Current parser rejects `--manifest` as unknown, exit 64. | Sentinels unchanged. |
| T05 exact receipt grammar | 0 | Current parser rejects `--manifest` as unknown, exit 64. | Sentinels unchanged. |

T01 was corrected to use `Fixture::new_without_manifest` with the complete
three-member archive. The earlier fixture-construction panic is eliminated.
T02–T04 also have complete archive fixtures. The current shell installer cannot
yet reach the requested member, digest, or staged-runtime checks; that is the
intended source gap.

## Freeze and integrity

- Test SHA-256: `d0a800ed7bda8b623aa427604c6ad2c28acdd37633d391079d651246ad0a8858`
- Installed entrypoint frozen hash: `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317`
- Native artifact frozen hash: `cb7d4cde7c3aed1916814c9aab747b68aa48015518c444e90bd8a58e66bd5c8b`
- `git diff --check`: clean.
- `vm_stat` snapshot: 5,077 free 16 KiB pages; no threshold breach observed.
- RED log: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/package-receipt-red.log`
- No network, signing, secrets, user DB, or product process used.

## Ownership required for GREEN

- Producer: `scripts/make-release-archive.py` plus serialized release matrix.
- POSIX installer: `scripts/install-oc2.sh` manifest/schema/hash/closure/runtime checks.
- Windows-GNU installer: `scripts/install-oc2.ps1` bounded ZIP/native/receipt parity.
- Runtime receipt: `crates/cli/src/main.rs` exact compile-time `--version` grammar.

## Deferred matrix T06-T14

- T06 wrong binary/native/SBOM member hashes.
- T07 missing, absolute, backslash, overlong, and unsafe member variants.
- T08 tar link/device/PAX and ZIP directory/symlink/reparse variants.
- T09 per-member, aggregate, compressed archive, and SBOM size caps.
- T10 runtime missing/malformed/uppercase/duplicate/wrong/extra-line/timeout.
- T11 wrong native suffix/path and Windows missing DLL/import pair.
- T12 legacy adjacency, destination symlink, copy/swap rollback.
- T13 successful upgrade plus post-install receipt equality.
- T14 producer rerun byte identity across all five platforms.

## Unknowns and deviations

- Producer implementation, installer implementation, runtime implementation,
  and Windows execution remain separate owners. This lane tests the consumer
  seam only; it does not import nonexistent Rust APIs.
- POSIX fixture uses a shell payload because the current staged identity seam
  executes the packaged member. This is deterministic and behaviorally
  exercises the installer; it is not a product/runtime claim.
- The current script's two-member archive contract differs from the corrected
  three-member POSIX contract. This is intentional RED evidence, not a fixture
  waiver.
