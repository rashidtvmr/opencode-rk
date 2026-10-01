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
- Original test-module suffix SHA-256:
  `86723f843bbce81dc85d7de3d72f7ddce44e10019cf1ac34d9835702352f77e6`.
- Corrected test-module suffix SHA-256:
  `bc874cd73ee69f1ce80d41fa5236e89da99bdf4a2f28df500de95eb7d73c68ad`.

The controller independently compared the complete corrected test-module bytes
against current canonical source after substituting only the old fixture path:
they are identical. Every assertion and all other fixture data remain unchanged.
This independent mechanical fixture maintenance supersedes the original module
hash for the tools gates; it does not supersede any behavioral assertion.

## Verification status

No runtime, build, Cargo, browser, or test command was run by this fixture
owner, per package instructions. This is a candidate mechanical repair only;
the parent must run the prescribed `tools_owned_spec` and full tools tests
serially, then verify the exact integrated SHA before acceptance.

The controller prepared preserved `e06bbd5`, `35779c7` and `8843709` net changes
against newly accepted canonical base
`a6b188eaa852a1bcb769626eeb16d16b56406baf`. All product source bytes match the
preserved combined candidate, and all other in-module test regions, the strict
external shell fixture and Cargo.lock remain byte-identical. The worker's
source-only response label does not constitute runtime PREVERIFIED evidence;
the full controlled sequence must pass on this prepared current candidate.

## Exact integrated acceptance

Independent verifier PREVERIFIED the current combined candidate on exact
`e1acd5258fed50b1f79f54b412921266f07dc8ed`; the controller fast-forwarded the
sole canonical branch and repeated the unchanged tools gate manifest on that
exact integrated SHA. Formatting, tools-library Clippy, all five strict owned
process cases, and **458 full all-target/all-feature tools tests** pass with
zero failed/ignored and no forced cleanup. The crash fixture now executes its
unchanged typed-error, two-retry budget and no-silent-restart assertions.

The controller also probed the existing Ubuntu verification container:
`docker exec oc2-v2-ubuntu-build /usr/bin/test -x /usr/bin/false` exits 0.
This is executable-presence evidence; Ubuntu runtime verification remains
separate. State: **ACCEPTED for mechanical Mac fixture-path maintenance on
exact integrated `e1acd52`**. Complete receipts are in the companion worklogs.
