# LANE-COMMANDS-LIVE — Scratchpad

## Claim
- Task: LANE-COMMANDS-LIVE (RAW_FEATURE 1.7 custom-commands gap, template engine)
- Session: ses_worker_commands
- Status: completed

## Source Evidence
- `RAW_FEATURE.md:117`: "Custom slash commands (markdown, $ARGUMENTS, !shell, @file) — LANDED-UNWIRED"
- `docs/research/UPSTREAM-V2-PARITY-SYNTHESIS.md:198`: `$1..$N` + `$ARGUMENTS` expansion, shell templates, @file mentions
- `crates/server/src/rules_loader.rs`: frontmatter house pattern (---, name:, body)
- `crates/agents/tests/agent_files.rs`: `#[path]` standalone test pattern

## Target Boundary
- Pure engine: parse/expand only, zero execution
- ShellPlan: typed data, requires_approval:true, broker executes later
- FileRef: validated path bounds, no traversal (no `..`, no absolute)
- Bounded: MAX_TEMPLATE_BYTES=64K, MAX_DESCRIPTION_BYTES=512, MAX_NAME_BYTES=128, MAX_POSITIONAL_ARGS=64, MAX_FILE_REFS=16
- Deterministic: same inputs → same output
- Round-trip: serialize → parse → identical CommandDef

## Tests Written (14)
- T01: $ARGUMENTS substitution
- T02: $1 $2 positional substitution
- T03: $ARGUMENTS empty args → empty string
- T04: @file mention → FileRef list
- T05: @file traversal (../) rejected
- T06: @file absolute path rejected
- T07: !shell prefix → ShellPlan, text retains raw
- T08: unknown variable → error
- T09: template size bound
- T10: argument count bound
- T11: file-ref count bound
- T12: determinism
- T13: round-trip serialize/parse
- T14: invalid name rejected

## Hashes
- RED (0/14 pass): ffaab087 (src) bd94a443 (test)
- GREEN (14/14 pass): fe89476b (src) bd94a443 (test — byte-identical RED vs GREEN)

## Decisions
- ShellPlan text retains raw template; broker strips `!shell` prefix when dispatching
- FileRef validated at extraction time (no lazy validation)
- validate_variables allows bare `$` as literal (not an error)
- round_trip via serialize_command_def → parse_command_def

## No-regress
- `cargo test -p opencode-rk-agents --lib`: 34/34 pass

## Remaining Unknowns
- None — lane is self-contained pure engine
