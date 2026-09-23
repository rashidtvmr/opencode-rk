# BRIDGE-041 persistence

Claim: crash-safe text IO mirroring `util/persistence.ts:1-33` @ a0d9b6c.
Evidence: readText=Bun.file.text (no cap), writeText=mkdir+Bun.write (not atomic), writeJsonAtomic=tmp+rename pid.uuid (ts:22-33); callers history/frecency/stash JSONL append, kv/local atomic; epilogue.tsx in-memory only, durable write app.tsx:361.
Target: `crates/opentui-bridge/src/persistence.rs`, `#![forbid(unsafe_code)]`, std only.
Tests (frozen, in-file, temp_dir only): roundtrip, write-over-cap no-touch, read-over-cap, missing Io, failed-write original intact, append newline, invalid utf8, json sniff. RED: file absent pre-write; impl logically green, cargo NOT run per scope.
Decision: harden ALL writes tmp+rename (upgrade over TS writeText); JsonShape brace/bracket sniff, no serde.
Unknown: Bun.write flush/fsync semantics exact; untested on disk (no cargo run).
