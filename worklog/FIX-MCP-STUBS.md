# FIX-MCP-STUBS — worklog

Claim: FIX-MCP-STUBS, session ses_f40414a26ffeBvlJ2vsm5EL207, scratchpad worklog/FIX-MCP-STUBS.md.
Baseline: 62f7eb1. Owned file ONLY: crates/tools/src/mcp_config.rs (untracked, 172 lines, pre-existing).

## Source evidence
- `crates/tools/src/mcp_config.rs:1-172` — module exists: McpConfigBounds (Copy struct, 5 bound fields), McpConfigSchema (Serialize+Deserialize+Default, command/args/env/url), McpConfigStatus (valid/errors/config/effective + from_result), validate_url_scheme (http/https allow), load_config (command-required + count/bytes bounds + url len + scheme guard), merge_configs (user-wins-non-empty, url or-else).
- `crates/tools/tests/mcp_config.rs:1-257` — frozen test, `#[path]` include (lib.rs pub mod NOT needed).
- Test imports (`tests/mcp_config.rs:6-8`): load_config, McpConfigBounds, McpConfigSchema, McpConfigStatus, merge_configs, validate_url_scheme — ALL present in impl. `json!(cfg)` needs Serialize — present. `..Default::default()` / `McpConfigStatus::default()` need Default — present. `..DEFAULT_BOUNDS` struct-update needs Copy — present.
- Frozen defects (NOT mine to fix): `:76` + `:87` `json!` array-repeat `[expr; N]` (serde_json json! macro has no repeat-arm → compile error); `:95` + `:109` bare `HashMap` with no `use std::collections::HashMap` (E0433).

## Impl review vs each test (static)
- valid_minimal/full (`:44,:51`): serialize round-trip → Ok. OK.
- missing_command (`:58`): pushes "command: ..." error. OK.
- args count/bytes, env count/bytes, url len, ftp scheme (`:73-:139`): each bound produces Err. OK (env-bytes case collapses to 1 key but 203B > 10B → still Err).
- traversal partial/with_url (`:142,:157`): defaults preserved. OK.
- merge precedence x2 (`:169,:191`): user-wins-non-empty + project fallback. OK.
- status default/with_errors (`:210,:219`): Default invalid; from_result Err → invalid + errors. OK.
- ssrf x5 (`:230-:254`): scheme split+lowercase; http/https Ok, ftp/file/data Err. OK.

## Verification
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 cargo test -p opencode-rk-tools --test mcp_config`
  tail: `error: no rules expected 'format'` at `tests/mcp_config.rs:76:18`,
  `error: no rules expected 'long_arg'` at `:87:18`, `could not compile ... due to 2 previous errors`.
  NOTE: only the two json!-repeat errors are shown by cargo; the bare-`HashMap`
  E0433 at `:95`/`:109` is masked (macro expansion fails first) and will surface
  once the json! arms are fixed — test owner must fix all four sites together.
- Standalone smoke (byte-identical copy of `src/mcp_config.rs` + equivalent test
  bodies with pre-built `json!` values instead of the illegal repeat arms):
  `rustc --test /tmp/opencode/smoke2.rs` + run → `1 passed; 0 failed`.
  Covers: args-count, args-bytes, env-count, env-bytes, url-len, missing-command,
  partial-ok, http/https-ok, ftp/file/data-rejected, merge both directions,
  status from_result + default. Proves impl logic, NOT the frozen target.

## Test-owner patch proposal (UNAPPLIED — frozen tests untouched)
File `crates/tools/tests/mcp_config.rs`, exact edits for the test owner only:
1. After line 9 (`use serde_json::json;`) insert `use std::collections::HashMap;`
   (fixes E0433 at `:95` and `:109`).
2. `:74-:77` replace `"args": [format!("--arg{}", i).to_string(); 100],`
   with two lines: build `let args: Vec<String> = (0..100).map(|i| format!("--arg{i}")).collect();`
   before `json!`, then `"args": args,`.
3. `:84-:88` replace `"args": [long_arg.clone(); 10],`
   with `let args: Vec<String> = vec![long_arg.clone(); 10];` before `json!`,
   then `"args": args,`.
No impl change needed: `src/mcp_config.rs` already satisfies every assertion.

## Result
- Impl complete, no edit needed. Target blocked by frozen-test compile errors → honest status = blocked + unapplied test-owner proposal.
