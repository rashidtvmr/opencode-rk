# APP-012-ROOT-ANCHOR-RED-W1

## Claim

- Task: APP-012-ROOT-ANCHOR-RED-W1 (TEST-AUTHOR, RED only, no implementation)
- Session: ses_f1ce2d44dffevpTWDqxvnObCFW
- Worktree: /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/red-app012-root-anchor-w1
- Branch: red/APP-012-ROOT-ANCHOR-W1
- Base: 68bd3510ace6c419df6f5ebcb58250d1c8e5df44 (seam commit, pushed lane/APP-012-ROOT-ANCHOR-SEAM-W1 head)
- Owned file: `crates/tools/src/file_ops.rs` (test-only addition in source-local `mod tests`)

## Source evidence

- `file_ops.rs:437-455`: `open_write_root_with_observer(Option<&mut dyn FnMut(&OwnedFd)->Result<(),ToolError>>)` opens `/` via `openat(rustix_fs::CWD, "/", RDONLY|DIRECTORY|NOFOLLOW|CLOEXEC)`; observer error propagates through `obs(&fd)?` before the fd is returned.
- `file_ops.rs:459-461`: production `open_write_root()` = wrapper with `None`.
- `file_ops.rs:389`: writer resolves approved project via `std::env::current_dir()`.
- `crates/server/tests/live_file_tool_symlink_escape.rs:74-89`: existing serialized `CwdGuard` RAII pattern (set cwd on construction, restore on drop), relies on `--test-threads=1`; replicated test-side, no globals.
- Seam lane precedent: `worklog/APP-012-ROOT-ANCHOR-SEAM-W1.md` "Remaining" names exactly this comparison.

## Contract under test

`open_write_root_with_observer` must anchor the root fd on the approved project
directory (current working directory), not the filesystem `/`. Observer fstats
the opened fd and the project dir (identity derived inside the callback) and
returns `Err` on device/inode mismatch. Required GREEN: function succeeds AND
opened (dev, ino) == approved project dir (dev, ino).

## RED expectation

Current code opens `/`: on macOS `/` is System-volume inode 2 vs Data-volume
tempdir; dev/inode differ deterministically. Observer returns
`ToolError::FileError` with an explicit mismatch description;
`open_write_root_with_observer` propagates it; the test `expect` fails on the
function error (genuine observer-driven failure, not a setup panic). Setup
(tempdir + set_current_dir) is guarded separately so any setup defect is
distinguishable from the assertion failure.

## Decisions

- No production/seam bytes changed; no manifest change; no new deps.
- No unsafe, no sleep, no mutable globals; `current_dir` derived inside the
  observer callback per card.
- CwdGuard mirrored inside the marked block to avoid touching existing tests.
- Serialized with `--test-threads=1` (existing lane convention), single owned
  test only mutates process cwd.

## Status

- BLOCKED pending implementation: root selection at
  `file_ops.rs:442-447` must anchor the approved project directory instead of `/`.
