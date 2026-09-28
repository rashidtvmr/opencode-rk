# APP-012-ROOT-ANCHOR-SEAM-W1

## Claim

- Task: APP-012-ROOT-ANCHOR-SEAM-W1
- Session: ses_f1cfa0acd3ffe2cOcevISAKEx8r
- Branch: lane/APP-012-ROOT-ANCHOR-SEAM-W1
- Base: 78b744ddeaa1b79d3cebddc4bbe80c8217bec8dd
- Owned product file: `crates/tools/src/file_ops.rs`

## Source evidence

- `crates/tools/src/file_ops.rs:343`: writes obtain one root descriptor through `open_write_root()` before descriptor-relative traversal.
- `crates/tools/src/file_ops.rs:430-459`: root opens `/` with `RDONLY | DIRECTORY | NOFOLLOW | CLOEXEC`; production wrapper previously had no observer.
- `crates/tools/src/file_ops.rs:354-357`: existing pre-open hook remains after initial parent traversal and before recheck/leaf open.
- `crates/tools/src/file_ops.rs:360-365`: existing parent descriptor re-traversal and identity comparison remain unchanged.
- Existing parent-swap and server test hashes retained from prior lane evidence: `63fbc93f9a39ef2bdd780af3dcd8242d4cef667ec2b4ae330a292377330aecdd`, `0366416c4863940221c74aac08deeeba3c9404cd92ff18c9411d2cab5b570829`, `6e3841384c7a477ed226f0bf5e28f5b40e06fe683fc9683059e699fd3768aa12`, `fe8d24946c8ce98aae095b6a2f92b7f3c19cc32532ae6263f9898d8a5699753c`.

## Contract

Internal observer seam only. `open_write_root_with_observer` accepts an optional borrowed-FD observer. Observer runs once after `/` is opened and before the FD is returned. `open_write_root()` calls it with `None`, retaining the existing `/` open and descriptor-relative behavior. No project-root selection or security fix claimed. No global state, unsafe, sleep, or detached work.

## Implementation

- Added `open_write_root_with_observer(Option<&mut dyn FnMut(&OwnedFd) -> Result<(), ToolError>>)`.
- Kept `open_write_root()` as the production `None` wrapper.
- Added one source-local test proving exactly-one invocation, observed FD identity equals returned FD identity, and `None` wrapper identity matches explicit `None`.
- Existing pre-open hook, parent recheck, parent-swap defense, and frozen server tests untouched.

## Verification

- `env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --lib file_ops -- --test-threads=1`: GREEN, 14/14.
- `env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --lib file_ops -- --list`: GREEN, 14 discovered.
- New test: `file_ops::tests::open_write_root_observes_actual_fd_once_and_none_is_unchanged`.
- Prior frozen hash evidence retained; no frozen test edits.
- `python3 tools/convergence_gate.py`: blocked by 64 pre-existing ledger findings, unrelated to this lane.

## Remaining

- Later lane must compare root identity to an approved disposable project root and implement root selection if required. This seam intentionally does not do that.
