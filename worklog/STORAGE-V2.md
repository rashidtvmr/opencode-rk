# Storage schema and crash protocol wave

User requested self-review and writing both designs to rashidtvmr/opencode-rk,
not another architecture explanation in chat. Base: 53aac452acfe64e5adab3fd4fcc9597f90e79cda.

Added exact workspace/catalog SQL, query/index map, normative crash protocol,
evaluator findings, Python/SQLite contract tests, Rust integration tests using the
same DDL, and local verification evidence. docs/STORAGE.md was absent from the
inspected GitHub base despite existing in the old local project kit.

Reference implementation: crates/storage/src/lib.rs blob
 af01d1cd556038be19939f50a999e2bfb575998b. Root AGENTS.md and the prepared plan,
TDD/security/storage contracts informed boundaries. This is a reviewed design
proposal, not activation of a new runtime or acceptance of a product story.

Key changes: GC tombstones/leases; shared immutable payloads; composite ownership
FKs; FULL destructive/dispatch barriers; bounded outbox cursor; uncertainty states;
snapshot-specific backups. No release flags, source pins or user databases changed.

Actual test scope and toolchain limitations are in validation/storage-v2-verification.md.
