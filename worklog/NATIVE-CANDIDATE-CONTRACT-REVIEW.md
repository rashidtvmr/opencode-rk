# Native candidate independent contract review

Review-only. No product, original frozen test, policy or acceptance-state edits.

## Candidate and integrity

Reviewed `lane/TUI-011-artifact-matrix` at `83b3827` against integrated
`d382fdf`. Streamed all six manifest-declared blobs directly from Git in 64 KiB
chunks, with a 128 MiB per-artifact ceiling. Every blob matched its declared
SHA-256 and byte size. Apple ARM64 header independently decoded as
little-endian Mach-O 64-bit (`0xfeedfacf`), ARM64 (`0x0100000c`), dylib (6).
This proves integrity/header identity, not runtime behavior or source provenance.

## Unchanged frozen candidate test reproduction

Copied exact Git blobs of the test, manifest, notices, SBOM and Apple ARM64
library to an approved disposable fixture. Compiled the std-only test directly
with `rustc --edition=2021 --test`, preserving its bytes and setting
`CARGO_MANIFEST_DIR` to the fixture root. Ran with one test thread and 30-second
compile/run deadlines. No Cargo workspace build or library execution occurred.

- Compile: exit 0.
- Tests: 3 executed; 1 passed, 2 failed; exit 101.
- Elapsed compile/run: 0.138 seconds.
- Test SHA-256:
  `efb642364b6a4dc559fff019d21c14f778c170f34f96bf5e7183b4aeb536aa40`.
- Log SHA-256:
  `c397aede5b3d6cbad23457a0c7d11df0507d1fe187f9341c77cc8fe7d944ac3e`.
- Receipt/log location:
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/native-frozen-review-20260930`.

## Exact failures and additional source defects

`crates/opentui-bridge/tests/native_artifact_manifest.rs` on the candidate:

- Line 122 rejects the real 6,863,648-byte library because it applies a 64 KiB
  sidecar-sized budget to the binary.
- Line 95 rejects the verified lowercase SHA-256 because the character
  predicate requires both hexadecimal and ASCII lowercase for every character;
  numeric hexadecimal characters are not ASCII lowercase.
- Header constants at lines 14-16 are incorrect for Mach-O ARM64; the parser
  also uses big-endian CPU decoding for this little-endian artifact. These
  assertions were not reached because the size assertion failed first.

These are disputed frozen contracts. They cannot be edited, bypassed or
replaced by implementation workers to obtain GREEN.

## Integration implications

The candidate is not GREEN and cannot be integrated as a completed TUI-011.
The current integrated bridge build script additionally accepts only `.a` or
`.so`, so supplying the verified Apple `.dylib` alone does not unblock APP-012.
The alternative `lane/TUI-011-prod` script handles `.dylib`, but its Windows
branch accepts either import-library flavor without enforcing the target ABI.
Runtime closure, clean-target loading and native rendering remain unproven.

Owner/controller contract review must address these findings and the legacy
backlog baseline conflict before product repair leases can proceed. No acceptance
is inferred from candidate metadata or independent hash verification.
