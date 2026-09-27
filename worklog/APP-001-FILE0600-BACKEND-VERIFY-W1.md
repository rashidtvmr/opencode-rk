# APP-001-FILE0600-BACKEND-VERIFY-W1

## Claim

- Task: independent backend verification at exact candidate `d137a091dbc5d1c2e9245c7df31c83a5fdfc30fd`.
- Branch: `verify/APP-001-FILE0600-D137-W1`.
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/verify-app001-file0600-d137-w1`.
- No product edits. Only this receipt and this task's claims row are changed.

## Source review

- `crates/providers/src/auth_store.rs:29-40`: bounded path/provider/secret limits; modes `0600`/`0700`.
- `crates/providers/src/auth_store.rs:104-145`: fixed redacted `StoreError` codes; no paths, OS errors, or secret material.
- `crates/providers/src/auth_store.rs:349-365`: exact lexical destination boundary; absolute, parent, empty, non-UTF-8, separators, and overlong paths rejected.
- `crates/providers/src/auth_store.rs:575-688`: directory-relative rustix traversal; `O_NOFOLLOW`, ownership, type, mode, sticky-directory checks; only exact macOS `/var -> /private/var` alias accepted.
- `crates/providers/src/auth_store.rs:694-790`: exclusive `0600` temp, bounded stale sweep, full write/flush/fsync, fd revalidation, `linkat` no-replace commit, temp unlink.
- `crates/providers/src/auth_store.rs:792-879`: atomic write/remove/load operations; parent fsync; load cap `MAX_LOAD_BYTES`.
- `crates/providers/src/auth_store.rs:900-1110`: 12 real backend unit tests covering modes, atomicity, symlinks, stale temp, oversize, removal, load.
- Commit scope: `Cargo.lock`, `crates/providers/src/auth_store.rs`, backend implementer claim/worklog only. No CLI caller wiring in this candidate.
- No debug/probe/stub markers (`dbg!`, `println!`, `eprintln!`, `todo!`, `unimplemented!`, `panic!`, `unreachable!`) in backend source.
- Frozen CLI test SHA256 unchanged: `c99b15bca871f1258f4ae3c886a57cc108a695476083da057caeae7cec0d3e40`.

## Verification

- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo check -p opencode-rk-providers`: PASS.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-providers --lib auth_store -- --test-threads=1`: PASS, 12/12.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-providers --test prov_022_auth_store -- --test-threads=1`: PASS, 5/5.
- `cargo build -p opencode-rk-cli --bin oc2 --no-default-features`: PASS, used only to exercise frozen CLI test.
- `rustc --edition 2021 --test crates/cli/tests/installed_setup_file0600.rs -o target/installed_setup_file0600`: PASS compile.
- `OC2_E2E_BIN=target/debug/oc2 RUST_TEST_THREADS=1 target/installed_setup_file0600 --test-threads=1`: EXPECTED caller-unwired failure, 2/5 pass, 3/5 fail. `t01`/`t02` missing `<data_dir>/credentials`; `t04` setup returns 0 and does not reject pre-created symlink. `t03`/`t05` pass.

## Boundary

Backend candidate is GREEN for provider tests. CLI setup/restart persistence caller remains unwired at this commit; no CLI implementation was added or altered. Frozen CLI RED is preserved as the exact reproduction above.
