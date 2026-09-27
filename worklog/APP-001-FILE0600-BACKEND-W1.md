# APP-001-FILE0600-BACKEND-W1 worklog

## Claim
Task: APP-001-FILE0600-BACKEND-W1 - Implement atomic File0600 backend for credential storage
Owned file: crates/providers/src/auth_store.rs
Claimed by: ses_vyce_deepseek_v4_flash

## Source evidence
- Base rev: 6f3333f (task card commits via orchestrator reclaim)
- tasks/PROV-022.md defines the storage planning boundary
- crates/providers/src/auth_store.rs currently only provides planning functions
- rustix 1.1.4 with fs, process features available in Cargo.toml
- crates/tools/src/shell_tool.rs uses rustix::process::{kill_process_group, Pid, Signal} as reference

## Target boundary
- Implement atomic file write with O_EXCL temp file creation
- Symlink detection and rejection
- Directory mode 0700
- File mode 0600
- Parent directory fsync
- Atomic rename without following symlinks

## Tests to run
- `cargo test -p opencode-rk-providers --test prov_022_auth_store`
- `cargo check --workspace`

## Decisions
- Will implement in auth_store.rs by extending existing Planner API
- Use rustix for safe atomic operations
- Keep existing planning functions intact, add execution functions

## Remaining unknowns
- Need to verify rustix API for:
  - creat() with O_EXCL
  - fchmod()
  - fstat() for symlink detection
  - fdatasync()/fsync()
  - renameat() without follow