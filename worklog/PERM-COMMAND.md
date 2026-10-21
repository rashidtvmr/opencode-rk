# PERM-COMMAND — wire `/permission` into chat REPL

User-directed repair. No ledger claim (explicitly out of scope).

## Claim
Interactive chat REPL must really change which tools the daemon may run.

## Source evidence
- `crates/cli/src/permission_command.rs:78` `parse`, `:163` `apply`, `:223` `save`,
  `:201` `load`, `:250` `usage` — module already complete, declared
  `mod permission_command;` at `crates/cli/src/main.rs:48`.
- `crates/cli/src/chat.rs:447` `loop_until_exit` dispatch (unknown `/` arm last).
- `crates/cli/src/chat.rs:542` `spawn_daemon` — child inherits environment.
- `crates/server/src/lib.rs:839-842` turn-tool allowlist source is
  `OPENCODE_RK_TURN_TOOLS` (comma-separated, deny-by-default).

## Change (owned file only: crates/cli/src/chat.rs)
- import `permission_command::{self, PermissionAction, PermissionStore}`.
- `Chat` gains `data_dir: PathBuf`, `permissions: PermissionStore`; `run()` loads
  the store from `data_dir`.
- `loop_until_exit`: `/permission...` arm before the unknown-`/` arm.
- `permission_command()`: parse → help/apply → print → `save` → trailing note.
- `help()` lists the four `/permission` forms.
- `permission_turn_tools(&PermissionStore) -> Option<String>` (pure, sorted join,
  `None` when empty) used by `spawn_daemon` to set `OPENCODE_RK_TURN_TOOLS`.
  Empty set sets nothing; inherited env untouched (out of scope).
- Tests appended to the existing `#[cfg(test)] mod tests` (chat.rs already had
  one; no duplicate module).

## Tests
Pure only: empty→None, allow bash+write→"bash,write", deny wins, parse+apply,
`/permission allow *` error text. No network, no spawn.

## Remaining unknowns
- Already-running daemon keeps its old env; note says so to the user.
