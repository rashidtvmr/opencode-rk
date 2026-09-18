# LANE-CI

## Claim
- Task: LANE-CI
- Session: ses_worker_ci
- Owned file: `crates/cli/src/ci_output.rs`
- Status: completed

## Implementation
- Pure-state, std-only, `forbid(unsafe_code)` module
- `CiExitCode` enum with explicit discriminants (0, 10, 20, 30, 40, 64)
- `CiEvent` JSONL emitter with hand-written JSON escaping (no serde)
- `OutputFormat::Json|Jsonl|Text` with `TextRenderer` for stable plain lines
- Fail-closed rule: `render_approval_required()` always produces exit 20
- Bounded 8 KiB line length, oversize truncated to valid JSON

## Tests (6/6 green)
- T01: Exit code discriminants match documented table
- T02: JSONL escaping of quotes/newlines/backslashes
- T03: Bounded line enforcement (oversize truncated, never invalid JSON)
- T04: Approval-required always maps to exit 20 in every format
- T05: Text renderer stable columns
- T06: Empty event stream -> still valid single final line

## Verification
```
rustc --edition 2021 --test src/ci_output.rs -o /tmp/ci_output_test
/tmp/ci_output_test
# running 6 tests
# test result: ok. 6 passed; 0 failed
```
