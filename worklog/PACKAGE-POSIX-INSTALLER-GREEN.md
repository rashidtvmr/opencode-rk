# PACKAGE-POSIX-INSTALLER-GREEN

## Claim

- Task: `PACKAGE-POSIX-INSTALLER-GREEN`
- Session: `ses_f296a2db7fferyABqhZt0Cup7P`
- Continued by orchestrator session `ses_f3c4de578ffelQv59xDXmOs03B` after
  lawful reclaim when `ses_f295d2e2affepsIfgS0JJAaE4P` exhausted its step
  budget with a reproducible positive-fixture parser failure.
- Branch: `lane/PACKAGE-POSIX-INSTALLER-GREEN`
- Base: `ca7a2df711ac5ce506e88c821d37ce94f3b1a940`
- Owned product file: `scripts/install-oc2.sh`
- Frozen RED: `crates/cli/tests/packaged_revision_binding.rs`, SHA-256 `d0a800ed7bda8b623aa427604c6ad2c28acdd37633d391079d651246ad0a8858`
- No tests, PowerShell, producer, runtime Rust, dependency, workflow, verifier, or controller edits.

## Source evidence

- `scripts/install-oc2.sh:57-71,177-202`: no `--manifest`; checksum-only input.
- `scripts/install-oc2.sh:210-325`: two-member archive checks; bounded required streams; no SBOM/member hashes.
- `scripts/install-oc2.sh:327-364`: substring identity checks, not exact receipt grammar.
- `scripts/install-oc2.sh:366-426`: pre-write staging plus backup transaction; no post-install receipt or active rollback after final verification.
- `worklog/PACKAGE-RECEIPT-SCHEMA-CORRECTION.md:39-221`: canonical sidecar, exact schema, archive closure, receipt grammar, codes, bounds, transaction contract.
- `worklog/PACKAGE-RECEIPT-SCHEMA-CORRECTION-VERIFY.md:21-36,82-100`: independent source-gap verification.
- `worklog/PACKAGE-RECEIPT-BINDING-RED.md:68-103`: frozen compile-valid RED; hash above.
- `crates/cli/tests/packaged_revision_binding.rs:184-319`: executable T01-T05.

## Target contract

- Required install inputs: archive, trusted checksum, manifest. Usage 64; absent input 66; integrity 65; identity/transaction 74; legacy 73.
- Sidecar: regular, non-symlink, <=65536 bytes; exact canonical ASCII JSON grammar for fixed schema; exact host platform and three-member closure; all hashes lowercase; SBOM linked.
- Archive: regular, non-symlink, <=134217728 bytes; USTAR regular-file-only, exact sorted closure, no duplicate/unsafe/nonregular entries; per-member/aggregate/SBOM bounds.
- Staged and installed binary: exact `oc2 <version> revision=<40-lowerhex>\n`, no stderr, bounded 4096 combined bytes, five seconds; revision equals manifest.
- Mutation only after all pre-write checks. Backup, swap, installed receipt, rollback on every post-mutation failure. Consumer only; unsigned checksum is integrity, not authenticity.

## Implementation evidence

- `scripts/install-oc2.sh:159` `parse_manifest`: bounded `od` byte stream; closed canonical ASCII grammar; exact keys/order/types/platform/native closure/hash formats/SBOM linkage/trailing-byte rejection. No `eval`, general-JSON claim, Python, or jq.
- `scripts/install-oc2.sh:306` `validate_ustar_header`; `:374` `validate_ustar_headers`: bounded gzip expansion; exact sorted offsets; regular USTAR headers; name/mode/uid/gid/size/mtime/type/magic/version/checksum/zero EOF; rejects PAX/GNU metadata, traversal/duplicate/extra/nonregular entries.
- `scripts/install-oc2.sh:403` `verify_runtime_receipt`: scrubbed revision environment, Linux `setsid` or host job-control process ownership, five-second watchdog, 2048-byte-per-stream file limit, 4096-byte combined parser cap, exact package grammar, one LF, and no stderr from the payload.
- `scripts/install-oc2.sh:523` `rollback_install`; transaction remains active through installed observation at `:898`; signal, swap, receipt, and cleanup paths converge.
- Input copy uses `cp -P`; staged archive/manifest are rechecked regular, non-symlink, and bounded. Archive hash must equal both trusted checksum and manifest. Every member and linked SBOM hash is compared after bounded extraction.

## Decisions

- No `eval`; no regex-based claim of general JSON parsing.
- Manifest parser is a closed schema grammar, not a general JSON parser.
- Current native tools suffice: `awk`, `od`, `dd`, POSIX `gzip`, SHA-256 utility, and `iconv`; no new dependency or workflow edit. Linux uses optional util-linux `setsid`; macOS uses job control.
- SBOM receives bounded UTF-8, exact SPDX-2.3 marker, closure/hash/size checks. No claim of validating arbitrary SBOM JSON schema.
- Consumer only. Unsigned checksum proves integrity only; no producer, signature, attestation, or authenticity claim.
- Cargo-heavy work remained serialized: the frozen target ran only after the recovery lane released the semaphore.

## Validation

- `sh -n scripts/install-oc2.sh` and `/bin/dash -n scripts/install-oc2.sh` -> exit 0 after final implementation.
- `shellcheck scripts/install-oc2.sh` -> unavailable, exit 127 (`rtk: No such file or directory`); no substitute claimed.
- `git diff --check` -> exit 0.
- `shasum -a 256 crates/cli/tests/packaged_revision_binding.rs` -> `d0a800ed7bda8b623aa427604c6ad2c28acdd37633d391079d651246ad0a8858`.
- A disposable canonical archive/manifest probe executed the script under
  `/bin/dash`, installed both fixture members, and emitted the exact revision
  receipt. It used a generated tree under the approved OpenCode temporary root
  and removed it afterward.
- No browser, network, database, credentials, or user data were accessed.

## Tests

- RED receipt inherited: compile PASS, 0/5 behavioral failures at `ca7a2df`.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -q -p
  opencode-rk-cli --test packaged_revision_binding -- --test-threads=1` -> 5
  passed, 0 failed, 0 ignored. Existing unrelated dead-code warnings remain.
- Frozen hash after GREEN remains
  `d0a800ed7bda8b623aa427604c6ad2c28acdd37633d391079d651246ad0a8858`.
- Post-run process scan found no `packaged_revision_binding`, fixture, or
  installer survivor.
- `python3 tools/validate_repository.py` remains blocked by the 51 pre-existing
  backlog-exhaustion reconciliation errors on the inherited base. No policy,
  controller, acceptance, or guard file was changed; integration authority is
  still required before this candidate can reach `main`.

## Remaining status

- Candidate GREEN is ready for commit/push and independent verification.
- This proves the POSIX consumer against T01-T05 only. It does not prove a
  producer, Windows consumer, release workflow, integrated runtime receipt,
  signing, authenticity, or packaged parent journey.

## Continuation diagnosis

- The canonical parser had an exact off-by-one grammar defect: the fixed
  `members` prefix consumed the first key's opening quote and the next
  expectation consumed a second opening quote. Consequently every canonical
  positive manifest failed before hash validation.
- Corrected the prefix from `\",\"members\":{\"` to
  `\",\"members\":{`; the member-key expectation remains the sole owner of
  the opening quote.
- Corrected four further exact defects exposed by the frozen suite: empty
  manifest normalization changed usage 64 to integrity 65; ancestor-wide
  symlink rejection rejected macOS `/var -> /private/var`; the USTAR zero-tail
  check overlapped the second version byte; and the EOF `dd` probe treated a
  byte offset as a block index and expected 1024 rather than 2048 hex digits.
- Direct destination symlink checks remain immediately before mutation. A
  shell installer cannot provide descriptor-bound directory identity; that
  stronger TOCTOU property remains an explicit parent-level gap rather than a
  claim made by this candidate.
