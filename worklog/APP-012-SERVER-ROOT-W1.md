# APP-012-SERVER-ROOT-W1

## Claim

- Session: `ses_f1cba404bffeU4evvbe7KFiOpP`
- Branch/base: `lane/APP-012-SERVER-ROOT-W1` from pushed `0289aefc059e9d33580ce5e78b9c960dc23238a4`
- Owned product file: `crates/server/src/lib.rs`
- Scratchpad + own ledger row only beyond owned product file.

## Source evidence

- `crates/server/src/lib.rs:903-920`: write helper parses operation, then currently calls unrooted `FileTool::new()`.
- `crates/server/src/lib.rs:1021-1035`: turn state builds the permission broker with `current_dir().unwrap_or_else(|_| PathBuf::from("/"))`, silently widening root on failure.
- `crates/server/src/lib.rs:1220-1228`: live write dispatch calls helper with broker; other tools use the existing executor path.
- `crates/tools/src/file_ops.rs:148-209`: `FileTool::with_project_root` owns a lexical root; rooted `execute_authorized` authorizes before I/O and rooted execution checks operation paths.
- `crates/security/src/lib.rs:256-288,331-338`: broker applies file path/root protections to absolute normalized paths.
- `worklog/APP-012-ROOT-ANCHOR-FIX-W1.md:9-11`: root-anchored FileTool implementation and outstanding server wiring.

## Contract

Capture one absolute `project_root` once during request setup. On lookup failure, return bounded internal API error before persisting turn input. Root both `SecurityPolicy` and `FileTool` from that capture. Resolve relative write targets against the same root before broker authorization; preserve absolute targets for broker and rooted writer. Never use `/` fallback or another server-side cwd lookup. Preserve write redaction, broker decisions, loop, cancellation, and unrelated tool dispatch.

## Tests and verification

- Frozen server targets: `live_file_tool_dispatch`, `live_file_tool_allowed_inroot`, `live_file_tool_symlink_escape`.
- Parent regressions: `opencode-rk-tools` `file_ops`; server `agent_loop_turns` serialized.
- Root RED block: extract `APP-012-ROOT-ANCHOR-RED-W1` markers from `crates/tools/src/file_ops.rs`, compare SHA256 with original worklog evidence `22d8e37607c4aa58b102fc83a263274cf8d547e7ebd997842a60801a3084ca82`; do not edit it.
- Frozen server hashes per original `worklog/APP-012-TOCTOU-SEAM-W1.md:130-133`: dispatch `0366416c4863940221c74aac08deeeba3c9404cd92ff18c9411d2cab5b570829`; inroot `6e3841384c7a477ed226f0bf5e28f5b40e06fe683fc9683059e699fd3768aa12`; symlink `fe8d24946c8ce98aae095b6a2f92b7f3c19cc32532ae6263f9898d8a5699753c`.
- `CARGO_BUILD_JOBS=1 cargo check -p opencode-rk-server`: GREEN after correcting `Path` alias collision with Axum extractor.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test live_file_tool_dispatch -- --test-threads=1`: GREEN, 1/1.
- Same serial command for `live_file_tool_allowed_inroot`, `live_file_tool_symlink_escape`: GREEN, 1/1 each.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --lib file_ops -- --test-threads=1`: GREEN.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --lib open_write_root_anchors_opened_fd_on_approved_project_dir -- --test-threads=1 --nocapture`: GREEN, 1/1.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test agent_loop_turns -- --test-threads=1`: GREEN, 1/1.
- All three frozen server test hashes match original worklog evidence exactly. Extracted frozen root RED block hash: `22d8e37607c4aa58b102fc83a263274cf8d547e7ebd997842a60801a3084ca82`, matches original worklog.
- `git diff --check`: GREEN. Exactly one `current_dir()` remains in server write path, no `/` fallback, no `FileTool::new()` write dispatch.
- `cargo fmt --check -- crates/server/src/lib.rs`: blocked by broad pre-existing formatting differences across unrelated server modules; no formatter writes performed.

## Remaining

- No known task-scope gaps. Integration and acceptance remain verifier-owned.
