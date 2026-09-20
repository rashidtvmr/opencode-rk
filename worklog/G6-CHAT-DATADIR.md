# G6-CHAT-DATADIR

Claim: G6-CHAT-DATADIR, session ses_f425545f6ffe4N8Iq6BNvdwYSC.
Task (orchestrator brief): forward data-dir in chat.rs spawn_daemon so spawned serve uses same HOME/descriptor.

## Source evidence
- `crates/cli/src/chat.rs:48` `run(data_dir: &Path)` already has dir; `chat.rs:53` called `spawn_daemon(&addr)` ignoring it (gap: only `--listen` forwarded).
- `crates/cli/src/chat.rs:61-63` `reuse_credential(data_dir, ...)` reads descriptor via `server_daemon::read_backend_descriptor(data_dir)`.
- `crates/cli/src/main.rs:62-63` `--data-dir` global (`env = OPENCODE_RK_HOME`); `main.rs:266-268` `Serve` resolves `data = resolve_data_dir(cli.data_dir)`; `main.rs:641,674` serve publishes descriptor with `&data`.
- Descriptor mismatch scenario: chat with `--data-dir /custom` spawns serve without it → serve writes descriptor under default HOME → chat reads `/custom` → stale/missing descriptor, credential reuse fails.

## Target boundary
Own only `crates/cli/src/chat.rs`. No main.rs/tui_entry.rs/frozen-test touches.

## Change
- `run`: `spawn_daemon(&addr)` → `spawn_daemon(&addr, data_dir)`.
- `spawn_daemon(addr, data_dir)`: args `--data-dir <dir> serve --listen <addr>`; `env_remove("OPENCODE_RK_HOME")` so explicit dir stays authoritative (clap env on the global flag would otherwise override ordering; clearing cannot regress default path which resolves via HOME anyway).

## Tests
- No new tests: private spawn path needs a live process; contract verified by `cargo check -p opencode-rk-cli --bins` (compiles call site + fn).
