# BRIDGE-GAP-105 scratchpad

Claim: BRIDGE-GAP-105 via ses_gap105, scratchpad worklog/BRIDGE-GAP-105.md.
Source evidence:
- crates/opentui-bridge/src/run_command.rs:1-111 (RunCommand parse/render, MAX_NAME_LEN 64, MAX_ARGS 16, MAX_ARG_LEN 512, forbid unsafe, mod tests 5 tests).
- TS truth /home/rashid/projects/opencode/packages/opencode/src/cli/cmd/run/footer.command.tsx:443-459 (slash entries footer `/${name}`, Project vs MCP categories), :512-518 (onCommand(name) dispatch).
Observed scenario: file has generic RunCommand only; missing registry, strict slash call, help line.
Target boundary: EXTEND run_command.rs only, APPEND-only, no edits to existing items/tests, no lib.rs/Cargo.toml, no cargo, no commit/push.
Tests: new mod tests2, 6 tests: register_ok, register_dup_errs, register_cap_errs, parse_ok, parse_no_slash_errs, help_non_empty.
Decisions:
- Reuse MAX_NAME_LEN/MAX_ARGS; add MAX_REGISTRY 64 + MAX_CALL_ARG_LEN 256 (new caps per spec vs existing 512).
- register: trim, strip one leading '/', trunc 64, empty/dup/cap errs.
- parse_call: strict leading '/' (differs from RunCommand::parse which tolerates bare), trunc 64 / 16x256.
- help_line mirrors render form `/{name} {args}`.
Remaining unknowns: exact upstream error strings (spec-derived); chosen strings: "empty name", "duplicate command", "registry full", "missing leading slash", "empty command".
