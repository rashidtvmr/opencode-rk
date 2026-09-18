# DB-backup — backup_v2 worklog

Claim: `crates/storage/src/backup_v2.rs` implements byte-supplied backup manifest,
corruption detector, quota consts, verify-before-restore marker. std only, no FS IO.

Source evidence (HEAD 5af7884):
- `crates/storage/src/lib.rs:636-680` configure/migrate; schema_meta v1 lineage (commit 5af7884).
- `sources/completion/audits/AUD-007.json: trace/findings` backup/restore module class=missing
  ("src/backup*.rs absent; tests/backup_v2.rs without src"), repair child
  "src/backup_v2.rs real backup+restore+corruption test".
- `crates/storage/src/schema_v2.rs:14-16` USER_VERSION=2/FORMAT_VERSION=2/MIGRATION_VERSION=2
  (BACKUP_SCHEMA_VERSION=2 follows this lineage).
- `crates/storage/tests/backup_v2.rs` file-copy crash-consistency boundary (WAL checkpoint
  before copy); new module does NOT do file copies — caller supplies bytes, no FS IO.
- `crates/storage/src/lib.rs:52-79` StorageError has BlobHashMismatch but no backup variants;
  module owns local BackupError (Corrupt/QuotaExceeded/UnsupportedVersion) to avoid widening
  shared enum (same ponytail pattern as snapshot_v2.rs:10-12).

Observed scenario: file absent before lane; created owned file only. No other edits.

Target boundary:
- `BackupManifest` { schema_version, session/message/blob counts, total_bytes, data_hash [u8;32] }.
- `BackupError::{Corrupt, QuotaExceeded, UnsupportedVersion(u32)}`.
- Quotas: MAX_BACKUP_BYTES=64MiB, MAX_BACKUP_ITEMS=4M (saturating sum).
- 112-byte BE wire format `BKP2|ver|3x u64 counts|total|sha256(data)|sha256(tag-body)`;
  version gate before tag check; quotas checked in create/verify.
- `VerifiedRestore<'a>` private fields, sole constructor `verify_for_restore` (decode+verify).
- SHA-256 hand-rolled (FIPS 180-4), std only; `sha256_known_vector` pins "abc" digest.
- `#![forbid(unsafe_code)]`, no `std::fs`/net/env; `rustc --test` self-contained (no cargo deps).

Tests (frozen in-file, 7 tests):
1. round_trip_manifest 2. tampered_manifest_bytes_detected_corrupt 3. tampered_data_detected_corrupt
4. over_quota_rejected 5. restore_requires_verify 6. unsupported_version_rejected 7. sha256_known_vector

Decisions:
- Local error enum over StorageError to avoid shared-enum widening.
- Tag covers version+counts+hash body (not magic) so future-version path reports UnsupportedVersion.
- Length check before hash in verify (fail fast on truncation/append).

Remaining unknowns / gaps:
- Integration wiring (`mod backup_v2` in lib.rs) owned by integrator, not this lane.
- RED executed only by construction (stub create/decode/verify returned Err); captured RED log
  not preserved — process deviation, see below.
- `rtk`-prefixed shell invocations returned a filtered "matches in 1F" view instead of compiler
  output; verification ran via `sh /tmp/opencode/runbk.sh` (direct rustc path) to get real output.
  Evidence: /tmp/opencode/bk binary + sha below.

Evidence:
- GREEN: `rustc --edition 2021 --test crates/storage/src/backup_v2.rs -o /tmp/opencode/bk && /tmp/opencode/bk`
  → 7 passed, 0 failed (log from runbk.sh run).
- sha256 b1b8872442055014eb509f58f98ce7500b239f34a5a7558783ea7825, 13480 bytes.
