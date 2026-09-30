# STORAGE-LIB-CLIPPY candidate

- **Base:** `de7e05fefba5374246078674432dcba07b0b5ff2`
- **Candidate branch:** `v2/storage-lint`
- **Status:** CANDIDATE only; no Cargo/runtime verification was run by this source-only worker.
- **Gate evidence:** Parent's fresh `cargo clippy --offline --locked --workspace --all-targets --all-features -- -D warnings` run on the base exited 101. Full log: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-workspace-gates-de7e05f-nspragpt/clippy.log`; command/environment: sibling `commands.json`. Storage diagnostics were 30: unused `actual`; four `too_many_arguments`; three unnecessary casts; two manual state ranges; three tuple-complexity findings; needless query-map `Ok(...?)`; six needless borrows; and nine needless string `as_bytes` calls plus two `io_other_error` diagnostics (the count categories reflect the actual retained log; exact output is authoritative).
- **Changes:** lint-equivalent conversions/borrows/ranges, internal row aliases, retained checked `byte_len` conversion renamed `_actual` at the otherwise unused site, equivalent direct row mapping, and `Error::other` with unchanged messages/kind.
- **Narrow style exceptions:** `ApprovalsV2::request`, `ExecV2::start_execution`, `ExecV2::plan_tool`, and `Storage::create_artifact` retain the existing explicit atomic operation arguments and public signatures. Their call sites are established contracts; splitting fields into DTOs just to meet a style threshold would churn frozen callers without improving correctness or safety. Each receives a method-local `#[expect(clippy::too_many_arguments, reason = ...)]`; this is checked for staleness and is not a correctness-lint suppression.
- **Preservation boundary:** no SQL text, schema/default, limits, errors, ordering, transaction behavior, expiry behavior, public signatures, or test-body assertions are intentionally changed. No test-body edits were made.
- **Verification performed:** source review and `git diff --check` only. No Cargo, tests, build, or runtime commands, as the independent test owner owns the heavy slot. Independent verifier must run storage library Clippy and full storage tests including typed admission/durability and expiry/retention controls; repeat required gates on integrated SHA before acceptance.
- **Paths granted:** `crates/storage/src/lib.rs`, `approvals_v2.rs`, `content_addr_v2.rs`, `execution_v2.rs`, `fork_v2.rs`, `retention_v2.rs`, `snapshot_v2.rs`, `writer_v2.rs`, and this worklog.

## Independent verification and exact integrated acceptance

Independent verifier `ses_f0b716ce4ffeEybDNTvAQMWRNw` confirmed candidate
`d099194edc1afb87adb1f1e10eab80e969aab2aa` was PREVERIFIED after checking
actual source, protected tests, installed artifacts and successful command logs.
Two earlier verification-environment failures are preserved: an isolated HOME
without the approved offline Cargo cache could not resolve `chrono`, and the
first full storage command lacked the required installed `OC2_TEST_BINARY`.
Neither is presented as a product/test pass.

The controller rebuilt, archived and actually installed the exact candidate,
then supplied that attested binary to the full storage tests. Candidate commands
passed with the literal approved Cargo/Rustup caches and disposable HOME/XDG
state. The same process was repeated after integration on exact SHA
**`ed6e78736ae8aad091ba2f0ba182754cbbbe155c`**:

```text
cargo build --offline --locked --release -p opencode-rk-cli --bin oc2 --features native
  exit 0; real archive installer exit 0
native_stream_tool.py / native_provider_setup.py against the installed release
  exit 0 / exit 0; four / two actual provider requests
cargo clippy --offline --locked -p opencode-rk-storage --lib -- -D warnings
  exit 0
cargo test --offline --locked -p opencode-rk-storage --all-targets -- --test-threads=1
  212 passed, 0 failed, 0 ignored (122 library + 90 integration/component)
cargo test --offline --locked -p opencode-rk-security --test app012_approval_expiry_red -- --test-threads=1
  1 passed, 0 failed, exit 0
```

All commands were serial, with two Cargo jobs and one test thread. Full storage
coverage includes real installed-daemon typed-history restart and transactional
fault controls, retained approval-expiry/retention contracts and typed round
admission/durability controls. No SQL strings, schema/defaults, public call
signatures, byte/error checks or test bodies changed. The four documented
method-specific `too_many_arguments` expectations cover existing auditable
atomic-operation APIs; no correctness warning or semantic assertion is disabled.

Actual commands/environments/source/test/artifact/log hashes are retained under
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/`:

- Candidate release: `v2-native-storage-preverify-d099194-5c3q1j1j`.
- Candidate verification: `v2-storage-preverify-d099194-kn2hzf0e`.
- Exact integrated release: `v2-native-storage-integrated-ed6e787-i82gzid6`.
- Exact integrated verification: `v2-storage-integrated-ed6e787-_y6n1zov`.

State: **ACCEPTED for storage-library diagnostic maintenance on exact integrated
`ed6e787`**. Workspace-wide formatting, all-target Clippy and full tests still
require their separate gates.
