# V2-NATIVE-CONTRACT-EVALUATION

Independent evaluator record for the native fixture. This is contract evidence,
not acceptance.

## Source conclusions

Pinned upstream is `95daf90670b7c039c436c85537da5fbfe2205b41` at
`/Users/mymac/Projects/opencode-upstream-reference`. Its run command rejects
interactive non-TTY stdout (`packages/opencode/src/cli/cmd/run.ts:319-320`) and
treats non-TTY stdin as piped input (`:416`; `cmd/tui.ts:60`). Local frozen
`app_start.rs:116-146` and tests `:580-627` require redirected native launch to
refuse raw mode with exit code 2. Those authorities supersede the old test's
attempt to run `--native --once` with null stdin/piped output.

Local main routing is distinct: no-subcommand native default calls `chat::run`
(`crates/cli/src/main.rs:221-258`, `chat.rs:48-86`), while `--native` calls
`tui_entry::run` and the `tui` subcommand also calls `tui_entry` directly
(`main.rs:234-245,278-284`). `tui_entry::run_with_dir` only resolves a bearer
when `--origin` is present (`tui_entry.rs:512-582`). Therefore changing T03 to
default chat would hide the explicit user-required `tui` discovery gap. The
candidate restores `tui` with no origin and retains the expected RED.

The old fixture forged a descriptor and token. The candidate instead starts
`serve` with piped bounded stdout/stderr, reads and validates its published
descriptor, uses its authenticated HTTP API to create live state, and binds T04
through `tui --origin --once`. PTY execution uses an embedded Python stdlib
driver with explicit argv and a `setsid` process group; cleanup kills/reaps the
known group before joining bounded readers. Plain `cmd.output()` calls were
replaced by deadline-bounded `Proc` execution with capped drains.
The model requirement remains honestly blocked: current `tui_entry.rs:530,545`
passes literal `"unset"`, and this test has no authorized configured/selected
model entrypoint. No first catalog model is invented.

## Required parent verification

Run the focused Cargo target only after the parent grants heavy execution. Check
the exact candidate source hash before running. Expected classifications:

1. T01/T02/T04 are runnable fixture checks but currently unverified.
2. T03 should fail for the genuine missing `tui` implicit descriptor-discovery
   behavior, not be changed to chat/default or `--origin` to manufacture GREEN.
3. Any compile failure is a fixture-maintenance blocker to repair within the
   owned test path; it is not RED evidence.
4. Do not claim PREVERIFIED or ACCEPTED from this handoff; only an independent
   run on the exact integrated SHA can establish those states.

## Handoff identity

- Supplied base: `f8c08e1759c0d10d3634ca3566ee767600c4e1e7`.
- Candidate test SHA: `ca201f8304c9cd8de93afe8149387543a0fc51ff1c41b37f13c524bdceb455d3`.
- Owned paths changed: `crates/cli/tests/native_daemon_flow.rs`,
  `worklog/V2-NATIVE-FIXTURE-MAINTENANCE.md`,
  `worklog/V2-NATIVE-CONTRACT-EVALUATION.md`.
- Commands run: source reads/grep/glob, `rustfmt`, `git diff --check`, `shasum`,
  `git rev-parse`; no Cargo/build/network command.
- Status: source-ready candidate, NOT PREVERIFIED / NOT ACCEPTED.
