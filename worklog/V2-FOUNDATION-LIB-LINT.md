# V2-FOUNDATION-LIB-MECHANICAL

- Package: SOURCE-ONLY FOUNDATION-LIB-MECHANICAL
- Base: `358690524cb82c5f9d49ce3043868195686d23b6`
- Scope: `crates/foundation/src/repo_cache_store.rs`, `crates/foundation/src/lib.rs` only for product code.
- Additional path: this worklog only.
- Candidate status: candidate pending parent independent verification and integration.

## Mechanical equivalence

- Renamed the stored-but-never-read `CacheStore.root` field to `_root`; the
  constructor still stores the exact `root.to_path_buf()` allocation and all
  root validation and `slots` path behavior are unchanged.
- Replaced `if let Err(_) = create_dir_all(...)` with
  `create_dir_all(...).is_err()`; the same error-to-`RootUnreadable` mapping is
  preserved.
- Replaced `checked_add(bytes).map_or(true, |next| next > self.limit)` with
  `checked_add(bytes).is_none_or(|next| next > self.limit)`; overflow remains
  fail-closed and the byte-budget boundary is unchanged.
- No public API, schema, test, manifest, lockfile, security, or resource-bound
  behavior was intentionally changed.

## Verification

- `rustfmt --edition 2021 crates/foundation/src/repo_cache_store.rs crates/foundation/src/lib.rs`: PASS.
- `git diff --check`: PASS.
- Cargo.lock SHA-256 remained `63ef5299dd93286950af00388796375b06aefc5a4a3eedfa38361954fefb6f03`.
- Protected test region in `crates/foundation/src/lib.rs` (from `#[cfg(test)]`)
  SHA-256: `f73006758533940779547d42927e01315312677ff1e767c350a339615cdc0433`.
- `repo_cache_store.rs` has no inline test region. No test files or test
  manifests were modified.
- Source verification confirmed only the requested lint transformations and
  `_root` rename in the two owned source files.
- Per package instruction, Cargo, Clippy, tests, builds, runtime, network, and
  child-process gates were not run; parent owns those heavy gates.

## Path scope

`git diff --name-only` before commit is exactly:

```text
crates/foundation/src/lib.rs
crates/foundation/src/repo_cache_store.rs
worklog/V2-FOUNDATION-LIB-LINT.md
```
