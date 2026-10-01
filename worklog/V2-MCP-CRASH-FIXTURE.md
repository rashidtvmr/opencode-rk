# V2 MCP crash fixture repair

## Package

- Package: `MCP-CRASH-FIXTURE-PATH`
- Role: independent test owner, mechanical fixture maintenance
- Base candidate: `35779c7a7ad839558d39f60c3a53439885744af1`
- Allowed source path: `crates/tools/src/mcp_spawn.rs`
- Additional handoff path: `worklog/V2-MCP-CRASH-FIXTURE.md`

## Classification

The failure is a platform fixture-path defect, not a product or assertion defect.
The existing `disc111_t04_crash_bounded_retry_no_silent_restart` test uses a
macOS-nonexistent `/bin/false`. The independent platform probe recorded
`/bin/false` as false and `/usr/bin/false` as true on macOS; `/usr/bin/false` is
also the standard path on Ubuntu. The fixture therefore failed at
`spawn_server(...).expect("spawn must run")` with `ENOENT`, before the frozen
crash/retry assertions ran.

The replacement is one fixture program pathname literal only:

```text
/bin/false -> /usr/bin/false
```

The child still exits unsuccessfully, so the negative crash fixture intent is
unchanged. The caller-observed crash code remains `Some(3)` via `note_crash`;
the test does not read a child exit code as the crash code.

## Protected contract

No assertions, retry budget, error mapping, restart behavior, security checks,
or test command configuration were changed. The protected expectations remain:

- first `note_crash(Some(3))` is `SpawnError::Crashed { code: Some(3) }`;
- second crash has the same typed error;
- third call is `SpawnError::RestartsExhausted { used: 2 }`;
- exhausted budget does not silently restart (`!srv.is_running()`).

The external frozen shell fixture `8b39...` is unchanged.

## Evidence

- Canonical baseline source reproduction: clean SHA
  `89aeaa81be72e4af95dc6fb39c28f54c07c92ae1`.
- Baseline receipt: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-mcp-spawn-baseline-89aeaa8-53wpz1rc`.
- Baseline isolated result: `0 passed, 1 failed`; failure was the same
  `ENOENT` spawn failure, not a compiler failure.
- Candidate pre-repair result: formatting passed, library clippy passed,
  strict owned cases passed; full tools tests reported `122 passed, 1 failed`,
  with this MCP crash fixture failing at the spawn `expect`.
- Cargo.lock SHA-256 before and after: `63ef5299dd93286950af00388796375b06aefc5a4a3eedfa38361954fefb6f03`.
- Original test-module suffix/source evidence: `86723f...` (as recorded by the
  independent verifier).

## Verification status

No runtime, build, Cargo, browser, or test command was run by this fixture
owner, per package instructions. This is a candidate mechanical repair only;
the parent must run the prescribed `tools_owned_spec` and full tools tests
serially, then verify the exact integrated SHA before acceptance.
