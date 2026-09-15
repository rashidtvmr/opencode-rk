# RW-SLICE-01 - Native codebase index/snapshot (file list + symbol table + content hashes)

Status: PROPOSED. Kind: product. Runtime optional: True.
Mandatory for full declared release: yes.
Requirements: proposed REQ-NEW (codebase index/snapshot, local-only); REQ-032 pattern (default-on with opt-out, zero hidden cost when off).
Dependencies: none.
Test obligations: RW-IDX-T01, RW-IDX-T02, RW-IDX-T03, RW-IDX-T04, RW-IDX-T05.

## User-observable outcome

A bounded, local-only codebase snapshot for one workspace root: file list
(path, size, content hash) plus a symbol table (name, kind, file, line span)
built by a single scan. Serves file/symbol lookup without re-walking the tree.
Default-on with opt-out flag; zero hidden cost when off. No network, no DB
mutation, no secret logging.

## Source evidence

- PLAN.md section 3 ADR-001: one native domain runtime, no process per subagent.
- PLAN.md section 5: slice template, additive registration fragments, integrator assembles lib.rs.
- PLAN.md section 6 + docs/TDD.md sections 2-5: RED-compiling-fail, freeze, GREEN.
- tasks/TOOL-014.md: task-card model (bounded store, byte budget, eviction).
- tasks/BASE-006.md: REQ-032 gating precedent (default-off flags, gate before init); here default-on index with opt-out flag, same zero-cost-when-off proof.

## Observable contract (happy path)

- `IndexConfig { enabled: bool, max_files: usize, max_bytes: u64, max_symbols: usize }`: `Default` is `enabled: true, max_files: 20000, max_bytes: 200_000_000, max_symbols: 100000`. `disabled()` scans nothing.
- `FileEntry { path: RelPath, size: u64, hash: blake3/sha256 hex }`; `Symbol { name, kind: Fn|Struct|Enum|Trait|Mod|Const, file: RelPath, start_line: u32, end_line: u32 }`.
- `Snapshot { root, files: Vec<FileEntry>, symbols: Vec<Symbol>, scanned_at_ms: u64, truncated: bool }`.
- `build_snapshot(root, cfg) -> Result<Snapshot, IndexError>`; `lookup_file(snap, path) -> Option<&FileEntry>`; `lookup_symbol(snap, name) -> Vec<&Symbol>` (exact match, sorted by file then line).
- Deterministic: same tree bytes + cfg => identical file/symbol order (sorted paths); no wall-clock in ordering; `scanned_at_ms` informational only.
- Disabled (`enabled: false`): returns empty snapshot (`files=[], symbols=[], truncated=false`) without touching the filesystem.

## Failure states

- Missing/unreadable root: `Err(IndexError::RootUnreadable)`; no partial snapshot returned.
- Over caps (`max_files`/`max_bytes`/`max_symbols` exceeded): stop scan, return partial snapshot with `truncated: true`; never exceed caps; never OOM on huge trees.
- Unreadable single file (permissions, broken symlink): skip entry, continue scan, record `skipped: u32` count; never abort whole scan.
- Non-UTF8 paths: lossy label for display only; hashing/lookup use raw bytes; scan continues.
- Secret safety: file contents hashed, never logged; symbol names only; full file bodies never retained. No SQLite/OpenCode DB writes (read-only walk + in-memory snapshot).

## Resource bounds

- Byte cap: `max_bytes` bounds summed `size` admitted; file body reads chunked (64 KiB), hashed streaming; bodies dropped after hash (no unbounded retained output).
- Count caps: `max_files`, `max_symbols` bound vectors; no unbounded queue of pending paths (bounded work stack, depth-first, cap-checked before push).
- Owner/cancel path: `build_snapshot_cancel(root, cfg, cancel: &AtomicBool) -> Result<...>`; on cancel returns `Err(IndexError::Cancelled)` promptly (checks flag per directory); caller owns snapshot lifetime; no detached task, no background thread.
- Disabled path: zero filesystem I/O, zero allocation beyond empty vecs; no watcher/thread started.

## Suggested module boundary

- Proposed owner: `crates/index/src/lib.rs` (new crate `opencode-rk-index`); fallback `crates/foundation/src/index.rs` if workspace resists a new crate.
- Integrator wires `pub mod index;` / re-export fragment per PLAN.md section 5; worker ships additive fragment only, never edits shared `lib.rs`, `Cargo.toml`, schemas, migrations.
- Feature flag (REQ-032 pattern): `Features::codebase_index: bool = true`; gate scan/watcher construction on flag; off means `disabled()` path.

## Frozen test obligations (exact asserts)

- RW-IDX-T01 full scan happy path: fixture tree (10 files, 3 with symbols): `assert_eq!(snap.files.len(), 10)`, `assert!(snap.symbols.len() >= 5)`, `assert!(!snap.truncated)`, `lookup_symbol(snap,"MyStruct").len() == 1`, file hashes equal direct hash of bytes.
- RW-IDX-T02 determinism + lookup order: two `build_snapshot` runs same fixture: `assert_eq!(snap1.files, snap2.files)`, `assert_eq!(snap1.symbols, snap2.symbols)`; `lookup_symbol` results sorted by (file, start_line).
- RW-IDX-T03 caps truncate: `max_files=3` (or tiny `max_bytes`): `assert!(snap.truncated)`, `assert!(snap.files.len() <= 3)`, `assert!(total_bytes(snap) <= max_bytes)`, scan of 10k-file generated fixture completes without OOM.
- RW-IDX-T04 failure states: missing root => `assert_eq!(err, RootUnreadable)`; unreadable file skipped with `skipped >= 1` and other files present; cancel flag pre-set => `assert_eq!(err, Cancelled)` and scan returns promptly (<1s on large fixture).
- RW-IDX-T05 opt-out zero cost + safety: `disabled()` on missing root => `Ok(empty)` with no I/O error and `files.is_empty()`; no test writes outside disposable fixture dir (assert DB/fixture-outside untouched); captured logs contain zero file-body bytes and zero `skipped` path contents beyond basename.

## TDD steps

1. Inspect PLAN.md 5-6, docs/TDD.md 2-5, TOOL-014/BASE-006 cards; cite commit + path:line.
2. Define contract/failures/bounds (above); open discovery proposal for gaps.
3. Author RED: 5 tests compile, fail on missing `index::build_snapshot/lookup_*`.
4. Freeze test hash + command manifest; controller verifies on disk.
5. Implement minimum native Rust; GREEN; refactor; rerun full suite.
6. Regressions: non-UTF8 paths, broken symlinks, cancel mid-scan, disabled no-I/O.
7. Submit evidence + patch, never acceptance.

## Verification

- `cargo test -p opencode-rk-index` passes RW-IDX-T01..T05 on frozen hash.
- `cargo check --workspace` clean; `cargo fmt --check` clean.
- Disabled config: test proves no filesystem I/O (missing root still Ok) and no watcher/thread.
