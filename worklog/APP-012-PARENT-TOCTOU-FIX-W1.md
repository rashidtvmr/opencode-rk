# APP-012-PARENT-TOCTOU-FIX-W1

## Claim

- Task: APP-012-PARENT-TOCTOU-FIX-W1
- Prior worker: ses_f1d6b37f7ffeuBh9LSLRWSmNfk (stopped before commit)
- Landing/verification session: ses_f1d3ebf66ffeTmRAQwEI4ZUQ7D
- Base: `6aee5973bd1ae700d4827cc9c0b8ff8e716ab5d5`
- Owned product file: `crates/tools/src/file_ops.rs`

## Source evidence

- `crates/tools/src/file_ops.rs:263-323`: prior write path checked names by path,
  created parents with `create_dir_all`, then path-opened the leaf. Leaf-only
  `O_NOFOLLOW` did not protect intermediate parents.
- `worklog/APP-012-PARENT-TOCTOU-RED-W1`: frozen parent swap hook/test. Hook SHA
  `c792c88f93de005e7368c1697f806955d2ae371c2ea4b96b5077f1b1be71c906`; test SHA
  `48f866866b12685df3b99df99e99e148ad58a3cd3fc3b72a574d830631456c12`.
- `Cargo.toml:34`, `crates/tools/Cargo.toml:10`: rustix 1.1.4 with `fs` feature
  is available on this base.

## Contract

On Linux/macOS, normalize a relative path against `current_dir`, narrowly map
macOS `/var` and `/tmp` aliases, reject traversal and malformed/bounded paths,
open `/` as a trusted dirfd, and traverse/create each parent with descriptor-
relative `openat`/`mkdirat`, `O_NOFOLLOW|O_DIRECTORY`. Keep parent descriptors
pinned. Run the frozen hook after traversal and descriptor-relative leaf checks;
re-traverse and compare the pinned parent after the hook. Open the leaf only with
descriptor-relative `openat`, `O_NOFOLLOW`, requested append/truncate flags, and
mode `0600`. Unsupported targets deny without filesystem I/O.

## Verification plan

- RED baseline: frozen parent-swap test must modify outside sentinel before fix.
- GREEN: focused file_ops tests, frozen server dispatch/in-root/symlink tests,
  frozen block hashes unchanged.
- Serial settings: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`.

## Remaining

- Parent APP-012 remains a candidate pending independent security review and
  integrated verifier acceptance.

## Verification receipts

- `CARGO_BUILD_JOBS=1 cargo check -p opencode-rk-tools`: passed.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --lib file_ops -- --test-threads=1`: 13 passed.
- Frozen parent swap test: `write_file_parent_toctou_symlink_swap_must_not_escape`: passed.
- Frozen server tests: `live_file_tool_dispatch`, `live_file_tool_allowed_inroot`,
  `live_file_tool_symlink_escape`: each passed, serially.
- Frozen test suffix byte comparison against base: unchanged; SHA256
  `63fbc93f9a39ef2bdd780af3dcd8242d4cef667ec2b4ae330a292377330aecdd`.
- `git diff --check`: passed.

## Landing continuation

- Reclaimed the completed-but-uncommitted lane claim after the orchestrator
  confirmed the prior worker stopped; the original completion note was not
  treated as test evidence.
- Review found hard-coded errno numbers and stale seam-test expectations in the
  uncommitted patch. Replaced ELOOP numeric check with rustix's `Errno::LOOP`,
  restored the frozen symlink precheck before the test hook, corrected hook docs
  and stale test error expectation. No tests or seams changed; test suffix remains
  byte-identical to base (`63fbc93f9a39ef2bdd780af3dcd8242d4cef667ec2b4ae330a292377330aecdd`).
- Current implementation bounds paths to 4096 bytes and 256 components; parent
  descriptors are held only for the duration of one write. Unsupported targets
  deny writes. `path_has_symlink_component` stays an early compatibility check;
  descriptor-relative no-follow traversal/revalidation enforces the race boundary.
- Final serial verification on macOS: `CARGO_BUILD_JOBS=1 cargo check -p
  opencode-rk-tools`; focused frozen `write_file_parent_toctou_symlink_swap_must_not_escape`
  1/1; all `file_ops` 13/13; server `live_file_tool_dispatch`,
  `live_file_tool_allowed_inroot`, `live_file_tool_symlink_escape` 1/1 each.
  `git diff --check` passed. Frozen in-file test suffix unchanged. No diagnostics
  added. Validation uses one Cargo job and one test thread; host reports 24 GiB
  physical RAM, with no cargo/rustc process competing at check start.
- Parent APP-012 is not released or accepted by this repair slice; independent
  integration/security verification remains required.
- Landing owner: `ses_f1d34837bffefFx9AcPpuZuOlR`; source review verified the
  parent dirfd chain is no-follow, held through leaf open, then inode/device
  re-traversal after hook denies the tested swap. No diagnostics, stubs, test
  edits or extra paths found. Linux/macOS only; other targets deny writes.
- Final commands: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo check -p
  opencode-rk-tools`; `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p
  opencode-rk-tools --lib file_ops::tests::write_file_parent_toctou_symlink_swap_must_not_escape
  -- --test-threads=1` (1/1); `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo
  test -p opencode-rk-tools --lib file_ops -- --test-threads=1` (13/13);
  server tests `live_file_tool_dispatch`, `live_file_tool_allowed_inroot`,
  `live_file_tool_symlink_escape` each 1/1, serial jobs/threads=1. All passed.
- Verification continued against exact frozen bytes: file_ops test suffix
  SHA256 `63fbc93f9a39ef2bdd780af3dcd8242d4cef667ec2b4ae330a292377330aecdd`;
  server test hashes remain `0366416c4863940221c74aac08deeeba3c9404cd92ff18c9411d2cab5b570829`,
  `6e3841384c7a477ed226f0bf5e28f5b40e06fe683fc9683059e699fd3768aa12`,
  `fe8d24946c8ce98aae095b6a2f92b7f3c19cc32532ae6263f9898d8a5699753c`.
- Convergence gate remains blocked by pre-existing off-plan ledger rows (64
  findings) and unrelated parent acceptance notes; no convergence claims made.
