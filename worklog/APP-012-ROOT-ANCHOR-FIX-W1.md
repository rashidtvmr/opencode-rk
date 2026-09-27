# APP-012-ROOT-ANCHOR-FIX-W1

- Claim: replace the filesystem-root descriptor anchor in `crates/tools/src/file_ops.rs`.
- Base: `8b72e84823ebb133854557e2f8b3e62de46af5f0`; frozen RED block SHA256 `c3c81f71355f6bdc315231083973a699f7b672980e1929fc70bc08e80f162cf9`.
- Ownership: `crates/tools/src/file_ops.rs`, this scratchpad, own ledger row.
- Source evidence: `FileTool` was unit-like at lines 147-176; descriptor writes opened `/` in `open_write_root_with_observer` lines 437-452; `write_file_descriptor_relative` normalized relative paths against `current_dir` then traversed from that filesystem-root FD at lines 326-382. Frozen observer test requires the opened root identity to equal `current_dir`.
- Contract: `FileTool::with_project_root` stores one lexical absolute root; rooted operations accept only descendants, reject outside paths, `..`, prefixes, and symlink components. Writes open the approved root with `NOFOLLOW|DIRECTORY`, traverse descriptor-relative components, preserve parent identity recheck, append/truncate/create, mode `0600`, and bounds. Legacy `new`/free execution use a target-parent compatibility anchor, never `/`; this path is compatibility only, not server authority.
- Tests: frozen root-anchor test; all `file_ops`; server dispatch/in-root/symlink write tests serially. Add source-local rooted success and outside/traversal denial tests without editing frozen tests.
- Implementation: `FileTool::with_project_root` captures a lexical root; rooted writes use one `NOFOLLOW|DIRECTORY` root FD and descriptor-relative traversal. Legacy writes choose nearest existing target parent, never `/`. `open_write_root_with_observer` opens `current_dir` once. Added two source-local rooted boundary tests.
- Verification: root RED GREEN; `cargo test -p opencode-rk-tools --lib file_ops -- --test-threads=1` 17/17; server `live_file_tool_dispatch`, `live_file_tool_allowed_inroot`, `live_file_tool_symlink_escape` each 1/1, serial jobs/threads=1; RED block hash `22d8e37607c4aa58b102fc83a263274cf8d547e7ebd997842a60801a3084ca82` unchanged.
- Remaining: server migration to construct rooted FileTool is next lane; this lane does not edit server.
