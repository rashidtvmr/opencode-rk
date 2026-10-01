# V2 CLI mechanical-quality candidate

## Package and authority

- Package: `V2-CLI-MECHANICAL-QUALITY` (`cli-mechanical` worker).
- Gate: existing `native-cli-clippy`, native `oc2` binary, `--no-deps -D warnings`.
- Base: `8fce37918a82efaf9b476e29eface0f82ae14322`.
- Branch: `v2/cli-mechanical-y52o0gi3`.
- Worktree: `/Users/mymac/Projects/opencode-rk-v2-cli-mechanical-y52o0gi3`.
- State: **CANDIDATE; PRODUCT ACCEPTANCE PENDING; CLI QUALITY STILL OPEN**.
- Candidate SHA is supplied in the worker's final handoff/receipt after commit.
- Implementer: GPT-6.1 Sol (`openai/gpt-6.1-sol`), parent-direct leaf worker.

The current explicit requirement authorizes only unambiguous mechanical changes
within the leased CLI paths. `AGENTS.md`, `PLAN.md`, `docs/CONVERGENCE.md`,
`docs/AGENT_STRATEGY_V2.md`, `docs/TDD.md`, and `docs/SECURITY.md` were read.
The parent owns integration and the serial Cargo/PTY/native validation slot.
The existing quality RED is the failure evidence for this maintenance package;
the worker neither authors nor alters frozen semantic tests.

`sources/upstream.lock.json` pins OpenCode to
`95daf90670b7c039c436c85537da5fbfe2205b41`. The read-only checkout at
`/Users/mymac/Projects/opencode-upstream-reference` was verified with
`git rev-parse HEAD` to equal that pin. Its `packages/cli/src/tui.ts:7-19`
(`runTui`) passes transport and resolved configuration to the native TUI runner.
This package changes no entrypoint, transport, configuration, or product contract;
that limited upstream inspection is context, not a parity acceptance claim.

## Exact existing RED

Receipt:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-today-current-quality-8fce379-a_tk7mj9/receipt.json`.

Log (same directory): `native-cli-clippy.log`, SHA-256
`8f79870bc87a62a8fd4cb0e5e5760351bfc667f78af8f6c52e8d6be5fdcf45ea`.

Recorded command on the exact base SHA, exit **101**:

```sh
/usr/bin/arch -arm64 cargo clippy --offline --locked -p opencode-rk-cli --features native --bin oc2 --no-deps -- -D warnings
```

Representative exact excerpts:

```text
error: unnecessary `>= y + 1` or `x - 1 >=`
   --> crates/cli/src/composer.rs:147:12
147 |         if n + 1 <= max_chars {

error: variable does not need to be mutable
  --> crates/cli/src/medown.rs:54:9
54 |     let mut flush = |buf: &mut String, spans: &mut Vec<Span>, bold: bool, code: bool| {

error: could not compile `opencode-rk-cli` (bin "oc2") due to 495 previous errors
```

The historical localized inventory is
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-grouped-salvage-priority-9_dtmq3z/synthesis/cli-quality-observed.json`
at `fe9830b2e61c07f2aee91c9d5a29c2111b495579`: 476 localized diagnostics,
453 `dead_inventory`, 23 `mechanical_or_correctness_review`. A read-only comparison
against the current base log confirmed the same diagnostic identities and counts.
One off-grant historical location moved: `tui_entry::run`, line 1001 to 1018.
All 19 diagnostic occurrences targeted here match their exact current-log locations.

## Candidate changes and equivalence evidence

Source locations in this table refer to the **base**, matching the RED log.
The patch addresses **19 logged diagnostic occurrences at 18 unique source
locations in 12 source files**. `can_transition` is diagnosed twice because
`native_transcript.rs` is compiled as two modules.

| Changed source path | Original lint / location | Mechanical change and inspected caller evidence |
| --- | --- | --- |
| `crates/cli/src/composer.rs` | `int_plus_one`, 147 | `n + 1 <= max_chars` becomes `n < max_chars`. `n` counts chars from the byte-bounded draft (`MAX_DRAFT = 8192`); the cursor-marked output branch is identical. Existing `display_marks_cursor`, `display_multibyte_safe`, and truncation assertions retain their exact bytes. |
| `crates/cli/src/medown.rs` | `unused_mut`, 54 | The `flush` binding becomes immutable. The closure mutates only its explicit `&mut` arguments; all three calls inside `parse_inline` remain identical. |
| `crates/cli/src/native_transcript.rs` | `match_like_matches_macro`, 62 (twice) | `ToolState::can_transition` uses `matches!` with the same three patterns: exactly the same five true pairs out of 16 possible pairs. Existing terminal/transition assertions remain byte-identical. |
| `crates/cli/src/chat.rs` | `unnecessary_map_or`, 378 | `map_or(true, predicate)` becomes `is_none_or(predicate)`: absent slash remains invalid; empty provider/id remain invalid. The `/model` caller at 451-456, rejection text, assignment, and successful output are identical. |
| `crates/cli/src/daemon_client.rs` | `doc_lazy_continuation`, 733 | Add a blank documentation line before the independent paragraph. `main.rs:229` still calls the same `discover_presence` body. |
| `crates/cli/src/diagnostics.rs` | `derivable_impls`, 187 | Derive `Default` for `ExportOptions`; both fields are `bool`, so both remain `false`. `redacted_export` default exclusion of secrets/transcripts and its existing assertions are preserved. |
| `crates/cli/src/install_commands.rs` | `manual_find`, 85 | Use `.into_iter().find(...)` on the same seven-element, `Copy` command array, with identical order, first-match behavior and empty/overlong guards. `unknown_subcommand_exit` still calls `lookup_command` at 118. |
| `crates/cli/src/modals.rs` | `manual_repeat_n`, 145 | `fit_cell` extends by exactly `missing` spaces using `repeat_n`; its two `render_top` callers at 103-104 retain the same char-safe width behavior. |
| `crates/cli/src/native_composer.rs` | `explicit_counter_loop`, 173; `doc_lazy_continuation`, 700-702 | Enumerate the same scalar iterator instead of incrementing `n`; the break precedes the same UTF-8 byte-offset increment. `EditBuffer::move_up` and `move_down` at 363/381 retain clamping and scalar-column behavior. Reflow the documentation's `+` to avoid an accidental Markdown list. |
| `crates/cli/src/native_theme.rs` | `must_use_unit`, 366 | Remove the meaningless `#[must_use]` on unit-returning `register_builtins`. Its registration calls and existing builtin lookup/application tests are unchanged. |
| `crates/cli/src/terminal_host.rs` | `doc_lazy_continuation`, 45-48 | Add a blank documentation line between the guard list and its common ceiling paragraph. No host state or restoration implementation changes. |
| `crates/cli/src/title.rs` | `manual_repeat_n`, 98 | `AppTitle::status_bar` extends by the same `width - len` spaces in the existing `len < width` branch. Rendering and char-count bounds are identical. |

`Cargo.toml:23` declares workspace MSRV **1.85**, inherited by
`crates/cli/Cargo.toml:7`. `std::iter::repeat_n` and `Option::is_none_or`
were stabilized in Rust 1.82 and fit that MSRV. No manifest/dependency changes
are needed.

The only additional changed path is this new worklog:
`worklog/V2-CLI-MECHANICAL-QUALITY.md`.

## Remaining observed quality failures and parent-owned patches

**The 453-entry historical dead/unwired inventory remains preserved and open.**
That classification includes three `unused_assignments` occurrences
(`native_transcript.rs:496`, twice; `native_timeline.rs:277`, once).
No blanket `allow`/`expect`, test-only module hiding, deletion of inventory,
unrelated API activation, or semantic assertion changes were used.

Four localized mechanical-review diagnostics remain outside this patch:

- `tui_entry.rs:339`: `redundant_field_names`.
- `tui_entry.rs:373`: `ptr_arg`.
- `tui_entry.rs:491`: `while_let_loop`.
- `app_start.rs:56`: `enum_variant_names`. The three `HeadlessReason` variant
  names are referenced by frozen assertions in that file (base lines 409-629)
  and contribute to derived `Debug` output. A rename requires independently
  authorized contract/test-owner maintenance; it is not silently performed here.

The current log additionally reports the non-localized `duplicate_mod` failure:
`native_transcript.rs` is loaded by both `main.rs:44` and `tui_entry.rs:25-27`.
The following minimal **unapplied** parent-owned patch covers the three
unambiguous `tui_entry` lints and consolidates that duplicate module:

```diff
--- a/crates/cli/src/tui_entry.rs
+++ b/crates/cli/src/tui_entry.rs
@@
 #[cfg(feature = "native")]
-#[path = "native_transcript.rs"]
-mod native_transcript;
+use crate::native_transcript;
@@
-        last_text: last_text,
+        last_text,
@@
-fn display_name(p: &PathBuf) -> String {
+fn display_name(p: &Path) -> String {
@@
-    loop {
-        let Some(line) = lines.next() else { break };
+    while let Some(line) = lines.next() {
         let line = line?;
```

`Path` is already imported at `tui_entry.rs:43`; `load_memory`'s `&PathBuf`
arguments coerce to `&Path`. `while let` retains read-error propagation, command
handling and EOF/quit behavior. The module consolidation reuses the unchanged
crate-root module for all six inspected `strip_ansi` call sites (635-663, 957).
It changes module identity and eliminates the duplicate unit-test namespace;
the parent must validate that consolidation independently. It is deliberately
not part of this candidate's grant.

The last observed global gate remains **495 errors / exit 101 on the base**.
No post-candidate diagnostic count or global Clippy GREEN is claimed.

## Worker checks and validation handoff

Actual lightweight checks:

```sh
rustfmt --edition 2021 --check --config skip_children=true crates/cli/src/composer.rs crates/cli/src/medown.rs crates/cli/src/native_transcript.rs crates/cli/src/chat.rs crates/cli/src/daemon_client.rs crates/cli/src/diagnostics.rs crates/cli/src/install_commands.rs crates/cli/src/modals.rs crates/cli/src/native_composer.rs crates/cli/src/native_theme.rs crates/cli/src/terminal_host.rs crates/cli/src/title.rs
git diff --check
```

- Standalone rustfmt initially requested multiline formatting of `find`; that
  correction was applied through the patch tool. The final 12-file check exited
  **0**. It parses/formats sources only, with child traversal disabled.
- `git diff --check` exited **0**.
- Read-only `python3 -B` source audit exited **0**: all 12 changed source paths
  match the scoped repair set; every inline `#[cfg(test)]` section is byte-equal
  to the base; added lines contain no suppressions/cfg hiding/unsafe/panic/todo;
  current RED and historical localized diagnostic identities match; all 19
  targeted occurrences match exact current-log locations. The initial full
  location comparison caught the off-grant `tui_entry::run` line drift described
  above, then the audit compared diagnostic identity and exact scoped locations.
- `git diff -- crates/cli/src` was reviewed against the equivalence table above.
- A final read-only `python3 -B` path-grant/receipt audit exited **0**: exactly
  the 12 source files plus this new worklog are changed; the base matches the
  current quality receipt; the log's computed SHA-256 matches the receipt; the
  recorded native CLI exit is 101.
- No Cargo, native/PTY, container, network, or database commands were run by
  this worker. No compiler/test GREEN, independent PREVERIFIED, or integrated
  ACCEPTED result is asserted.

Proposed smallest existing regression target: the native `oc2` **binary unit
test target**, which includes the unchanged tests for the repaired pure modules.
For independent validation under the parent's serial slot and disposable
HOME/XDG profile matching the receipt:

```sh
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 /usr/bin/arch -arm64 cargo test --offline --locked -p opencode-rk-cli --features native --bin oc2
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 /usr/bin/arch -arm64 cargo clippy --offline --locked -p opencode-rk-cli --features native --bin oc2 --no-deps -- -D warnings
```

These commands have **not** run on the candidate. The Clippy rerun must account
for the remaining inventory, four deferred localized lints, and duplicate module;
the worker's targeted source repairs cannot establish global quality acceptance.
Acceptance requires independent validation and the parent-owned gate rerun on
the exact integrated revision.
