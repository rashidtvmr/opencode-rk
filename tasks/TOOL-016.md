# TOOL-016

Status: NOT STARTED. Kind: product. Runtime optional: True.
Mandatory for full declared release: yes.
Requirements: REQ-039 (proposed, native codebase index/snapshot, local-only).
Dependencies: none.
Test obligations: TOOL-016-T01, TOOL-016-T02, TOOL-016-T03, TOOL-016-T04, TOOL-016-T05.

## User-observable outcome

A bounded, local-only codebase snapshot for one workspace root: file list
(path, size, content hash) plus a symbol table (name, kind, file, line span)
built by a single scan. Serves file/symbol lookup without re-walking the tree.
Default-on with opt-out flag; zero hidden cost when off. No network, no DB
mutation, no secret logging.

## Source evidence

- docs/proposals/RW-SLICE-01-index-spec.md: index/snapshot spec, contract, bounds,
  RW-IDX-T01..T05 definitions mirrored below as TOOL-016-T01..T05.
- PLAN.md sections 5-6: slice independence rules, mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash, GREEN minimum.
- tasks/TOOL-014.md: task-card model (Status/Kind/contract/test obligations).
- tasks/BASE-006.md: REQ-032 gating precedent (default-off flags, gate before init);
  here default-on index with opt-out flag, same zero-cost-when-off proof.
- crates/foundation/src/config.rs:169-223 (`Feature`, `Feature::ALL`, `Features`
  default-off flags, `enabled`/`require` gating helpers); new `codebase_index`
  flag follows the REQ-032 pattern (default-on exception recorded in contract).
- Classification: new user requirement (REQ-039 proposed), deliberate
  resource-bounded deviation (caps + truncation instead of full-tree retention;
  bodies hashed then dropped).

## Observable contract

- `IndexConfig { enabled: bool, max_files: usize, max_bytes: u64, max_symbols: usize }`:
  `Default` is `enabled: true, max_files: 20000, max_bytes: 200_000_000,
  max_symbols: 100000`. `disabled()` scans nothing.
- `FileEntry { path: RelPath, size: u64, hash: blake3/sha256 hex }`;
  `Symbol { name, kind: Fn|Struct|Enum|Trait|Mod|Const, file: RelPath,
  start_line: u32, end_line: u32 }`.
- `Snapshot { root, files: Vec<FileEntry>, symbols: Vec<Symbol>, scanned_at_ms: u64,
  truncated: bool }`.
- `build_snapshot(root, cfg) -> Result<Snapshot, IndexError>`;
  `lookup_file(snap, path) -> Option<&FileEntry>`;
  `lookup_symbol(snap, name) -> Vec<&Symbol>` (exact match, sorted by file then line).
- Deterministic: same tree bytes + cfg => identical file/symbol order (sorted paths);
  no wall-clock in ordering; `scanned_at_ms` informational only.
- Disabled (`enabled: false`): returns empty snapshot
  (`files=[], symbols=[], truncated=false`) without touching the filesystem.
- Suggested module boundary: `crates/index/src/lib.rs` (new crate
  `opencode-rk-index`); fallback `crates/foundation/src/index.rs` if workspace
  resists a new crate. Worker ships additive fragment only, never edits shared
  `lib.rs`, `Cargo.toml`, schemas, migrations.
- Feature flag (REQ-032 pattern): `Features::codebase_index: bool = true`;
  gate scan/watcher construction on flag; off means `disabled()` path.

## Failure states

- Missing/unreadable root: `Err(IndexError::RootUnreadable)`; no partial snapshot returned.
- Over caps (`max_files`/`max_bytes`/`max_symbols` exceeded): stop scan, return partial
  snapshot with `truncated: true`; never exceed caps; never OOM on huge trees.
- Unreadable single file (permissions, broken symlink): skip entry, continue scan,
  record `skipped: u32` count; never abort whole scan.
- Non-UTF8 paths: lossy label for display only; hashing/lookup use raw bytes;
  scan continues.
- Secret safety: file contents hashed, never logged; symbol names only; full file
  bodies never retained. No SQLite/OpenCode DB writes (read-only walk + in-memory
  snapshot). No changes to the user's existing OpenCode database; tests use
  disposable fixture dirs only.

## Resource bounds

- Byte cap: `max_bytes` bounds summed `size` admitted; file body reads chunked
  (64 KiB), hashed streaming; bodies dropped after hash (no unbounded retained output).
- Count caps: `max_files`, `max_symbols` bound vectors; no unbounded queue of pending
  paths (bounded work stack, depth-first, cap-checked before push).
- Owner/cancel path: `build_snapshot_cancel(root, cfg, cancel: &AtomicBool)`; on cancel
  returns `Err(IndexError::Cancelled)` promptly (checks flag per directory); caller owns
  snapshot lifetime; no detached task, no background thread.
- Disabled path: zero filesystem I/O, zero allocation beyond empty vecs; no
  watcher/thread started. Zero hidden cost when off.

## Test obligations (frozen)

- TOOL-016-T01 (full scan happy path, mirrors RW-IDX-T01): fixture tree (10 files,
  3 with symbols): `assert_eq!(snap.files.len(), 10)`,
  `assert!(snap.symbols.len() >= 5)`, `assert!(!snap.truncated)`,
  `lookup_symbol(snap,"MyStruct").len() == 1`, file hashes equal direct hash of bytes.
- TOOL-016-T02 (determinism + lookup order, mirrors RW-IDX-T02): two `build_snapshot`
  runs same fixture: `assert_eq!(snap1.files, snap2.files)`,
  `assert_eq!(snap1.symbols, snap2.symbols)`; `lookup_symbol` results sorted by
  (file, start_line).
- TOOL-016-T03 (caps truncate, mirrors RW-IDX-T03): `max_files=3` (or tiny `max_bytes`):
  `assert!(snap.truncated)`, `assert!(snap.files.len() <= 3)`,
  `assert!(total_bytes(snap) <= max_bytes)`, scan of 10k-file generated fixture
  completes without OOM.
- TOOL-016-T04 (failure states, mirrors RW-IDX-T04): missing root =>
  `assert_eq!(err, RootUnreadable)`; unreadable file skipped with `skipped >= 1` and
  other files present; cancel flag pre-set => `assert_eq!(err, Cancelled)` and scan
  returns promptly (<1s on large fixture).
- TOOL-016-T05 (opt-out zero cost + safety, mirrors RW-IDX-T05): `disabled()` on missing
  root => `Ok(empty)` with no I/O error and `files.is_empty()`; no test writes outside
  disposable fixture dir (assert DB/fixture-outside untouched); captured logs contain
  zero file-body bytes and zero `skipped` path contents beyond basename.

## TDD steps

1. Inspect: PLAN.md 5-6, TDD.md 2-5, TOOL-014.md, RW-SLICE-01-index-spec.md
   (done, see evidence).
2. Contract: defined above.
3. Author tests TOOL-016-T01..T05; establish compiling RED (fail: no index module).
4. Freeze test hash + command manifest.
5. Implement minimum native Rust index/snapshot.
6. GREEN, refactor, rerun; negative tests (non-UTF8 paths, broken symlinks,
   cancel mid-scan, disabled no-I/O).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/TOOL-016.md
cargo test -p opencode-rk-index
cargo check --workspace
```
