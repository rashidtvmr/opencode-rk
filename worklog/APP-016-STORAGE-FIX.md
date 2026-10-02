# APP-016 storage fixture correction

- Base: `6dcabf83dbfb0e254b91ba4656562d91b3210d09`.
- Scope: storage test fixture only (`crates/storage/tests/typed_tool_history_same_batch_stop.rs`).
- Corrected the database assertion to `data_root/state.db`, matching the
  canonical `Storage::open(StoragePaths::under(data_root))` implementation in
  `crates/storage/src/lib.rs` (`StoragePaths::under` joins `state.db` directly).
- Replaced default `TempDir::new()` with a disposable `/private/tmp` fixture
  using a process-id/counter prefix plus tempfile's collision-safe suffix. This
  bounds the root path without changing the 20-second deadline or semantic
  same-batch assertions; HOME, project, runtime, database, socket and logs stay
  inside the disposable fixture.
- Verification is recorded by the parent orchestrator; no product, CLI,
  native, server, dependency, or Cargo files are changed here.
